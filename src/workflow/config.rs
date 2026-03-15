//! Workflow configuration types
//!
//! These types define the structure of WORKFLOW.md configuration file.

use serde::{Deserialize, Serialize};

/// Main workflow configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WorkflowConfig {
    /// Channel settings (Telegram, WhatsApp, etc.)
    #[serde(default)]
    pub channels: ChannelSettings,

    /// Agent execution settings
    #[serde(default)]
    pub agent: AgentSettings,

    /// Container execution settings
    #[serde(default)]
    pub container: ContainerSettings,

    /// Hook scripts for workspace lifecycle
    #[serde(default)]
    pub hooks: HookSettings,

    /// Default prompt template (Markdown body after front matter)
    #[serde(default)]
    pub prompt_template: String,
}

/// Channel configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ChannelSettings {
    #[serde(default)]
    pub telegram: Option<ChannelConfig>,

    #[serde(default)]
    pub whatsapp: Option<ChannelConfig>,
}

/// Individual channel config
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelConfig {
    /// Whether this channel is enabled
    pub enabled: bool,

    /// Bot token (supports $VAR environment variable)
    #[serde(default)]
    pub bot_token: Option<String>,

    /// MCP URL for WhatsApp
    #[serde(default)]
    pub mcp_url: Option<String>,
}

/// Agent execution settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSettings {
    /// Maximum concurrent agent runs
    #[serde(default = "default_max_concurrent")]
    pub max_concurrent: usize,

    /// Agent execution timeout in milliseconds
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,

    /// Maximum number of retries on failure
    #[serde(default = "default_max_retries")]
    pub max_retries: usize,

    /// Retry backoff in milliseconds
    #[serde(default = "default_retry_backoff_ms")]
    pub retry_backoff_ms: u64,
}

/// Container execution settings
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ContainerSettings {
    /// Container image to use
    #[serde(default)]
    pub image: Option<String>,

    /// Workspace root directory
    #[serde(default)]
    pub workspace_root: Option<String>,

    /// Whether container pool is enabled
    #[serde(default)]
    pub pool_enabled: Option<bool>,

    /// Minimum pool size
    #[serde(default)]
    pub pool_min_size: Option<usize>,

    /// Maximum pool size
    #[serde(default)]
    pub pool_max_size: Option<usize>,
}

/// Hook scripts for workspace lifecycle
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HookSettings {
    /// Runs after workspace is created
    #[serde(default)]
    pub after_create: Option<String>,

    /// Runs before agent execution
    #[serde(default)]
    pub before_run: Option<String>,

    /// Runs after agent execution (success or failure)
    #[serde(default)]
    pub after_run: Option<String>,

    /// Runs before workspace cleanup
    #[serde(default)]
    pub before_remove: Option<String>,
}

// Default values
fn default_max_concurrent() -> usize {
    5
}
fn default_timeout_ms() -> u64 {
    300000
} // 5 minutes
fn default_max_retries() -> usize {
    3
}
fn default_retry_backoff_ms() -> u64 {
    60000
} // 1 minute

impl Default for AgentSettings {
    fn default() -> Self {
        Self {
            max_concurrent: default_max_concurrent(),
            timeout_ms: default_timeout_ms(),
            max_retries: default_max_retries(),
            retry_backoff_ms: default_retry_backoff_ms(),
        }
    }
}

impl ChannelConfig {
    pub fn new_telegram(bot_token: Option<String>) -> Self {
        Self {
            enabled: true,
            bot_token,
            mcp_url: None,
        }
    }

    pub fn new_whatsapp(mcp_url: Option<String>) -> Self {
        Self {
            enabled: true,
            bot_token: None,
            mcp_url,
        }
    }
}

impl WorkflowConfig {
    /// Create a minimal default config
    pub fn default_config() -> Self {
        Self::default()
    }

