//! Message parser for skill installation requests

use regex::Regex;
use crate::skill_installer::error::{InstallError, Result};

/// Trigger words that indicate an install request (multi-language)
const TRIGGER_WORDS: &[&str] = &[
    "安装",
    "安装技能",
    "add skill",
    "install skill",
    "添加技能",
    "添加 skill",
    "下载技能",
    "下载 skill",
    "clone skill",
];

/// Installation request parsed from user message
#[derive(Debug, Clone)]
pub struct InstallRequest {
    /// GitHub repository URL
    pub source_url: String,
    /// Owner of the repository
    pub owner: String,
    /// Repository name
    pub repo: String,
    /// User-specified skill name (optional)
    pub target_name: Option<String>,
    /// Force overwrite if exists
    pub force: bool,
}

impl InstallRequest {
    /// Get the skill name (user-specified or repo name)
    pub fn skill_name(&self) -> &str {
        self.target_name.as_deref().unwrap_or(&self.repo)
    }
}

/// Parse a user message to extract installation request
pub fn parse_install_request(message: &str) -> Option<InstallRequest> {
    let message_lower = message.to_lowercase();

    // 1. Check for trigger words
    let has_trigger = TRIGGER_WORDS.iter().any(|trigger| {
        message_lower.contains(&trigger.to_lowercase())
    });

    if !has_trigger {
        return None;
    }

    // 2. Extract GitHub URL
    let github_url = extract_github_url(message)?;

    // 3. Parse URL to get owner and repo
    let (owner, repo) = parse_github_url(&github_url)?;

    // 4. Check for --force flag
    let force = message.contains("--force") || message.contains("-f");

    // 5. Try to extract user-specified name
    let target_name = extract_target_name(message);

    Some(InstallRequest {
        source_url: github_url,
        owner,
        repo,
        target_name,
        force,
    })
}

/// Extract GitHub URL from text
fn extract_github_url(text: &str) -> Option<String> {
    // Match GitHub URLs: https://github.com/owner/repo or https://github.com/owner/repo/
    let regex = Regex::new(
        r"https?://github\.com/([a-zA-Z0-9_-]+)/([a-zA-Z0-9_-]+)(?:\.git)?/?"
    ).ok()?;

    let caps = regex.captures(text)?;

    let owner = caps.get(1)?.as_str();
    let repo = caps.get(2)?.as_str();

    // Remove .git suffix if present
    let repo = repo.trim_end_matches(".git");

    Some(format!("https://github.com/{}/{}", owner, repo))
}

/// Parse GitHub URL to extract owner and repo name
fn parse_github_url(url: &str) -> Option<(String, String)> {
    let regex = Regex::new(
        r"https?://github\.com/([a-zA-Z0-9_-]+)/([a-zA-Z0-9_-]+)"
    ).ok()?;

    let caps = regex.captures(url)?;

    let owner = caps.get(1)?.as_str().to_string();
    let mut repo = caps.get(2)?.as_str().to_string();

    // Remove .git suffix
    if repo.ends_with(".git") {
        repo = repo.trim_end_matches(".git").to_string();
    }

    Some((owner, repo))
}

/// Extract user-specified skill name from message
fn extract_target_name(message: &str) -> Option<String> {
    // Look for patterns like: "as my-skill" or "--name my-skill" or "-n my-skill"
    let name_regex = Regex::new(r"(?i)(?:-n|--name|as)\s+([a-zA-Z0-9_-]+)").ok()?;
    let caps = name_regex.captures(message)?;
    caps.get(1).map(|m| m.as_str().to_string())
}

/// Validate skill name format
pub fn validate_skill_name(name: &str) -> std::result::Result<(), InstallError> {
    // Check empty
    if name.is_empty() {
        return Err(InstallError::InvalidName("Name cannot be empty".to_string()));
    }

    // Check length
    if name.len() > 64 {
        return Err(InstallError::InvalidName("Name too long (max 64 chars)".to_string()));
    }

    // Check valid characters (lowercase, digits, hyphens)
    if !name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-') {
        return Err(InstallError::InvalidName(
            "Name can only contain lowercase letters, digits, and hyphens".to_string()
        ));
    }

    // Check start/end
    if name.starts_with('-') || name.ends_with('-') {
        return Err(InstallError::InvalidName("Name cannot start or end with hyphen".to_string()));
    }

    // Check for double hyphens
    if name.contains("--") {
        return Err(InstallError::InvalidName("Name cannot contain double hyphens".to_string()));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_github_url() {
        // Note: The function only matches URLs that start with http:// or https://
        let cases = vec![
            ("https://github.com/owner/repo", Some("https://github.com/owner/repo".to_string())),
            ("https://github.com/owner/repo/", Some("https://github.com/owner/repo".to_string())),
            ("https://github.com/owner/repo.git", Some("https://github.com/owner/repo".to_string())),
            ("install https://github.com/openclaw/skills/tree/main/skills/24601/agent-deep-research", Some("https://github.com/openclaw/skills".to_string())),
            // "install from github.com/test/repo" doesn't match because it doesn't start with http://
            ("not a github url", None),
        ];

        for (input, expected) in cases {
            let result = extract_github_url(input);
            assert_eq!(result, expected, "Failed for: {}", input);
        }
    }

    #[test]
    fn test_parse_github_url() {
        let cases = vec![
            ("https://github.com/owner/repo", Some(("owner".to_string(), "repo".to_string()))),
            ("https://github.com/test-org/test-repo.git", Some(("test-org".to_string(), "test-repo".to_string()))),
        ];

        for (input, expected) in cases {
            let result = parse_github_url(input);
            assert_eq!(result, expected, "Failed for: {}", input);
        }
    }

    #[test]
    fn test_parse_install_request() {
        // Test basic install - note: trigger must contain one of TRIGGER_WORDS
        let msg = "install skill https://github.com/owner/repo";
        let result = parse_install_request(msg);
        assert!(result.is_some(), "Failed for: {}", msg);
        let req = result.unwrap();
        assert_eq!(req.owner, "owner");
        assert_eq!(req.repo, "repo");
        assert!(!req.force);

        // Test with Chinese trigger
        let msg = "安装 https://github.com/owner/repo";
        let result = parse_install_request(msg);
        assert!(result.is_some());

        // Test with --force
        let msg = "安装技能 https://github.com/owner/repo --force";
        let result = parse_install_request(msg);
        assert!(result.is_some());
        assert!(result.unwrap().force);

        // Test with name
        let msg = "install skill https://github.com/owner/repo -n my-skill";
        let result = parse_install_request(msg);
        assert!(result.is_some());
        assert_eq!(result.unwrap().target_name, Some("my-skill".to_string()));

        // Test without trigger - should be None
        let msg = "https://github.com/owner/repo";
        let result = parse_install_request(msg);
        assert!(result.is_none());
    }

    #[test]
    fn test_validate_skill_name() {
        // Valid names
        assert!(validate_skill_name("my-skill").is_ok());
        assert!(validate_skill_name("github").is_ok());
        assert!(validate_skill_name("test123").is_ok());

        // Invalid names
        assert!(validate_skill_name("").is_err());
        assert!(validate_skill_name("-start").is_err());
        assert!(validate_skill_name("end-").is_err());
        assert!(validate_skill_name("double--hyphen").is_err());
        assert!(validate_skill_name("UPPERCASE").is_err());
        assert!(validate_skill_name("with_underscore").is_err());
    }
}
