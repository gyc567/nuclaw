//! Context Loader - Core module for loading context files
//!
//! This module provides:
//! - ContextLoader: Loads context files (SOUL.md, USER.md, AGENTS.md, MEMORY.md)
//! - Identity, User, AgentRules, Memory: Data structures for context
//!
//! The Memory struct is now provided by the memory module for better reuse.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use thiserror::Error;

use crate::context::security::{ContentSanitizer, PathValidator, SecurityError};

// Re-export Memory from memory module for backward compatibility
pub use crate::context::memory::Memory;

// ============================================================================
// Error Types
// ============================================================================

#[derive(Error, Debug)]
pub enum ContextError {
    #[error("File not found: {0}")]
    FileNotFound(String),

    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("Security error: {0}")]
    SecurityError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

// ============================================================================
// Data Structures
// ============================================================================

/// Identity from SOUL.md
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Identity {
    pub name: String,
    pub role: String,
    pub vibe: String,
    pub emoji: String,
    pub traits: Vec<String>,
    #[serde(default)]
    pub persona: String,
}

impl Identity {
    pub fn default_identity() -> Self {
        Self {
            name: "NuClaw".to_string(),
            role: "Assistant".to_string(),
            vibe: "Helpful".to_string(),
            emoji: "🤖".to_string(),
            traits: vec![],
            persona: String::new(),
        }
    }
}

/// User profile from USER.md
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct User {
    pub name: String,
    pub timezone: String,
    pub language: String,
    pub preferences: Vec<String>,
    #[serde(default)]
    pub background: String,
}

impl User {
    pub fn default_user() -> Self {
        Self {
            name: "User".to_string(),
            timezone: "UTC".to_string(),
            language: "en".to_string(),
            preferences: vec![],
            background: String::new(),
        }
    }
}

/// Agent rules from AGENTS.md
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentRules {
    pub version: String,
    pub startup_sequence: Vec<String>,
    pub memory_rules: String,
    pub safety_boundaries: Vec<String>,
}

impl AgentRules {
    pub fn default_rules() -> Self {
        Self {
            version: "1.0".to_string(),
            startup_sequence: vec![
                "load_identity".to_string(),
                "load_user".to_string(),
                "load_memory".to_string(),
            ],
            memory_rules: "If user says 'remember this' → Write to MEMORY.md".to_string(),
            safety_boundaries: vec![
                "Never expose private data".to_string(),
                "When in doubt, ask first".to_string(),
            ],
        }
    }
}

// NOTE: Memory struct is now imported from memory module
// pub struct Memory { ... }  // Moved to context/memory.rs

/// Complete context for an agent
#[derive(Debug, Clone, Default)]
pub struct AgentContext {
    pub identity: Identity,
    pub user: User,
    pub rules: AgentRules,
    pub memory: Memory,
}

// ============================================================================
// ContextLoader
// ============================================================================

/// Loads context files from group directories
pub struct ContextLoader {
    base_path: PathBuf,
    validator: PathValidator,
    sanitizer: ContentSanitizer,
}

impl ContextLoader {
    /// Create a new ContextLoader
    pub fn new(base_path: PathBuf) -> Self {
        let validator_path = base_path.clone();
        Self {
            base_path,
            validator: PathValidator::new(vec![validator_path]),
            sanitizer: ContentSanitizer::new(),
        }
    }

    /// Get the context directory for a group
    fn get_context_dir(&self, group: &str) -> PathBuf {
        self.base_path.join(group).join("context")
    }

    /// Load complete context for a group
    pub async fn load_context(&self, group: &str) -> Result<AgentContext, ContextError> {
        let context_dir = self.get_context_dir(group);

        // Load all components in parallel
        let (identity, user, rules, memory) = tokio::join!(
            self.load_identity(&context_dir),
            self.load_user(&context_dir),
            self.load_agents(&context_dir),
            self.load_memory(&context_dir),
        );

        Ok(AgentContext {
            identity: identity.unwrap_or_default(),
            user: user.unwrap_or_default(),
            rules: rules.unwrap_or_default(),
            memory: memory.unwrap_or_default(),
        })
    }

    /// Load Identity from SOUL.md
    pub async fn load_identity(&self, context_dir: &Path) -> Result<Identity, ContextError> {
        // Security check: validate context_dir is within allowed roots
        if let Err(e) = self.validator.validate_dir(context_dir) {
            // If directory doesn't exist, return default
            if matches!(e, crate::context::security::SecurityError::PathNotFound(_)) {
                return Ok(Identity::default_identity());
            }
            return Err(ContextError::SecurityError(e.to_string()));
        }

        let path = context_dir.join("SOUL.md");

        if !path.exists() {
            return Ok(Identity::default_identity());
        }

        let content = self.load_and_sanitize(&path)?;

        Self::parse_yaml_frontmatter::<Identity>(&content)
            .map(|mut i| {
                if let Some(body) = content.split("---").nth(2) {
                    Self::parse_identity_from_body(&body.to_string(), &mut i);
                }
                i
            })
            .or_else(|_| Ok(Identity::default_identity()))
    }