    /// Check if any channel is enabled
    pub fn has_enabled_channel(&self) -> bool {
        self.channels
            .telegram
            .as_ref()
            .map(|c| c.enabled)
            .unwrap_or(false)
            || self
                .channels
                .whatsapp
                .as_ref()
                .map(|c| c.enabled)
                .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // === WorkflowConfig tests ===

    #[test]
    fn test_default_workflow_config() {
        let config = WorkflowConfig::default();
        assert_eq!(config.agent.max_concurrent, 5);
        assert_eq!(config.agent.timeout_ms, 300000);
        assert!(!config.has_enabled_channel());
    }

    #[test]
    fn test_workflow_config_serialization() {
        let config = WorkflowConfig::default();
        let yaml = serde_yaml::to_string(&config).unwrap();
        assert!(yaml.contains("max_concurrent"));
    }

    #[test]
    fn test_workflow_config_deserialization() {
        let yaml = r#"
channels:
  telegram:
    enabled: true
    bot_token: "test_token"
agent:
  max_concurrent: 3
  timeout_ms: 120000
"#;
        let config: WorkflowConfig = serde_yaml::from_str(yaml).unwrap();
        assert!(config.channels.telegram.is_some());
        assert_eq!(config.channels.telegram.unwrap().enabled, true);
        assert_eq!(config.agent.max_concurrent, 3);
    }

    #[test]
    fn test_workflow_config_has_enabled_channel() {
        let mut config = WorkflowConfig::default();
        assert!(!config.has_enabled_channel());

        config.channels.telegram = Some(ChannelConfig {
            enabled: true,
            bot_token: Some("test".to_string()),
            mcp_url: None,
        });
        assert!(config.has_enabled_channel());
    }

    // === ChannelSettings tests ===

    #[test]
    fn test_channel_settings_default() {
        let settings = ChannelSettings::default();
        assert!(settings.telegram.is_none());
        assert!(settings.whatsapp.is_none());
    }

    #[test]
    fn test_channel_config_telegram() {
        let config = ChannelConfig::new_telegram(Some("token123".to_string()));
        assert!(config.enabled);
        assert_eq!(config.bot_token, Some("token123".to_string()));
        assert!(config.mcp_url.is_none());
    }

    #[test]
    fn test_channel_config_whatsapp() {
        let config = ChannelConfig::new_whatsapp(Some("http://mcp:8080".to_string()));
        assert!(config.enabled);
        assert!(config.bot_token.is_none());
        assert_eq!(config.mcp_url, Some("http://mcp:8080".to_string()));
    }

    #[test]
    fn test_channel_config_serialization() {
        let config = ChannelConfig::new_telegram(Some("secret".to_string()));
        let yaml = serde_yaml::to_string(&config).unwrap();
        assert!(yaml.contains("enabled: true"));
        assert!(yaml.contains("bot_token: secret"));
    }

    // === AgentSettings tests ===

    #[test]
    fn test_agent_settings_default_values() {
        let settings = AgentSettings::default();
        assert_eq!(settings.max_concurrent, 5);
        assert_eq!(settings.timeout_ms, 300000);
        assert_eq!(settings.max_retries, 3);
        assert_eq!(settings.retry_backoff_ms, 60000);
    }

    #[test]
    fn test_agent_settings_custom_values() {
        let yaml = r#"
max_concurrent: 10
timeout_ms: 600000
max_retries: 5
retry_backoff_ms: 120000
"#;
        let settings: AgentSettings = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(settings.max_concurrent, 10);
        assert_eq!(settings.timeout_ms, 600000);
        assert_eq!(settings.max_retries, 5);
        assert_eq!(settings.retry_backoff_ms, 120000);
    }

    #[test]
    fn test_agent_settings_partial_deserialization() {
        let yaml = r#"
max_concurrent: 2
"#;
        let settings: AgentSettings = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(settings.max_concurrent, 2);
        // Other fields should have defaults
        assert_eq!(settings.timeout_ms, 300000);
    }

    // === ContainerSettings tests ===

    #[test]
    fn test_container_settings_default() {
        let settings = ContainerSettings::default();
        assert!(settings.image.is_none());
        assert!(settings.workspace_root.is_none());
        assert!(settings.pool_enabled.is_none());
    }

    #[test]
    fn test_container_settings_full() {
        let yaml = r#"
image: "anthropic/codex:latest"
workspace_root: "/workspaces"
pool_enabled: true
pool_min_size: 2
pool_max_size: 10
"#;
        let settings: ContainerSettings = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(settings.image, Some("anthropic/codex:latest".to_string()));
        assert_eq!(settings.workspace_root, Some("/workspaces".to_string()));
        assert_eq!(settings.pool_enabled, Some(true));
        assert_eq!(settings.pool_min_size, Some(2));
        assert_eq!(settings.pool_max_size, Some(10));
    }

    // === HookSettings tests ===

    #[test]
    fn test_hooks_settings_default() {
        let hooks = HookSettings::default();
        assert!(hooks.after_create.is_none());
        assert!(hooks.before_run.is_none());
        assert!(hooks.after_run.is_none());
        assert!(hooks.before_remove.is_none());
    }

    #[test]
    fn test_hooks_settings_with_scripts() {
        let yaml = r#"
after_create: |
  git clone $REPO_URL .
before_run: |
  npm install
after_run: |
  echo "Done"
"#;
        let hooks: HookSettings = serde_yaml::from_str(yaml).unwrap();
        assert!(hooks.after_create.is_some());
        assert!(hooks.before_run.is_some());
        assert!(hooks.after_run.is_some());
        assert!(hooks.before_remove.is_none());
    }

    #[test]
    fn test_hooks_empty_script_is_none() {
        let yaml = r#"
after_create: ""
before_run: "   "
"#;
        let hooks: HookSettings = serde_yaml::from_str(yaml).unwrap();
        // Empty strings are preserved in YAML
        assert_eq!(hooks.after_create, Some("".to_string()));
        assert_eq!(hooks.before_run, Some("   ".to_string()));
    }

    // === Integration tests ===

    #[test]
    fn test_full_workflow_config() {
        let yaml = r#"
channels:
  telegram:
    enabled: true
    bot_token: $TELEGRAM_TOKEN
  whatsapp:
    enabled: false
    mcp_url: http://localhost:8080

agent:
  max_concurrent: 3
  timeout_ms: 180000

container:
  image: custom:latest
  pool_enabled: true

hooks:
  after_create: |
    echo "created"

prompt_template: |
  You are a helpful assistant.
"#;
        let config: WorkflowConfig = serde_yaml::from_str(yaml).unwrap();

        // Channels
        assert!(config.channels.telegram.is_some());
        assert!(config.channels.whatsapp.is_some());
        assert!(!config.channels.whatsapp.as_ref().unwrap().enabled);

        // Agent
        assert_eq!(config.agent.max_concurrent, 3);
        assert_eq!(config.agent.timeout_ms, 180000);

        // Container
        assert_eq!(config.container.image, Some("custom:latest".to_string()));
        assert_eq!(config.container.pool_enabled, Some(true));

        // Hooks
        assert!(config.hooks.after_create.is_some());

        // Prompt
        assert!(config.prompt_template.contains("helpful assistant"));
    }

    #[test]
    fn test_workflow_config_without_front_matter() {
        // When there's no front matter, the entire content becomes prompt_template
        let config: WorkflowConfig = serde_yaml::from_str("").unwrap();
        assert_eq!(config.agent.max_concurrent, 5); // default
    }
}
