//! NuClaw Onboard - Interactive configuration setup
//!
//! Provides an interactive CLI wizard to configure:
//! - LLM Provider (API Key, Base URL)
//! - Telegram Bot Token

use crate::config::nuclaw_home;
use crate::error::{NuClawError, Result};
use crate::providers::PROVIDERS;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;

/// Configuration file name
const ENV_FILE: &str = ".env";

/// Get the environment file path
pub fn env_file_path() -> PathBuf {
    nuclaw_home().join(ENV_FILE)
}

/// Configuration items to set
#[derive(Debug, Clone, Default)]
pub struct OnboardConfig {
    pub provider: Option<String>,
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    pub telegram_token: Option<String>,
}

impl OnboardConfig {
    /// Create a new empty config
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if any configuration exists
    pub fn has_any_config(&self) -> bool {
        self.api_key.is_some() || self.telegram_token.is_some()
    }
}

/// Prompt user for input with a default value
fn prompt_with_default(prompt: &str, default: &str) -> String {
    print!("{} [{}]: ", prompt, default);
    io::stdout().flush().ok();

    let mut input = String::new();
    if io::stdin().read_line(&mut input).is_ok() {
        let input = input.trim();
        if input.is_empty() {
            default.to_string()
        } else {
            input.to_string()
        }
    } else {
        default.to_string()
    }
}

/// Prompt user for sensitive input (no echo)
fn prompt_password(prompt: &str) -> String {
    print!("{}: ", prompt);
    io::stdout().flush().ok();

    // Simple password input - in production could use rpassword crate
    let mut input = String::new();
    if io::stdin().read_line(&mut input).is_ok() {
        input.trim().to_string()
    } else {
        String::new()
    }
}

/// Prompt user for yes/no confirmation
fn prompt_yes_no(prompt: &str, default: bool) -> bool {
    let default_str = if default { "Y/n" } else { "y/N" };
    print!("{} [{}]: ", prompt, default_str);
    io::stdout().flush().ok();

    let mut input = String::new();
    if io::stdin().read_line(&mut input).is_ok() {
        let input = input.trim().to_lowercase();
        match input.as_str() {
            "y" | "yes" => true,
            "n" | "no" => false,
            _ => default,
        }
    } else {
        default
    }
}

/// Display available providers
fn display_providers() {
    println!("\nAvailable LLM Providers:");
    println!("------------------------");
    for (i, spec) in PROVIDERS.iter().enumerate() {
        println!("  {}. {} - {}", i + 1, spec.name, spec.description);
        println!(
            "     API Key: {}, Base URL: {}",
            spec.api_key_env, spec.base_url_env
        );
    }
    println!();
}

