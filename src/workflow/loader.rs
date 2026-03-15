//! Workflow loader - parses WORKFLOW.md with YAML front matter

use std::env;
use std::fs;
use std::path::Path;
use regex::Regex;

use crate::workflow::config::{ChannelConfig, WorkflowConfig};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum WorkflowLoaderError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("YAML parse error: {0}")]
    YamlParse(#[from] serde_yaml::Error),
    
    #[error("File not found: {0}")]
    FileNotFound(String),
    
    #[error("Config validation error: {0}")]
    ValidationError(String),
    
    #[error("Environment variable error: {0}")]
    EnvVarError(String),
}

pub struct WorkflowLoader;

impl WorkflowLoader {
    pub fn parse_workflow_content(content: &str) -> Result<(WorkflowConfig, String), WorkflowLoaderError> {
        let content = content.trim();
        
        if content.is_empty() {
            return Ok((WorkflowConfig::default(), String::new()));
        }
        
        if !content.starts_with("---") {
            return Ok((WorkflowConfig::default(), content.to_string()));
        }
        
        let mut parts = content.splitn(3, "---");
        parts.next();
        
        let yaml_content = match parts.next() {
            Some(s) => s.trim(),
            None => return Ok((WorkflowConfig::default(), String::new())),
        };
        
        if yaml_content.is_empty() {
            return Ok((WorkflowConfig::default(), String::new()));
        }
        
        let config: WorkflowConfig = serde_yaml::from_str(yaml_content)?;
        
        let remaining = parts.next().unwrap_or("").trim().to_string();
        
        Ok((config, remaining))
    }
    
    pub fn resolve_env_vars(input: &str) -> Result<String, WorkflowLoaderError> {
        let re = Regex::new(r"\$\{(\w+)\}|\$(\w+)").map_err(|e| WorkflowLoaderError::EnvVarError(e.to_string()))?;
        
        let result = re.replace_all(input, |caps: &regex::Captures| {
            let var_name = caps.get(1).or(caps.get(2)).map(|m| m.as_str()).unwrap_or("");
            env::var(var_name).unwrap_or_else(|_| caps[0].to_string())
        });
        
        Ok(result.to_string())
    }
    
    pub fn resolve_env_vars_in_config(yaml: &str) -> Result<String, WorkflowLoaderError> {
        Self::resolve_env_vars(yaml)
    }
    
    pub fn validate_config(config: &WorkflowConfig) -> Result<(), WorkflowLoaderError> {
        if let Some(telegram) = &config.channels.telegram {
            if telegram.enabled && telegram.bot_token.is_none() && telegram.mcp_url.is_none() {
                return Err(WorkflowLoaderError::ValidationError(
                    "Telegram channel is enabled but has no bot_token or mcp_url".to_string()
                ));
            }
        }
        
        if config.agent.timeout_ms == 0 {
            return Err(WorkflowLoaderError::ValidationError(
                "timeout_ms must be greater than 0".to_string()
            ));
        }
        
        if config.agent.max_retries > 10 {
            return Err(WorkflowLoaderError::ValidationError(
                "max_retries must be between 0 and 10".to_string()
            ));
        }
        
        Ok(())
    }
    
    pub fn load_workflow(path: &Path) -> Result<(WorkflowConfig, String), WorkflowLoaderError> {
        if !path.exists() {
            return Err(WorkflowLoaderError::FileNotFound(path.display().to_string()));
        }
        
        let content = fs::read_to_string(path)?;
        
        let (mut config, template) = Self::parse_workflow_content(&content)?;
        
        let yaml_for_resolution = serde_yaml::to_string(&config).map_err(WorkflowLoaderError::YamlParse)?;
        let resolved_yaml = Self::resolve_env_vars_in_config(&yaml_for_resolution)?;
        
        config = serde_yaml::from_str(&resolved_yaml)?;
        
        Ok((config, template))
    }
    
    pub fn load_and_validate(path: &Path) -> Result<(WorkflowConfig, String), WorkflowLoaderError> {
        let (config, template) = Self::load_workflow(path)?;
        Self::validate_config(&config)?;
        Ok((config, template))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::fs;
    use std::path::Path;
    use tempfile::TempDir;

    #[test]
    fn test_parse_workflow_with_front_matter() {
        let content = r#"---
channels:
  telegram:
    enabled: true
    bot_token: test_token
agent:
  max_concurrent: 3
---

This is the prompt template.
"#;
        let result = WorkflowLoader::parse_workflow_content(content);
        assert!(result.is_ok());
        let (config, template) = result.unwrap();
        assert_eq!(config.agent.max_concurrent, 3);
        assert!(config.channels.telegram.is_some());
        assert_eq!(template.trim(), "This is the prompt template.");
    }

    #[test]
    fn test_parse_workflow_without_front_matter() {
        let content = "Just a prompt without config.";
        let result = WorkflowLoader::parse_workflow_content(content);
        assert!(result.is_ok());
        let (config, template) = result.unwrap();
        assert_eq!(config.agent.max_concurrent, 5);
        assert_eq!(template.trim(), "Just a prompt without config.");
    }

    #[test]
    fn test_parse_workflow_empty() {
        let result = WorkflowLoader::parse_workflow_content("");
        assert!(result.is_ok());
        let (config, template) = result.unwrap();
        assert_eq!(config.agent.max_concurrent, 5);
        assert!(template.is_empty());
    }

    #[test]
    fn test_parse_workflow_only_front_matter() {
        let content = r#"---
agent:
  timeout_ms: 600000
---
"#;
        let result = WorkflowLoader::parse_workflow_content(content);
        assert!(result.is_ok());
        let (config, template) = result.unwrap();
        assert_eq!(config.agent.timeout_ms, 600000);
        assert!(template.is_empty());
    }

    #[test]
    fn test_parse_workflow_invalid_yaml() {
        let content = r#"---
invalid: yaml: content::
---
Prompt"#;
        let result = WorkflowLoader::parse_workflow_content(content);
        assert!(result.is_err());
    }

    #[test]
    fn test_resolve_env_vars_simple() {
        env::set_var("TEST_VAR", "test_value");
        let input = "prefix $TEST_VAR suffix";
        let result = WorkflowLoader::resolve_env_vars(input);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "prefix test_value suffix");
        env::remove_var("TEST_VAR");
    }