    /// Load User from USER.md
    pub async fn load_user(&self, context_dir: &Path) -> Result<User, ContextError> {
        let path = context_dir.join("USER.md");

        if !path.exists() {
            return Ok(User::default_user());
        }

        let content = self.load_and_sanitize(&path)?;

        Self::parse_yaml_frontmatter::<User>(&content).or_else(|_| Ok(User::default_user()))
    }

    /// Load AgentRules from AGENTS.md
    pub async fn load_agents(&self, context_dir: &Path) -> Result<AgentRules, ContextError> {
        let path = context_dir.join("AGENTS.md");

        if !path.exists() {
            return Ok(AgentRules::default_rules());
        }

        let content = self.load_and_sanitize(&path)?;

        Self::parse_yaml_frontmatter::<AgentRules>(&content)
            .or_else(|_| Ok(AgentRules::default_rules()))
    }

    /// Load Memory from MEMORY.md
    pub async fn load_memory(&self, context_dir: &Path) -> Result<Memory, ContextError> {
        let path = context_dir.join("MEMORY.md");

        if !path.exists() {
            return Ok(Memory::default_memory());
        }

        let content = self.load_and_sanitize(&path)?;

        // Use the new memory module's parser
        crate::context::memory::FileMemory::parse_from_markdown(&content)
            .map_err(|e| ContextError::ParseError(e.to_string()))
            .or_else(|_| Ok(Memory::default_memory()))
    }

    /// Load and sanitize a file
    fn load_and_sanitize(&self, path: &Path) -> Result<String, ContextError> {
        // Security validation
        self.validator
            .validate(path)
            .map_err(|e| ContextError::SecurityError(e.to_string()))?;

        // Read content
        let content = std::fs::read_to_string(path)?;

        // Sanitize
        let sanitized = self.sanitizer.sanitize(&content);

        Ok(sanitized)
    }

    /// Parse YAML frontmatter from content
    fn parse_yaml_frontmatter<T: serde::de::DeserializeOwned>(
        content: &str,
    ) -> Result<T, ContextError> {
        let parts: Vec<&str> = content.split("---").collect();

        if parts.len() >= 3 {
            // Has frontmatter
            let yaml = parts[1].trim();
            serde_yaml::from_str(yaml).map_err(|e| ContextError::ParseError(e.to_string()))
        } else {
            // Try parsing whole content as YAML
            serde_yaml::from_str(content).map_err(|e| ContextError::ParseError(e.to_string()))
        }
    }

    /// Parse identity traits from markdown body
    fn parse_identity_from_body(body: &str, identity: &mut Identity) {
        // Look for ## Traits section
        if body.contains("## Core Traits") || body.contains("## Traits") {
            // Could parse bullet points here if needed
        }

        // Store body as persona if empty
        if identity.persona.is_empty() && !body.trim().is_empty() {
            identity.persona = body.trim().to_string();
        }
    }
}

// ============================================================================
// Tests (Backward Compatibility)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_load_identity_valid() {
        let temp = tempdir().expect("Failed to create temp dir");
        let context_dir = temp.path().join("context");
        std::fs::create_dir_all(&context_dir).unwrap();

        let content = r#"---
name: TestBot
role: Assistant
vibe: Professional
emoji: 🔍
traits:
  - thorough
  - accurate
---

# Identity

