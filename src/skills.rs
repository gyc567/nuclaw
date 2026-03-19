use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::config::skills_dir;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SkillMetadata {
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(flatten)]
    pub extra: HashMap<String, String>,
}

/// Skill execution type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SkillType {
    /// Pure text prompt (existing behavior)
    #[default]
    Text,
    /// Executable tool with external bindings
    Tool,
    /// WASM module (Phase 3)
    Wasm,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Skill {
    pub name: String,
    pub description: String,
    pub license: Option<String>,
    pub compatibility: Option<String>,
    pub metadata: SkillMetadata,
    pub allowed_tools: Option<String>,
    pub content: String,
    pub path: Option<PathBuf>,
    /// Skill execution type
    #[serde(default)]
    pub skill_type: SkillType,
    /// Tools required for Tool type skills
    #[serde(default)]
    pub tools: Vec<String>,
    /// Configuration for skill execution
    #[serde(default)]
    pub config: HashMap<String, serde_json::Value>,
}

impl Skill {
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        content: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            license: None,
            compatibility: None,
            metadata: SkillMetadata::default(),
            allowed_tools: None,
            content: content.into(),
            path: None,
            skill_type: SkillType::default(),
            tools: Vec::new(),
            config: HashMap::new(),
        }
    }

    /// Create a skill from content string (for testing)
    pub fn from_content(name: &str, content: &str) -> Self {
        let (frontmatter, body) = parse_frontmatter(content).unwrap_or((
            Frontmatter {
                name: name.to_string(),
                description: String::new(),
                ..Default::default()
            },
            content.to_string(),
        ));

        Self {
            name: frontmatter.name,
            description: frontmatter.description,
            license: frontmatter.license,
            compatibility: frontmatter.compatibility,
            metadata: frontmatter.metadata,
            allowed_tools: frontmatter.allowed_tools,
            content: body,
            path: None,
            skill_type: frontmatter.skill_type,
            tools: frontmatter.tools,
            config: frontmatter.config,
        }
    }

    pub fn from_directory(dir: &Path) -> Option<Self> {
        let skill_md = dir.join("SKILL.md");
        if !skill_md.exists() {
            return None;
        }

        let content = fs::read_to_string(&skill_md).ok()?;
        let dir_name = dir.file_name()?.to_str()?.to_string();

        let (frontmatter, body) = parse_frontmatter(&content)?;

        let name = if frontmatter.name.is_empty() {
            dir_name.clone()
        } else {
            frontmatter.name
        };

        Some(Self {
            name,
            description: frontmatter.description,
            license: frontmatter.license,
            compatibility: frontmatter.compatibility,
            metadata: frontmatter.metadata,
            allowed_tools: frontmatter.allowed_tools,
            content: body,
            path: Some(dir.to_path_buf()),
            skill_type: frontmatter.skill_type,
            tools: frontmatter.tools,
            config: frontmatter.config,
        })
    }

    pub fn skill_dir(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    pub fn scripts_dir(&self) -> Option<PathBuf> {
        self.path.as_ref().map(|p| p.join("scripts"))
    }

    pub fn references_dir(&self) -> Option<PathBuf> {
        self.path.as_ref().map(|p| p.join("references"))
    }

    pub fn assets_dir(&self) -> Option<PathBuf> {
        self.path.as_ref().map(|p| p.join("assets"))
    }

    pub fn has_scripts(&self) -> bool {
        self.scripts_dir().map(|p| p.exists()).unwrap_or(false)
    }

    pub fn has_references(&self) -> bool {
        self.references_dir().map(|p| p.exists()).unwrap_or(false)
    }

    pub fn has_assets(&self) -> bool {
        self.assets_dir().map(|p| p.exists()).unwrap_or(false)
    }

    /// Check if this is a tool skill
    pub fn is_tool_skill(&self) -> bool {
        self.skill_type == SkillType::Tool
    }

    pub fn validate(&self) -> Vec<SkillValidationError> {
        let mut errors = Vec::new();

        if self.name.is_empty() {
            errors.push(SkillValidationError::NameEmpty);
        }
        if self.name.len() > 64 {
            errors.push(SkillValidationError::NameTooLong);
        }
        if !is_valid_name(&self.name) {
            errors.push(SkillValidationError::NameInvalidFormat);
        }
        if self.description.is_empty() {
            errors.push(SkillValidationError::DescriptionEmpty);
        }
        if self.description.len() > 1024 {
            errors.push(SkillValidationError::DescriptionTooLong);
        }
        if let Some(ref compat) = self.compatibility {
            if compat.len() > 500 {
                errors.push(SkillValidationError::CompatibilityTooLong);
            }
        }

        errors
    }

    pub fn is_valid(&self) -> bool {
        self.validate().is_empty()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum SkillValidationError {
    NameEmpty,
    NameTooLong,
    NameInvalidFormat,
    DescriptionEmpty,
    DescriptionTooLong,
    CompatibilityTooLong,
}

impl std::fmt::Display for SkillValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SkillValidationError::NameEmpty => write!(f, "name cannot be empty"),
            SkillValidationError::NameTooLong => write!(f, "name exceeds 64 characters"),
            SkillValidationError::NameInvalidFormat => {
                write!(f, "name contains invalid characters")
            }
            SkillValidationError::DescriptionEmpty => write!(f, "description cannot be empty"),
            SkillValidationError::DescriptionTooLong => {
                write!(f, "description exceeds 1024 characters")
            }
            SkillValidationError::CompatibilityTooLong => {
                write!(f, "compatibility exceeds 500 characters")
            }
        }
    }
}

