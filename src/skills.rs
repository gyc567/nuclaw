use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::Arc;

use crate::config::skills_dir;

#[derive(Debug, Clone)]
pub struct Skill {
    pub name: String,
    pub description: String,
    pub content: String,
    /// Optional path to skill directory (for bundled resources)
    pub path: Option<std::path::PathBuf>,
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
            content: content.into(),
            path: None,
        }
    }

    /// Create a skill from a directory containing SKILL.md
    pub fn from_directory(dir: &Path) -> Option<Self> {
        let skill_md = dir.join("SKILL.md");
        if !skill_md.exists() {
            return None;
        }

        let content = fs::read_to_string(&skill_md).ok()?;
        let name = dir.file_name()?.to_str()?.to_string();

        // Parse YAML frontmatter for name and description
        let (description, body) = parse_skill_md(&content);

        Some(Self {
            name,
            description,
            content: body,
            path: Some(dir.to_path_buf()),
        })
    }
}

/// Parse SKILL.md to extract YAML frontmatter and body
fn parse_skill_md(content: &str) -> (String, String) {
    if !content.starts_with("---") {
        // No frontmatter, use first line as description
        let lines: Vec<&str> = content.lines().collect();
        let description = lines.first().unwrap_or(&"").to_string();
        return (description, content.to_string());
    }

    // Find closing ---
    if let Some(end_idx) = content[3..].find("---") {
        let frontmatter = &content[3..3 + end_idx];
        let body = content[3 + end_idx + 3..].trim_start().to_string();

        // Parse name and description from frontmatter
        let mut name = String::new();
        let mut description = String::new();

        for line in frontmatter.lines() {
            let line = line.trim();
            if let Some(stripped) = line.strip_prefix("name:") {
                name = stripped.trim().to_string();
            } else if let Some(stripped) = line.strip_prefix("description:") {
                description = stripped.trim().to_string();
            }
        }

        (description, body)
    } else {
        (String::new(), content.to_string())
    }
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
            "Manage GitHub repositories, issues, and pull requests",
            r#"# GitHub Skill

You are a GitHub assistant. You can help users with:
- Creating and managing repositories
- Working with issues and pull requests
- Searching code
- Managing branches
- Getting repository information

When asked to perform GitHub actions, use appropriate GitHub CLI commands or API calls."#,
        ));

        self.register(Skill::new(
            "weather",
            "Get weather information for locations",
            r#"# Weather Skill

You are a weather assistant. You can help users with:
- Current weather conditions
- Weather forecasts
- Temperature, humidity, and wind information

Use available weather APIs to fetch accurate information."#,
        ));

        self.register(Skill::new(
            "search",
            "Search the web for information",
            r#"# Web Search Skill

You are a web search assistant. You can help users with:
- Finding information on the web
- Researching topics
- Getting current news
- Fact checking

Use search tools to find relevant information."#,
        ));

        self.register(Skill::new(
            "memory",
            "Persistent memory and context management",
            r#"# Memory Skill

You are a memory assistant. You can help users with:
- Remembering important information
- Retrieving past conversations
- Managing context
- Storing preferences

Use the memory system to persist and retrieve information across sessions."#,
        ));
    }

    /// Load external skills from ~/.nuclaw/skills/
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
                        // Don't override builtin skills
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

    /// Get skill by name, including external skills
    pub fn get_skill(&self, name: &str) -> Option<Arc<Skill>> {
        self.skills.get(name).cloned()
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
        assert!(names.contains(&"weather".to_string()));
        assert!(names.contains(&"search".to_string()));
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
    fn test_parse_skill_md_with_frontmatter() {
        let content = r#"---
name: test-skill
description: A test skill
---

# Test Skill

This is the body content."#;
        let (desc, body) = parse_skill_md(content);
        assert_eq!(desc, "A test skill");
        assert!(body.contains("Test Skill"));
    }

    #[test]
    fn test_parse_skill_md_without_frontmatter() {
        let content = r#"# Test Skill

This is the body content."#;
        let (desc, body) = parse_skill_md(content);
        assert!(desc.contains("Test Skill"));
        assert!(body.contains("body content"));
    }
}
