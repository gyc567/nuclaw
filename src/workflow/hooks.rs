//! Hook runner for workspace lifecycle

use std::process::Command;
use std::path::Path;
use thiserror::Error;

use crate::workflow::config::HookSettings;

#[derive(Error, Debug)]
pub enum HookError {
    #[error("Hook execution failed: {0}")]
    ExecutionFailed(String),
    
    #[error("Hook not found: {0}")]
    NotFound(String),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookType {
    AfterCreate,
    BeforeRun,
    AfterRun,
    BeforeRemove,
}

impl HookType {
    pub fn as_str(&self) -> &'static str {
        match self {
            HookType::AfterCreate => "after_create",
            HookType::BeforeRun => "before_run",
            HookType::AfterRun => "after_run",
            HookType::BeforeRemove => "before_remove",
        }
    }
}

pub struct HookRunner;

impl HookRunner {
    pub fn run_hook(
        hook_type: HookType,
        script: &str,
        workspace_path: &Path,
    ) -> Result<String, HookError> {
        if script.trim().is_empty() {
            return Ok(String::new());
        }
        
        let output = Command::new("sh")
            .arg("-c")
            .arg(script)
            .current_dir(workspace_path)
            .output()?;
        
        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            Err(HookError::ExecutionFailed(stderr))
        }
    }
    
    pub fn run_hooks(
        hook_type: HookType,
        settings: &HookSettings,
        workspace_path: &Path,
    ) -> Result<Vec<String>, HookError> {
        let script = match hook_type {
            HookType::AfterCreate => settings.after_create.as_deref(),
            HookType::BeforeRun => settings.before_run.as_deref(),
            HookType::AfterRun => settings.after_run.as_deref(),
            HookType::BeforeRemove => settings.before_remove.as_deref(),
        };
        
        match script {
            Some(s) if !s.trim().is_empty() => {
                let output = Self::run_hook(hook_type, s, workspace_path)?;
                Ok(vec![output])
            },
            _ => Ok(vec![]),
        }
    }
    
    pub fn run_all_hooks(
        settings: &HookSettings,
        workspace_path: &Path,
    ) -> Result<(), HookError> {
        let hook_types = [
            HookType::AfterCreate,
            HookType::BeforeRun,
            HookType::AfterRun,
            HookType::BeforeRemove,
        ];
        
        for hook_type in hook_types {
            Self::run_hooks(hook_type, settings, workspace_path)?;
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_hook_type_as_str() {
        assert_eq!(HookType::AfterCreate.as_str(), "after_create");
        assert_eq!(HookType::BeforeRun.as_str(), "before_run");
        assert_eq!(HookType::AfterRun.as_str(), "after_run");
        assert_eq!(HookType::BeforeRemove.as_str(), "before_remove");
    }

    #[test]
    fn test_run_empty_script() {
        let temp_dir = TempDir::new().unwrap();
        let result = HookRunner::run_hook(HookType::BeforeRun, "", temp_dir.path());
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn test_run_simple_echo() {
        let temp_dir = TempDir::new().unwrap();
        let result = HookRunner::run_hook(HookType::BeforeRun, "echo hello", temp_dir.path());
        assert!(result.is_ok());
        assert!(result.unwrap().contains("hello"));
    }

    #[test]
    fn test_run_hook_with_env_var() {
        let temp_dir = TempDir::new().unwrap();
        std::env::set_var("TEST_WORKSPACE", "test_value");
        let result = HookRunner::run_hook(
            HookType::BeforeRun,
            "echo $TEST_WORKSPACE",
            temp_dir.path(),
        );
        assert!(result.is_ok());
        assert!(result.unwrap().contains("test_value"));
        std::env::remove_var("TEST_WORKSPACE");
    }

    #[test]
    fn test_run_hook_failure() {
        let temp_dir = TempDir::new().unwrap();
        let result = HookRunner::run_hook(HookType::BeforeRun, "exit 1", temp_dir.path());
        assert!(result.is_err());
    }

    #[test]
    fn test_run_hooks_with_settings() {
        let temp_dir = TempDir::new().unwrap();
        let mut settings = HookSettings::default();
        settings.before_run = Some("echo before".to_string());
        
        let result = HookRunner::run_hooks(HookType::BeforeRun, &settings, temp_dir.path());
        assert!(result.is_ok());
        assert!(!result.unwrap().is_empty());
    }

    #[test]
    fn test_run_hooks_skips_empty() {
        let temp_dir = TempDir::new().unwrap();
        let settings = HookSettings::default();
        
        let result = HookRunner::run_hooks(HookType::BeforeRun, &settings, temp_dir.path());
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn test_run_all_hooks() {
        let temp_dir = TempDir::new().unwrap();
        let mut settings = HookSettings::default();
        settings.after_create = Some("echo created".to_string());
        settings.before_run = Some("echo running".to_string());
        
        let result = HookRunner::run_all_hooks(&settings, temp_dir.path());
        assert!(result.is_ok());
    }

    #[test]
    fn test_hook_runs_in_workspace_directory() {
        let temp_dir = TempDir::new().unwrap();
        
        let script = "pwd";
        let result = HookRunner::run_hook(HookType::BeforeRun, script, temp_dir.path());
        
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.contains(".tmp") || output.contains("Temp"));
    }

    #[test]
    fn test_hook_multiline_script() {
        let temp_dir = TempDir::new().unwrap();
        
        let script = r#"
echo "line1"
echo "line2"
"#;
        let result = HookRunner::run_hook(HookType::BeforeRun, script, temp_dir.path());
        
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.contains("line1"));
        assert!(output.contains("line2"));
    }
}