#[derive(Debug, Default, Deserialize)]
struct Frontmatter {
    #[serde(default)]
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    license: Option<String>,
    #[serde(default)]
    compatibility: Option<String>,
    #[serde(default)]
    metadata: SkillMetadata,
    #[serde(rename = "allowed-tools", default)]
    allowed_tools: Option<String>,
    #[serde(rename = "skill-type", default)]
    skill_type: SkillType,
    #[serde(default)]
    tools: Vec<String>,
    #[serde(default)]
    config: HashMap<String, serde_json::Value>,
}

fn parse_frontmatter(content: &str) -> Option<(Frontmatter, String)> {
    if !content.starts_with("---") {
        let lines: Vec<&str> = content.lines().collect();
        let description = lines.first().unwrap_or(&"").to_string();
        return Some((
            Frontmatter {
                description,
                ..Default::default()
            },
            content.to_string(),
        ));
    }

    let end_idx = content[3..].find("---")?;
    let frontmatter_str = &content[3..3 + end_idx];
    let body = content[3 + end_idx + 3..].trim_start().to_string();

    let frontmatter: Frontmatter = serde_yaml::from_str(frontmatter_str).ok()?;

    Some((frontmatter, body))
}

fn is_valid_name(name: &str) -> bool {
    if name.is_empty() || name.len() > 64 {
        return false;
    }
    if name.starts_with('-') || name.ends_with('-') {
        return false;
    }
    if name.contains("--") {
        return false;
    }
    name.chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

pub trait SkillRegistry: Send + Sync {
    fn get(&self, name: &str) -> Option<Arc<Skill>>;
    fn list(&self) -> Vec<Arc<Skill>>;
    fn names(&self) -> Vec<String>;
}

#[derive(Default)]
pub struct BuiltinSkillRegistry {
    skills: HashMap<String, Arc<Skill>>,
}

impl BuiltinSkillRegistry {
    pub fn new() -> Self {
        let mut registry = Self::default();
        registry.register_builtin_skills();
        registry.load_external_skills();
        registry
    }

    fn register_builtin_skills(&mut self) {
        self.register(Skill::new(
            "github",
            "Manage GitHub repositories, issues, and pull requests. Use when working with GitHub.",
            r#"# GitHub Skill

You are a GitHub assistant. Use the `gh` CLI to help users with:
- Creating and managing repositories
- Working with issues and pull requests
- Searching code
- Managing branches
- Getting repository information

When asked to perform GitHub actions, use appropriate GitHub CLI commands."#,
        ));

        self.register(Skill::new(
            "web-search",
            "Search the web for information. Use when user asks to search, find, or look up something online.",
            r#"# Web Search Skill

You are a web search assistant. Use search tools to:
- Find information on the web
- Research topics
- Get current news
- Fact check information

Use web search when you need current or external information."#,
        ));

        self.register(Skill::new(
            "weather",
            "Get weather information for locations. Use when user asks about weather, temperature, or forecasts.",
            r#"# Weather Skill

You are a weather assistant. Help users with:
- Current weather conditions
- Weather forecasts
- Temperature, humidity, and wind information

Use available weather data to provide accurate information."#,
        ));

        self.register(Skill::new(
            "memory",
            "Persistent memory and context management. Use to remember important information across sessions.",
            r#"# Memory Skill

You are a memory assistant. Help users with:
- Remembering important information
- Retrieving past conversations
- Managing context
- Storing preferences

Use the memory system to persist and retrieve information."#,
        ));
    }

    fn load_external_skills(&mut self) {
        let skills_path = skills_dir();
        if !skills_path.exists() {
            return;
        }

        if let Ok(entries) = fs::read_dir(&skills_path) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    if let Some(skill) = Skill::from_directory(&path) {
                        if !self.skills.contains_key(&skill.name) {
                            self.skills.insert(skill.name.clone(), Arc::new(skill));
                        }
                    }
                }
            }
        }
    }

    pub fn register(&mut self, skill: Skill) {
        self.skills.insert(skill.name.clone(), Arc::new(skill));
    }

    pub fn get_skill(&self, name: &str) -> Option<Arc<Skill>> {
        self.skills.get(name).cloned()
    }

    pub fn validate_all(&self) -> HashMap<String, Vec<SkillValidationError>> {
        let mut errors = HashMap::new();
        for (name, skill) in &self.skills {
            let errs = skill.validate();
            if !errs.is_empty() {
                errors.insert(name.clone(), errs);
            }
        }
        errors
    }
}

