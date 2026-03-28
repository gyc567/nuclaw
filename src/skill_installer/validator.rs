//! Skill validator with tool whitelist

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::skill_installer::error::{InstallError, Result};

/// Allowed tools for external skills (whitelist)
/// These tools are considered safe for user-installed skills
const ALLOWED_TOOLS: &[&str] = &[
    "read",
    "glob",
    "grep",
    "webfetch",
    "websearch",
    "look_at",
    "lsp_document_symbols",
    "lsp_workspace_symbols",
    "lsp_goto_definition",
    "lsp_find_references",
    "lsp_hover",
];

/// Dangerous tools that are NOT allowed for external skills
const FORBIDDEN_TOOLS: &[&str] = &["bash", "write", "edit", "delete", "mcp", "run", "execute"];

/// Validated skill with approved tools
#[derive(Debug, Clone)]
pub struct ValidatedSkill {
    /// Skill name
    pub name: String,
    /// Skill description
    pub description: String,
    /// Allowed tools (filtered by whitelist)
    pub allowed_tools: Vec<String>,
    /// Skill path
    pub path: PathBuf,
    /// Whether this is a tool skill
    pub is_tool_skill: bool,
}

/// Skill validator configuration
#[derive(Debug, Clone)]
pub struct ValidatorConfig {
    /// Tool whitelist
    pub allowed_tools: Vec<String>,
    /// Maximum skill size in MB
    pub max_size_mb: u32,
    /// Allow tool skills
    pub allow_tool_skills: bool,
}

impl Default for ValidatorConfig {
    fn default() -> Self {
        Self {
            allowed_tools: ALLOWED_TOOLS.iter().map(|s| s.to_string()).collect(),
            max_size_mb: 50,
            allow_tool_skills: true,
        }
    }
}

/// Skill validator
pub struct SkillValidator {
    config: ValidatorConfig,
}

impl SkillValidator {
    /// Create a new validator
    pub fn new(config: ValidatorConfig) -> Self {
        Self { config }
    }

    /// Create with default config
    pub fn with_defaults() -> Self {
        Self::new(ValidatorConfig::default())
    }

    /// Validate a skill directory
    pub fn validate(&self, skill_dir: &Path) -> Result<ValidatedSkill> {
        // Check directory exists
        if !skill_dir.is_dir() {
            return Err(InstallError::InvalidSkill(
                "Skill directory not found".to_string(),
            ));
        }

        // Read SKILL.md
        let skill_md_path = skill_dir.join("SKILL.md");
        if !skill_md_path.exists() {
            return Err(InstallError::InvalidSkill("SKILL.md not found".to_string()));
        }

        let content = std::fs::read_to_string(&skill_md_path)?;

        // Parse frontmatter
        let (frontmatter, _body) = parse_frontmatter(&content)
            .ok_or_else(|| InstallError::InvalidSkill("Invalid SKILL.md format".to_string()))?;

        // Validate name
        let name = frontmatter.name;
        if name.is_empty() {
            return Err(InstallError::InvalidSkill(
                "Skill name is empty".to_string(),
            ));
        }

        // Validate tool permissions
        let allowed_tools = self.filter_allowed_tools(&frontmatter.allowed_tools)?;

        // Check if it's a tool skill
        let is_tool_skill = frontmatter.skill_type == "tool";

        if is_tool_skill && !self.config.allow_tool_skills {
            return Err(InstallError::InvalidSkill(
                "Tool skills are not allowed".to_string(),
            ));
        }

        tracing::info!(
            "Validated skill '{}': {} tools allowed, is_tool_skill={}",
            name,
            allowed_tools.len(),
            is_tool_skill
        );

        Ok(ValidatedSkill {
            name,
            description: frontmatter.description,
            allowed_tools,
            path: skill_dir.to_path_buf(),
            is_tool_skill,
        })
    }

    /// Filter tools against whitelist
    fn filter_allowed_tools(&self, requested_tools: &Option<String>) -> Result<Vec<String>> {
        let Some(tools_str) = requested_tools else {
            return Ok(Vec::new());
        };

        let mut allowed = Vec::new();
        let requested: HashSet<&str> = tools_str.split_whitespace().collect();

        for tool in &requested {
            // Check if tool is forbidden
            if FORBIDDEN_TOOLS.contains(tool) {
                tracing::warn!("Tool '{}' is forbidden for external skills, skipping", tool);
                continue;
            }

            // Check if tool is in whitelist
            if self.config.allowed_tools.contains(&tool.to_string()) {
                allowed.push(tool.to_string());
            } else {
                tracing::warn!("Tool '{}' not in whitelist, skipping", tool);
            }
        }

        Ok(allowed)
    }

    /// Check if a tool is allowed
    pub fn is_tool_allowed(&self, tool: &str) -> bool {
        self.config.allowed_tools.contains(&tool.to_string())
    }
}

/// Parse frontmatter from SKILL.md
fn parse_frontmatter(content: &str) -> Option<(Frontmatter, String)> {
    if !content.starts_with("---") {
        return None;
    }

    let end_idx = content[3..].find("---")?;
    let frontmatter_str = &content[3..3 + end_idx];
    let body = content[3 + end_idx + 3..].trim_start().to_string();

    let frontmatter: Frontmatter = serde_yaml::from_str(frontmatter_str).ok()?;

    Some((frontmatter, body))
}

/// Frontmatter structure
#[derive(Debug, serde::Deserialize)]
struct Frontmatter {
    #[serde(default)]
    name: String,
    #[serde(default)]
    description: String,
    #[serde(rename = "skill-type", default)]
    skill_type: String,
    #[serde(rename = "allowed-tools", default)]
    allowed_tools: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_validator_config_default() {
        let config = ValidatorConfig::default();
        assert!(config.allowed_tools.contains(&"read".to_string()));
        // bash is forbidden for security in external skills
        assert!(!config.allowed_tools.contains(&"bash".to_string()));
    }

    #[test]
    fn test_is_tool_allowed() {
        let validator = SkillValidator::with_defaults();
        assert!(validator.is_tool_allowed("read"));
        // bash is forbidden for security reasons
        assert!(!validator.is_tool_allowed("bash"));
        assert!(!validator.is_tool_allowed("unknown_tool"));
    }

    #[test]
    fn test_validate_skill() {
        let temp_dir = TempDir::new().unwrap();
        let skill_dir = temp_dir.path();

        let content = r#"---
name: test-skill
description: A test skill
allowed-tools: read glob
---

# Test Skill
"#;

        fs::write(skill_dir.join("SKILL.md"), content).unwrap();

        let validator = SkillValidator::with_defaults();
        let result = validator.validate(skill_dir);

        assert!(result.is_ok());
        let validated = result.unwrap();
        assert_eq!(validated.name, "test-skill");
        assert!(validated.allowed_tools.contains(&"read".to_string()));
    }

    #[test]
    fn test_forbidden_tools_filtered() {
        let temp_dir = TempDir::new().unwrap();
        let skill_dir = temp_dir.path();

        let content = r#"---
name: dangerous-skill
description: A dangerous skill
allowed-tools: read bash write
---

# Dangerous Skill
"#;

        fs::write(skill_dir.join("SKILL.md"), content).unwrap();

        let validator = SkillValidator::with_defaults();
        let result = validator.validate(skill_dir);

        assert!(result.is_ok());
        let validated = result.unwrap();
        // bash and write should be filtered out
        assert!(validated.allowed_tools.contains(&"read".to_string()));
        assert!(!validated.allowed_tools.contains(&"bash".to_string()));
        assert!(!validated.allowed_tools.contains(&"write".to_string()));
    }
}