You are TestBot, a thorough assistant.
"#;
        std::fs::write(context_dir.join("SOUL.md"), content).unwrap();

        let loader = ContextLoader::new(temp.path().to_path_buf());
        let identity = loader.load_identity(&context_dir).await.unwrap();

        assert_eq!(identity.name, "TestBot");
        assert_eq!(identity.role, "Assistant");
        assert_eq!(identity.vibe, "Professional");
        assert!(identity.traits.contains(&"thorough".to_string()));
    }

    #[tokio::test]
    async fn test_load_identity_missing() {
        let temp = tempdir().expect("Failed to create temp dir");
        let context_dir = temp.path().join("context");
        std::fs::create_dir_all(&context_dir).unwrap();

        let loader = ContextLoader::new(temp.path().to_path_buf());
        let identity = loader.load_identity(&context_dir).await.unwrap();

        // Should return default
        assert_eq!(identity.name, "NuClaw");
    }

    #[tokio::test]
    async fn test_load_user() {
        let temp = tempdir().expect("Failed to create temp dir");
        let context_dir = temp.path().join("context");
        std::fs::create_dir_all(&context_dir).unwrap();

        let content = r#"---
name: John
timezone: Asia/Shanghai
language: zh-CN
preferences:
  - short_responses
  - bullet_points
---

# User Profile

Software Engineer.
"#;
        std::fs::write(context_dir.join("USER.md"), content).unwrap();

        let loader = ContextLoader::new(temp.path().to_path_buf());
        let user = loader.load_user(&context_dir).await.unwrap();

        assert_eq!(user.name, "John");
        assert_eq!(user.timezone, "Asia/Shanghai");
        assert!(user.preferences.contains(&"short_responses".to_string()));
    }

    #[tokio::test]
    async fn test_load_user_missing() {
        let temp = tempdir().expect("Failed to create temp dir");
        let context_dir = temp.path().join("context");
        std::fs::create_dir_all(&context_dir).unwrap();

        let loader = ContextLoader::new(temp.path().to_path_buf());
        let user = loader.load_user(&context_dir).await.unwrap();

        assert_eq!(user.name, "User");
    }

    #[tokio::test]
    async fn test_load_agents() {
        let temp = tempdir().expect("Failed to create temp dir");
        let context_dir = temp.path().join("context");
        std::fs::create_dir_all(&context_dir).unwrap();

        let content = r#"---
version: "1.0"
startup_sequence:
  - load_identity
  - load_user
  - load_memory
memory_rules: "Remember important things"
safety_boundaries:
  - Never expose private data
  - Ask first
---

# Agent Rules
"#;
        std::fs::write(context_dir.join("AGENTS.md"), content).unwrap();

        let loader = ContextLoader::new(temp.path().to_path_buf());
        let rules = loader.load_agents(&context_dir).await.unwrap();

        assert_eq!(rules.version, "1.0");
        assert!(rules
            .startup_sequence
            .contains(&"load_identity".to_string()));
        assert!(rules
            .safety_boundaries
            .contains(&"Never expose private data".to_string()));
    }

    #[tokio::test]
    async fn test_load_memory() {
        let temp = tempdir().expect("Failed to create temp dir");
        let context_dir = temp.path().join("context");
        std::fs::create_dir_all(&context_dir).unwrap();

        let content = r#"---
last_updated: "2026-03-19"
version: 3
preferences:
  - bullet_points
lessons_learned:
  - Don't recommend steak restaurants
---

# Memory

Some additional content.
"#;
        std::fs::write(context_dir.join("MEMORY.md"), content).unwrap();

        let loader = ContextLoader::new(temp.path().to_path_buf());
        let memory = loader.load_memory(&context_dir).await.unwrap();

        assert_eq!(memory.version, 3);
        assert!(memory.preferences.contains(&"bullet_points".to_string()));
        assert!(memory
            .lessons_learned
            .contains(&"Don't recommend steak restaurants".to_string()));
    }

    #[tokio::test]
    async fn test_load_context_full() {
        let temp = tempdir().expect("Failed to create temp dir");
        let group_dir = temp.path().join("test_group");
        let context_dir = group_dir.join("context");
        std::fs::create_dir_all(&context_dir).unwrap();

        std::fs::write(
            context_dir.join("SOUL.md"),
            "---\nname: Bot\nrole: Test\nvibe: Test\nemoji: \"\"\ntraits: []\npersona: \"\"\n---\n",
        )
        .unwrap();
        std::fs::write(context_dir.join("USER.md"), "---\nname: User\ntimezone: UTC\nlanguage: en\npreferences: []\nbackground: \"\"\n---\n").unwrap();
        std::fs::write(context_dir.join("AGENTS.md"), "---\nversion: \"1.0\"\nstartup_sequence: []\nmemory_rules: \"\"\nsafety_boundaries: []\n---\n").unwrap();
        std::fs::write(context_dir.join("MEMORY.md"), "---\nlast_updated: \"2026-01-01\"\nversion: 1\npreferences: []\nlessons_learned: []\ntechnical_context: \"\"\n---\n").unwrap();

        let loader = ContextLoader::new(temp.path().to_path_buf());
        let ctx = loader.load_context("test_group").await.unwrap();

        assert_eq!(ctx.identity.name, "Bot");
        assert_eq!(ctx.user.name, "User");
        assert_eq!(ctx.rules.version, "1.0");
        assert_eq!(ctx.memory.version, 1);
    }

    #[tokio::test]
    async fn test_load_context_graceful_degradation() {
        let temp = tempdir().expect("Failed to create temp dir");
        let _group_dir = temp.path().join("empty_group");
        // No context directory

        let loader = ContextLoader::new(temp.path().to_path_buf());
        let ctx = loader.load_context("empty_group").await.unwrap();

        // Should return defaults
        assert_eq!(ctx.identity.name, "NuClaw");
        assert_eq!(ctx.user.name, "User");
    }

    #[tokio::test]
    async fn test_security_path_traversal() {
        let temp = tempdir().expect("Failed to create temp dir");
        let loader = ContextLoader::new(temp.path().to_path_buf());

        // Try to load from outside base path - should fail
        let result = loader.load_identity(&PathBuf::from("/etc")).await;
        assert!(result.is_err());
    }
}