impl SkillRegistry for BuiltinSkillRegistry {
    fn get(&self, name: &str) -> Option<Arc<Skill>> {
        self.skills.get(name).cloned()
    }

    fn list(&self) -> Vec<Arc<Skill>> {
        self.skills.values().cloned().collect()
    }

    fn names(&self) -> Vec<String> {
        self.skills.keys().cloned().collect()
    }
}

pub fn builtin_skills() -> BuiltinSkillRegistry {
    BuiltinSkillRegistry::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_valid_name() {
        assert!(is_valid_name("pdf-processing"));
        assert!(is_valid_name("data-analysis"));
        assert!(is_valid_name("code-review"));
        assert!(is_valid_name("test123"));

        assert!(!is_valid_name("PDF-Processing"));
        assert!(!is_valid_name("-pdf"));
        assert!(!is_valid_name("pdf-"));
        assert!(!is_valid_name("pdf--processing"));
        assert!(!is_valid_name(""));
    }

    #[test]
    fn test_builtin_skill_registry_has_skills() {
        let registry = BuiltinSkillRegistry::new();
        assert!(!registry.names().is_empty());
    }

    #[test]
    fn test_get_skill_github() {
        let registry = BuiltinSkillRegistry::new();
        let skill = registry.get("github");
        assert!(skill.is_some());
        let skill = skill.unwrap();
        assert_eq!(skill.name, "github");
        assert!(skill.description.contains("GitHub"));
    }

    #[test]
    fn test_get_skill_weather() {
        let registry = BuiltinSkillRegistry::new();
        let skill = registry.get("weather");
        assert!(skill.is_some());
    }

    #[test]
    fn test_get_skill_nonexistent() {
        let registry = BuiltinSkillRegistry::new();
        let skill = registry.get("nonexistent");
        assert!(skill.is_none());
    }

    #[test]
    fn test_list_skills() {
        let registry = BuiltinSkillRegistry::new();
        let skills = registry.list();
        assert!(skills.len() >= 4);
    }

    #[test]
    fn test_names() {
        let registry = BuiltinSkillRegistry::new();
        let names = registry.names();
        assert!(names.contains(&"github".to_string()));
        assert!(names.contains(&"web-search".to_string()));
        assert!(names.contains(&"weather".to_string()));
        assert!(names.contains(&"memory".to_string()));
    }

    #[test]
    fn test_register_custom_skill() {
        let mut registry = BuiltinSkillRegistry::new();
        let custom = Skill::new("custom", "A custom skill", "Custom content");
        registry.register(custom);

        let skill = registry.get("custom");
        assert!(skill.is_some());
        assert_eq!(skill.unwrap().name, "custom");
    }

    #[test]
    fn test_skill_content() {
        let registry = BuiltinSkillRegistry::new();
        let skill = registry.get("github").unwrap();
        assert!(skill.content.contains("GitHub"));
    }

    #[test]
    fn test_builtin_skills_function() {
        let registry = builtin_skills();
        assert!(!registry.names().is_empty());
    }

    #[test]
    fn test_skill_is_arc() {
        let registry = BuiltinSkillRegistry::new();
        let skill1 = registry.get("github").unwrap();
        let skill2 = registry.get("github").unwrap();
        assert!(Arc::ptr_eq(&skill1, &skill2));
    }

    #[test]
    fn test_parse_frontmatter_full() {
        let content = r#"---
name: test-skill
description: A test skill
license: Apache-2.0
compatibility: Requires Python 3.8+
metadata:
  author: test
  version: "1.0"
allowed-tools: Bash Read
---

# Test Skill

This is the body content."#;
        let (fm, body) = parse_frontmatter(content).unwrap();
        assert_eq!(fm.name, "test-skill");
        assert_eq!(fm.description, "A test skill");
        assert_eq!(fm.license, Some("Apache-2.0".to_string()));
        assert_eq!(fm.compatibility, Some("Requires Python 3.8+".to_string()));
        assert!(body.contains("Test Skill"));
    }

    #[test]
    fn test_parse_frontmatter_minimal() {
        let content = r#"---
name: minimal-skill
description: A minimal skill
---

# Body"#;
        let (fm, body) = parse_frontmatter(content).unwrap();
        assert_eq!(fm.name, "minimal-skill");
        assert_eq!(fm.description, "A minimal skill");
        assert!(body.contains("Body"));
    }

    #[test]
    fn test_parse_frontmatter_no_frontmatter() {
        let content = "# Just a heading\n\nSome content";
        let (fm, body) = parse_frontmatter(content).unwrap();
        assert!(fm.description.contains("Just a heading"));
        assert!(body.contains("Some content"));
    }

    #[test]
    fn test_skill_validate_valid() {
        let skill = Skill::new("valid-skill", "A valid skill description", "Content");
        assert!(skill.is_valid());
        assert!(skill.validate().is_empty());
    }

    #[test]
    fn test_skill_validate_empty_name() {
        let mut skill = Skill::new("", "A valid description", "Content");
        skill.path = Some(PathBuf::from("test"));
        assert!(!skill.is_valid());
        let errors = skill.validate();
        assert!(errors.contains(&SkillValidationError::NameEmpty));
    }

    #[test]
    fn test_skill_validate_name_too_long() {
        let long_name = "a".repeat(65);
        let mut skill = Skill::new(long_name, "Description", "Content");
        skill.path = Some(PathBuf::from("test"));
        assert!(!skill.is_valid());
        let errors = skill.validate();
        assert!(errors.contains(&SkillValidationError::NameTooLong));
    }

    #[test]
    fn test_skill_validate_description_too_long() {
        let long_desc = "a".repeat(1025);
        let skill = Skill::new("valid", long_desc, "Content");
        assert!(!skill.is_valid());
        let errors = skill.validate();
        assert!(errors.contains(&SkillValidationError::DescriptionTooLong));
    }

    #[test]
    fn test_skill_validate_invalid_name_format() {
        let mut skill = Skill::new("Invalid-Name", "Valid description", "Content");
        skill.path = Some(PathBuf::from("Invalid-Name"));
        assert!(!skill.is_valid());
        let errors = skill.validate();
        assert!(errors.contains(&SkillValidationError::NameInvalidFormat));
    }

    #[test]
    fn test_validate_all() {
        let registry = BuiltinSkillRegistry::new();
        let errors = registry.validate_all();
        assert!(errors.is_empty(), "All built-in skills should be valid");
    }

    #[test]
    fn test_skill_with_path() {
        let mut skill = Skill::new("test", "Test skill", "Content");
        skill.path = Some(PathBuf::from("/tmp/test-skill"));

        assert_eq!(skill.skill_dir(), Some(Path::new("/tmp/test-skill")));
        assert_eq!(
            skill.scripts_dir(),
            Some(PathBuf::from("/tmp/test-skill/scripts"))
        );
        assert_eq!(
            skill.references_dir(),
            Some(PathBuf::from("/tmp/test-skill/references"))
        );
        assert_eq!(
            skill.assets_dir(),
            Some(PathBuf::from("/tmp/test-skill/assets"))
        );
    }

    #[test]
    fn test_skill_metadata_fields() {
        let mut skill = Skill::new("test", "Test", "Content");
        skill.license = Some("MIT".to_string());
        skill.compatibility = Some("Requires Docker".to_string());

        assert_eq!(skill.license, Some("MIT".to_string()));
        assert_eq!(skill.compatibility, Some("Requires Docker".to_string()));
    }
}