/// Run interactive onboard wizard
pub fn run_onboard() -> Result<OnboardConfig> {
    println!("\n=== NuClaw Onboard Wizard ===\n");
    println!("This wizard will help you configure NuClaw.");
    println!(
        "Configuration will be saved to: {}\n",
        env_file_path().display()
    );

    let mut config = OnboardConfig::new();

    // Check if config already exists
    let existing = load_config().ok();
    if let Some(ref existing_config) = existing {
        if existing_config.has_any_config() {
            println!("Existing configuration found!\n");
            if existing_config.api_key.is_some() {
                println!("  - LLM API: configured");
            }
            if existing_config.telegram_token.is_some() {
                println!("  - Telegram: configured");
            }
            println!();

            if !prompt_yes_no("Overwrite existing configuration?", false) {
                println!("\nOnboard cancelled. Existing configuration kept.");
                return Ok(existing_config.clone());
            }
        }
    }

    // Step 1: Select LLM Provider
    display_providers();
    let provider_idx = prompt_with_default("Select provider number", "1");

    let idx: usize = provider_idx.parse::<usize>().unwrap_or(1).saturating_sub(1);
    if idx < PROVIDERS.len() {
        let spec = &PROVIDERS[idx];
        config.provider = Some(spec.name.to_string());
        println!("\nSelected: {} - {}", spec.name, spec.description);

        // Step 2: Get API Key
        println!("\n--- LLM Configuration ---");
        let api_key = prompt_password(&format!(
            "Enter {} API Key ({}):",
            spec.name, spec.api_key_env
        ));

        if api_key.is_empty() {
            println!("Warning: API Key is empty, skipping LLM configuration.");
        } else {
            config.api_key = Some(api_key);

            // Step 3: Get Base URL (optional)
            let default_url = match spec.name {
                "anthropic" => "https://api.anthropic.com",
                "openai" => "https://api.openai.com/v1",
                "openrouter" => "https://openrouter.ai/api/v1",
                _ => "",
            };

            let base_url = prompt_with_default(
                &format!("Enter Base URL (optional, default: {})", default_url),
                default_url,
            );

            if !base_url.is_empty() && base_url != default_url {
                config.base_url = Some(base_url);
            }
        }
    } else {
        println!("Invalid selection, using anthropic by default.");
        config.provider = Some("anthropic".to_string());
    }

    // Step 4: Telegram Configuration
    println!("\n--- Telegram Configuration ---");
    if prompt_yes_no("Configure Telegram bot?", true) {
        let token = prompt_password("Enter Telegram Bot Token (from @BotFather):");

        if !token.is_empty() {
            config.telegram_token = Some(token);
            println!("Telegram bot token configured.");
        } else {
            println!("Skipping Telegram configuration.");
        }
    } else {
        println!("Skipping Telegram configuration.");
    }

    // Save configuration
    if config.has_any_config() {
        save_config(&config)?;
    } else {
        println!("\nNo configuration provided. Nothing to save.");
    }

    println!("\n=== Onboard Complete ===\n");
    println!("✓ Configuration saved to {}", env_file_path().display());
    println!();
    println!("Run 'nuclaw --start' to start the service");
    println!("Run 'nuclaw --status' to check status");
    println!("Run 'nuclaw --stop' to stop the service");
    println!("Run 'nuclaw --restart' to restart the service");
    println!();

    Ok(config)
}

/// Save configuration to .env file
pub fn save_config(config: &OnboardConfig) -> Result<()> {
    let path = env_file_path();

    // Ensure directory exists
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| NuClawError::FileSystem {
            message: format!("Failed to create config directory: {}", e),
        })?;
    }

    let mut content = String::new();

    // Add header
    content.push_str("# NuClaw Configuration\n");
    content.push_str("# Generated by onboard wizard\n\n");

    // Add LLM config
    if let (Some(provider), Some(api_key)) = (&config.provider, &config.api_key) {
        let spec = PROVIDERS.iter().find(|s| s.name == provider);

        if let Some(spec) = spec {
            content.push_str(&format!("# {}\n", spec.description));
            content.push_str(&format!("{}={}\n", spec.api_key_env, api_key));

            if let Some(base_url) = &config.base_url {
                content.push_str(&format!("{}={}\n", spec.base_url_env, base_url));
            }

            if let Some(default_model) = spec.default_model {
                content.push_str(&format!(
                    "{}_MODEL={}\n",
                    provider.to_uppercase(),
                    default_model
                ));
            }
            content.push('\n');
        }
    }

    // Add Telegram config
    if let Some(token) = &config.telegram_token {
        content.push_str("# Telegram Bot Configuration\n");
        content.push_str(&format!("TELEGRAM_BOT_TOKEN={}\n", token));
    }

    fs::write(&path, content).map_err(|e| NuClawError::FileSystem {
        message: format!("Failed to write config file: {}", e),
    })?;

    Ok(())
}

