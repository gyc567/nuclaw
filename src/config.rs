//! Configuration for NuClaw

use std::env;
use std::path::PathBuf;

pub fn project_root() -> PathBuf {
    env::current_dir().expect("Failed to get current directory")
}

pub fn store_dir() -> PathBuf {
    project_root().join("store")
}

pub fn groups_dir() -> PathBuf {
    project_root().join("groups")
}

pub fn data_dir() -> PathBuf {
    project_root().join("data")
}

pub fn logs_dir() -> PathBuf {
    groups_dir().join("logs")
}

pub fn mount_allowlist_path() -> PathBuf {
    let home = home::home_dir().unwrap_or_else(|| PathBuf::from("/Users/user"));
    home.join(".config")
        .join("nuclaw")
        .join("mount-allowlist.json")
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