// ============================================================================
// TDD Tests for Skill Hot-Loading Framework - Phase 1.1: SkillType
// ============================================================================

#[cfg(test)]
mod skill_type_tests {
    use super::*;

    #[test]
    fn test_skill_type_default() {
        let skill_type: SkillType = SkillType::default();
        assert_eq!(skill_type, SkillType::Text);
    }

    #[test]
    fn test_skill_type_all_variants() {
        let text = SkillType::Text;
        assert_eq!(format!("{:?}", text), "Text");
        
        let tool = SkillType::Tool;
        assert_eq!(format!("{:?}", tool), "Tool");
        
        let wasm = SkillType::Wasm;
        assert_eq!(format!("{:?}", wasm), "Wasm");
    }

    #[test]
    fn test_skill_new_has_default_skill_type() {
        let skill = Skill::new("test", "Test description", "Content");
        assert_eq!(skill.skill_type, SkillType::Text);
    }

    #[test]
    fn test_skill_with_explicit_tool_type() {
        let mut skill = Skill::new("tool-skill", "A tool skill", "Do something");
        skill.skill_type = SkillType::Tool;
        skill.tools = vec!["bash".to_string(), "http".to_string()];
        
        assert_eq!(skill.skill_type, SkillType::Tool);
        assert_eq!(skill.tools.len(), 2);
        assert!(skill.tools.contains(&"bash".to_string()));
    }