/// Load configuration from .env file
pub fn load_config() -> Result<OnboardConfig> {
    let path = env_file_path();

    if !path.exists() {
        return Ok(OnboardConfig::new());
    }

    // Ensure parent directory exists
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            return Ok(OnboardConfig::new());
        }
    }

    let content = fs::read_to_string(&path).map_err(|e| NuClawError::FileSystem {
        message: format!("Failed to read config file: {}", e),
    })?;

    let mut config = OnboardConfig::new();

    for line in content.lines() {
        let line = line.trim();

        // Skip comments and empty lines
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if let Some((key, value)) = line.split_once('=') {
            let key = key.trim();
            let value = value.trim();

            match key {
                "ANTHROPIC_API_KEY" => {
                    config.api_key = Some(value.to_string());
                    config.provider = Some("anthropic".to_string());
                }
                "ANTHROPIC_BASE_URL" => {
                    config.base_url = Some(value.to_string());
                }
                "OPENAI_API_KEY" => {
                    config.api_key = Some(value.to_string());
                    config.provider = Some("openai".to_string());
                }
                "OPENAI_BASE_URL" => {
                    config.base_url = Some(value.to_string());
                }
                "OPENROUTER_API_KEY" => {
                    config.api_key = Some(value.to_string());
                    config.provider = Some("openrouter".to_string());
                }
                "OPENROUTER_BASE_URL" => {
                    config.base_url = Some(value.to_string());
                }
                "CUSTOM_API_KEY" => {
                    config.api_key = Some(value.to_string());
                    config.provider = Some("custom".to_string());
                }
                "CUSTOM_BASE_URL" => {
                    config.base_url = Some(value.to_string());
                }
                "TELEGRAM_BOT_TOKEN" => {
                    config.telegram_token = Some(value.to_string());
                }
                _ => {}
            }
        }
    }

    Ok(config)
}