    #[test]
    fn test_resolve_env_vars_multiple() {
        env::set_var("VAR1", "a");
        env::set_var("VAR2", "b");
        let input = "$VAR1 and $VAR2";
        let result = WorkflowLoader::resolve_env_vars(input);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "a and b");
        env::remove_var("VAR1");
        env::remove_var("VAR2");
    }

    #[test]
    fn test_resolve_env_vars_undefined() {
        env::remove_var("UNDEFINED_VAR");
        let input = "value: $UNDEFINED_VAR";
        let result = WorkflowLoader::resolve_env_vars(input);
        assert!(result.is_ok());
        assert!(result.unwrap().contains("UNDEFINED_VAR"));
    }

    #[test]
    fn test_resolve_env_vars_in_config() {
        env::set_var("TELEGRAM_TOKEN", "secret123");
        let yaml = r#"
channels:
  telegram:
    enabled: true
    bot_token: $TELEGRAM_TOKEN
"#;
        let result = WorkflowLoader::resolve_env_vars_in_config(yaml);
        assert!(result.is_ok());
        assert!(result.unwrap().contains("secret123"));
        env::remove_var("TELEGRAM_TOKEN");
    }

    #[test]
    fn test_resolve_env_vars_braces() {
        env::set_var("MY_VAR", "bracketed");
        let input = "value: ${MY_VAR}";
        let result = WorkflowLoader::resolve_env_vars(input);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "value: bracketed");
        env::remove_var("MY_VAR");
    }

    #[test]
    fn test_validate_config_valid() {
        let config = WorkflowConfig::default();
        let result = WorkflowLoader::validate_config(&config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_config_enabled_channel_no_token() {
        let mut config = WorkflowConfig::default();
        config.channels.telegram = Some(ChannelConfig {
            enabled: true,
            bot_token: None,
            mcp_url: None,
        });
        let result = WorkflowLoader::validate_config(&config);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_config_invalid_timeout() {
        let mut config = WorkflowConfig::default();
        config.agent.timeout_ms = 0;
        let result = WorkflowLoader::validate_config(&config);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_config_invalid_max_retries() {
        let mut config = WorkflowConfig::default();
        config.agent.max_retries = 100;
        let result = WorkflowLoader::validate_config(&config);
        assert!(result.is_err());
    }

    #[test]
    fn test_load_workflow_from_file() {
        let temp_dir = TempDir::new().unwrap();
        let workflow_path = temp_dir.path().join("WORKFLOW.md");
        
        let content = r#"---
agent:
  max_concurrent: 7
---
Prompt content here.
"#;
        fs::write(&workflow_path, content).unwrap();
        
        let result = WorkflowLoader::load_workflow(&workflow_path);
        assert!(result.is_ok());
        let (config, template) = result.unwrap();
        assert_eq!(config.agent.max_concurrent, 7);
        assert!(template.contains("Prompt content"));
    }

    #[test]
    fn test_load_workflow_file_not_found() {
        let result = WorkflowLoader::load_workflow(Path::new("/nonexistent/path/WORKFLOW.md"));
        assert!(result.is_err());
    }

    #[test]
    fn test_load_workflow_with_env_resolution() {
        env::set_var("MY_IMAGE", "custom:latest");
        
        let temp_dir = TempDir::new().unwrap();
        let workflow_path = temp_dir.path().join("WORKFLOW.md");
        
        let content = r#"---
container:
  image: $MY_IMAGE
agent:
  max_concurrent: 2
---
Run the agent.
"#;
        fs::write(&workflow_path, content).unwrap();
        
        let result = WorkflowLoader::load_workflow(&workflow_path);
        assert!(result.is_ok());
        let (config, _) = result.unwrap();
        assert_eq!(config.container.image, Some("custom:latest".to_string()));
        assert_eq!(config.agent.max_concurrent, 2);
        
        env::remove_var("MY_IMAGE");
    }

    #[test]
    fn test_load_and_validate_workflow() {
        let temp_dir = TempDir::new().unwrap();
        let workflow_path = temp_dir.path().join("WORKFLOW.md");
        
        let content = r#"---
channels:
  telegram:
    enabled: true
    bot_token: valid_token
agent:
  timeout_ms: 60000
---
Prompt here.
"#;
        fs::write(&workflow_path, content).unwrap();
        
        let result = WorkflowLoader::load_and_validate(&workflow_path);
        assert!(result.is_ok());
        let (config, _) = result.unwrap();
        assert!(config.has_enabled_channel());
    }
}