    #[test]
    fn test_text_skill_has_empty_tools() {
        let skill = Skill::new("text-skill", "A text skill", "Content");
        assert_eq!(skill.skill_type, SkillType::Text);
        assert!(skill.tools.is_empty());
    }

    #[test]
    fn test_parse_frontmatter_with_skill_type() {
        let content = r#"---
name: tool-skill
description: A tool skill
skill-type: tool
tools:
  - bash
  - http
---

# Tool Skill
This is a tool skill."#;
        
        let (fm, body) = parse_frontmatter(content).unwrap();
        assert_eq!(fm.name, "tool-skill");
        assert_eq!(fm.skill_type, SkillType::Tool);
        assert_eq!(fm.tools, vec!["bash", "http"]);
        assert!(body.contains("Tool Skill"));
    }

    #[test]
    fn test_parse_frontmatter_default_skill_type() {
        let content = r#"---
name: simple-skill
description: A simple skill
---

# Simple"#;
        
        let (fm, _body) = parse_frontmatter(content).unwrap();
        assert_eq!(fm.skill_type, SkillType::Text);
    }

    #[test]
    fn test_skill_content_for_tool_type() {
        let content = r#"---
name: calculator
description: A calculator tool
skill-type: tool
tools:
  - bash
---

# Calculator Tool

You are a calculator. Use bash to perform calculations."#;
        
        let skill = Skill::from_content("calculator", content);
        assert_eq!(skill.skill_type, SkillType::Tool);
        assert!(skill.content.contains("Calculator"));
    }

    #[test]
    fn test_wasm_skill_type() {
        let content = r#"---
name: wasm-skill
description: A WASM skill
skill-type: wasm
wasm-path: ./skill.wasm
---

# WASM Skill
This runs in WASM."#;
        
        let skill = Skill::from_content("wasm-skill", content);
        assert_eq!(skill.skill_type, SkillType::Wasm);
    }

    #[test]
    fn test_tool_skill_multiple_tools() {
        let mut skill = Skill::new("multi-tool", "Multiple tools", "Content");
        skill.skill_type = SkillType::Tool;
        skill.tools = vec![
            "bash".to_string(),
            "read".to_string(), 
            "glob".to_string(),
            "grep".to_string(),
        ];
        
        assert_eq!(skill.tools.len(), 4);
        assert!(skill.is_tool_skill());
    }

    #[test]
    fn test_is_tool_skill_helper() {
        let text_skill = Skill::new("text", "Text", "Content");
        assert!(!text_skill.is_tool_skill());
        
        let mut tool_skill = Skill::new("tool", "Tool", "Content");
        tool_skill.skill_type = SkillType::Tool;
        assert!(tool_skill.is_tool_skill());
        
        let mut wasm_skill = Skill::new("wasm", "Wasm", "Content");
        wasm_skill.skill_type = SkillType::Wasm;
        assert!(!wasm_skill.is_tool_skill());
    }

    #[test]
    fn test_skill_type_clone() {
        let original = SkillType::Tool;
        let cloned = original.clone();
        assert_eq!(original, cloned);
    }

    #[test]
    fn test_skill_config_field() {
        let content = r#"---
name: config-skill
description: A skill with config
skill-type: tool
tools:
  - bash
config:
  timeout: 5000
  retries: 3
---

# Config Skill"#;
        
        let skill = Skill::from_content("config-skill", content);
        assert!(!skill.config.is_empty());
        assert_eq!(skill.config.get("timeout"), Some(&serde_json::json!(5000)));
    }
}
