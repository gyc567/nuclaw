//! Configuration for NuClaw

use std::env;
use std::path::PathBuf;

/// Get NuClaw home directory, defaulting to ~/.nuclaw/
pub fn nuclaw_home() -> PathBuf {
    env::var("NUCLAW_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            home::home_dir()
                .unwrap_or_else(|| PathBuf::from("/Users/user"))
                .join(".nuclaw")
        })
}

pub fn project_root() -> PathBuf {
    env::current_dir().expect("Failed to get current directory")
}

/// Storage directory for database and credentials
pub fn store_dir() -> PathBuf {
    nuclaw_home().join("store")
}

/// Groups directory for group-specific CLAUDE.md files
pub fn groups_dir() -> PathBuf {
    nuclaw_home().join("groups")
}

/// Runtime data directory (sessions, IPC)
pub fn data_dir() -> PathBuf {
    nuclaw_home().join("data")
}

/// Logs directory
pub fn logs_dir() -> PathBuf {
    groups_dir().join("logs")
}

/// Mount allowlist configuration path
pub fn mount_allowlist_path() -> PathBuf {
    nuclaw_home().join("mount-allowlist.json")
}

/// Main configuration file path
pub fn config_path() -> PathBuf {
    nuclaw_home().join("config.json")
}

pub fn assistant_name() -> String {
    env::var("ASSISTANT_NAME").unwrap_or_else(|_| "Andy".to_string())
}

pub fn anthropic_api_key() -> Option<String> {
    env::var("ANTHROPIC_API_KEY").ok()
}

pub fn anthropic_base_url() -> Option<String> {
    env::var("ANTHROPIC_BASE_URL").ok()
}

pub fn claude_model() -> Option<String> {
    env::var("CLAUDE_MODEL").ok()
}

pub fn timezone() -> String {
    env::var("TZ").unwrap_or_else(|_| "UTC".to_string())
}

pub fn ensure_directories() -> std::io::Result<()> {
    let dirs = [
        store_dir(),
        groups_dir(),
        data_dir(),
        mount_allowlist_path().parent().unwrap().to_path_buf(),
    ];
    for dir in dirs {
        if !dir.exists() {
            std::fs::create_dir_all(&dir)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nuclaw_home_default() {
        std::env::remove_var("NUCLAW_HOME");
        let home = nuclaw_home();
        assert!(home.to_string_lossy().contains(".nuclaw"));
    }

    #[test]
    fn test_nuclaw_home_from_env() {
        std::env::remove_var("NUCLAW_HOME");
        std::env::set_var("NUCLAW_HOME", "/custom/path");
        assert_eq!(nuclaw_home(), PathBuf::from("/custom/path"));
        std::env::remove_var("NUCLAW_HOME");
    }

    #[test]
    fn test_store_dir_uses_nuclaw_home() {
        std::env::remove_var("NUCLAW_HOME");
        std::env::set_var("NUCLAW_HOME", "/test/nuclaw");
        assert_eq!(store_dir(), PathBuf::from("/test/nuclaw/store"));
        std::env::remove_var("NUCLAW_HOME");
    }

    #[test]
    fn test_anthropic_api_key_from_env() {
        std::env::remove_var("ANTHROPIC_API_KEY");
        assert!(anthropic_api_key().is_none());

        std::env::set_var("ANTHROPIC_API_KEY", "test-key-123");
        assert_eq!(anthropic_api_key(), Some("test-key-123".to_string()));

        std::env::remove_var("ANTHROPIC_API_KEY");
    }

    #[test]
    fn test_anthropic_base_url_from_env() {
        std::env::remove_var("ANTHROPIC_BASE_URL");
        assert!(anthropic_base_url().is_none());

        std::env::set_var("ANTHROPIC_BASE_URL", "https://api.anthropic.com");
        assert_eq!(
            anthropic_base_url(),
            Some("https://api.anthropic.com".to_string())
        );

        std::env::remove_var("ANTHROPIC_BASE_URL");
    }

    #[test]
    fn test_anthropic_base_url_custom_endpoint() {
        std::env::remove_var("ANTHROPIC_BASE_URL");

        std::env::set_var("ANTHROPIC_BASE_URL", "https://custom.endpoint.com/v1");
        assert_eq!(
            anthropic_base_url(),
            Some("https://custom.endpoint.com/v1".to_string())
        );

        std::env::remove_var("ANTHROPIC_BASE_URL");
    }

    #[test]
    fn test_claude_model_from_env() {
        std::env::remove_var("CLAUDE_MODEL");
        assert!(claude_model().is_none());

        std::env::set_var("CLAUDE_MODEL", "MiniMax-M2.5");
        assert_eq!(claude_model(), Some("MiniMax-M2.5".to_string()));

        std::env::remove_var("CLAUDE_MODEL");
    }
}