/// Print current configuration status (without showing secrets)
pub fn print_config_status() -> Result<()> {
    let config = load_config()?;

    println!("\n=== NuClaw Configuration Status ===\n");

    // LLM Provider
    if let Some(provider) = &config.provider {
        if config.api_key.is_some() {
            println!("✓ LLM Provider: {}", provider);
            if config.base_url.is_some() {
                println!("  Base URL: configured");
            }
        } else {
            println!("✗ LLM Provider: {} (not configured)", provider);
        }
    } else {
        println!("✗ LLM Provider: not configured");
    }

    // Telegram
    if config.telegram_token.is_some() {
        println!("✓ Telegram Bot: configured");
    } else {
        println!("✗ Telegram Bot: not configured");
    }

    println!("\nConfig file: {}", env_file_path().display());
    println!();

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Mutex;

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);
    static TEST_MUTEX: Mutex<()> = Mutex::new(());

    fn setup_test_env() -> String {
        let _guard = TEST_MUTEX.lock().unwrap();
        let counter = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let test_dir = format!("/tmp/nuclaw_test_{}_{}", counter, std::process::id());
        env::remove_var("NUCLAW_HOME");
        env::set_var("NUCLAW_HOME", &test_dir);
        let _ = fs::create_dir_all(&test_dir);
        test_dir
    }

    fn cleanup_test_dir(test_dir: &str) {
        let _guard = TEST_MUTEX.lock().unwrap();
        let _ = fs::remove_dir_all(test_dir);
    }

    #[test]
    fn test_openai_provider_config() {
        let test_dir = setup_test_env();

        let content =
            "OPENAI_API_KEY=sk-openai-test\nOPENAI_BASE_URL=https://custom.openai.com/v1\n";
        fs::write(env_file_path(), content).unwrap();

        let config = load_config().unwrap();

        assert_eq!(config.provider, Some("openai".to_string()));
        assert_eq!(config.api_key, Some("sk-openai-test".to_string()));
        assert_eq!(
            config.base_url,
            Some("https://custom.openai.com/v1".to_string())
        );

        cleanup_test_dir(&test_dir);
    }

    #[test]
    fn test_onboard_config_default() {
        let config = OnboardConfig::new();
        assert!(!config.has_any_config());
        assert!(config.provider.is_none());
        assert!(config.api_key.is_none());
        assert!(config.base_url.is_none());
        assert!(config.telegram_token.is_none());
    }

    #[test]
    fn test_onboard_config_has_api_key() {
        let mut config = OnboardConfig::new();
        assert!(!config.has_any_config());

        config.api_key = Some("test-key".to_string());
        assert!(config.has_any_config());
    }

    #[test]
    fn test_onboard_config_has_telegram() {
        let mut config = OnboardConfig::new();
        assert!(!config.has_any_config());

        config.telegram_token = Some("test-token".to_string());
        assert!(config.has_any_config());
    }

    #[test]
    fn test_save_and_load_config() {
        let test_dir = setup_test_env();
        let _ = fs::create_dir_all(&test_dir);

        let config = OnboardConfig {
            provider: Some("anthropic".to_string()),
            api_key: Some("sk-test-key".to_string()),
            base_url: Some("https://api.anthropic.com".to_string()),
            telegram_token: Some("123456:ABC-DEF1234ghIkl-zyx57W2v1u123ew11".to_string()),
        };

        save_config(&config).unwrap();
        let loaded = load_config().unwrap();

        assert_eq!(loaded.provider, config.provider);
        assert_eq!(loaded.api_key, config.api_key);
        assert_eq!(loaded.base_url, config.base_url);
        assert_eq!(loaded.telegram_token, config.telegram_token);

        cleanup_test_dir(&test_dir);
    }

    #[test]
    fn test_custom_provider_config() {
        let test_dir = setup_test_env();
        let _ = fs::create_dir_all(&test_dir);

        let content =
            "CUSTOM_API_KEY=sk-custom-test\nCUSTOM_BASE_URL=https://custom.endpoint.com/v1\n";
        fs::write(env_file_path(), content).unwrap();

        let config = load_config().unwrap();

        assert_eq!(config.provider, Some("custom".to_string()));
        assert_eq!(config.api_key, Some("sk-custom-test".to_string()));
        assert_eq!(
            config.base_url,
            Some("https://custom.endpoint.com/v1".to_string())
        );

        cleanup_test_dir(&test_dir);
    }

    #[test]
    fn test_load_config_with_comments() {
        let test_dir = setup_test_env();
        let _ = fs::create_dir_all(&test_dir);

        let content = r#"
# NuClaw Configuration
# Generated by onboard wizard

# Anthropic Claude API
ANTHROPIC_API_KEY=sk-test-key-123
ANTHROPIC_BASE_URL=https://api.anthropic.com

# Telegram Bot Configuration
TELEGRAM_BOT_TOKEN=123456:ABC-DEF
"#;

        fs::write(env_file_path(), content).unwrap();

        let config = load_config().unwrap();

        assert_eq!(config.provider, Some("anthropic".to_string()));
        assert_eq!(config.api_key, Some("sk-test-key-123".to_string()));
        assert_eq!(
            config.base_url,
            Some("https://api.anthropic.com".to_string())
        );
        assert_eq!(config.telegram_token, Some("123456:ABC-DEF".to_string()));

        cleanup_test_dir(&test_dir);
    }

    #[test]
    fn test_save_config_creates_directory() {
        let test_dir = setup_test_env();
        let _ = fs::remove_dir_all(&test_dir);

        let config = OnboardConfig {
            provider: Some("anthropic".to_string()),
            api_key: Some("test-key".to_string()),
            base_url: None,
            telegram_token: None,
        };

        save_config(&config).unwrap();

        assert!(env_file_path().exists());

        cleanup_test_dir(&test_dir);
    }

    #[test]
    fn test_load_config_partial() {
        let test_dir = setup_test_env();
        let _ = fs::create_dir_all(&test_dir);

        let content = "ANTHROPIC_API_KEY=sk-test-only\n";
        fs::write(env_file_path(), content).unwrap();

        let config = load_config().unwrap();

        assert_eq!(config.provider, Some("anthropic".to_string()));
        assert_eq!(config.api_key, Some("sk-test-only".to_string()));
        assert!(config.base_url.is_none());
        assert!(config.telegram_token.is_none());

        cleanup_test_dir(&test_dir);
    }

    #[test]
    fn test_save_config_overwrites() {
        let test_dir = setup_test_env();
        let _ = fs::remove_dir_all(&test_dir);

        let config1 = OnboardConfig {
            provider: Some("anthropic".to_string()),
            api_key: Some("key1".to_string()),
            base_url: None,
            telegram_token: None,
        };
        save_config(&config1).unwrap();

        let config2 = OnboardConfig {
            provider: Some("openai".to_string()),
            api_key: Some("key2".to_string()),
            base_url: None,
            telegram_token: Some("tele-token".to_string()),
        };
        save_config(&config2).unwrap();

        let loaded = load_config().unwrap();

        assert_eq!(loaded.provider, Some("openai".to_string()));
        assert_eq!(loaded.api_key, Some("key2".to_string()));
        assert_eq!(loaded.telegram_token, Some("tele-token".to_string()));

        cleanup_test_dir(&test_dir);
    }
}
