//! Context Memory - Simplified memory management for Agent context
//!
//! This module provides:
//! - Memory struct with preferences and lessons learned
//! - Simple file-based persistence (MEMORY.md)
//! - Remember/recall functionality via files
//!
//! Design principles:
//! - KISS: Keep it simple, no database dependencies
//! - High cohesion: Single responsibility for memory management
//! - Low coupling: Only depends on std and serde

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use thiserror::Error;

// ============================================================================
// Error Types
// ============================================================================

#[derive(Error, Debug)]
pub enum MemoryError {
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("Memory not found: {0}")]
    NotFound(String),
}

// ============================================================================
// Memory Structure
// ============================================================================

/// Memory stored in MEMORY.md
/// Contains user preferences and lessons learned from interactions
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Memory {
    /// Last update timestamp (YYYY-MM-DD)
    pub last_updated: String,
    /// Version number for tracking changes
    pub version: u32,
    /// User preferences learned from interactions
    pub preferences: Vec<String>,
    /// Lessons learned from past interactions (stored as raw strings)
    #[serde(default)]
    pub lessons_learned: Vec<String>,
    /// Technical context (code snippets, configs, etc.)
    #[serde(default)]
    pub technical_context: String,
}

impl Memory {
    /// Create a new Memory with default values
    pub fn new() -> Self {
        Self {
            last_updated: chrono::Utc::now().format("%Y-%m-%d").to_string(),
            version: 1,
            preferences: Vec::new(),
            lessons_learned: Vec::new(),
            technical_context: String::new(),
        }
    }

    /// Create default memory (alias for new())
    pub fn default_memory() -> Self {
        Self::new()
    }

    /// Remember a new preference
    pub fn add_preference(&mut self, preference: &str) {
        if !self.preferences.contains(&preference.to_string()) {
            self.preferences.push(preference.to_string());
            self.last_updated = chrono::Utc::now().format("%Y-%m-%d").to_string();
            self.version += 1;
        }
    }

    /// Remember a new lesson
    pub fn remember(&mut self, key: &str, lesson: &str) {
        // Remove old entry with same key to avoid duplicates
        // Include key in the lesson for proper deduplication
        let formatted = format!(
            "- **[{}]** **{}**: {}",
            key,
            chrono::Utc::now().format("%Y-%m-%d"),
            lesson
        );
        self.lessons_learned.retain(|l| !l.contains(key));
        self.lessons_learned.push(formatted);
        self.last_updated = chrono::Utc::now().format("%Y-%m-%d").to_string();
        self.version += 1;
    }

    /// Add technical context
    pub fn set_technical_context(&mut self, context: &str) {
        self.technical_context = context.to_string();
        self.last_updated = chrono::Utc::now().format("%Y-%m-%d").to_string();
        self.version += 1;
    }
}

// ============================================================================
// File Operations
// ============================================================================

/// Simple file-based memory storage
pub struct FileMemory {
    base_path: PathBuf,
}

impl FileMemory {
    /// Create a new FileMemory with the given base path
    pub fn new(base_path: impl Into<PathBuf>) -> Self {
        Self {
            base_path: base_path.into(),
        }
    }

    /// Get the memory file path for a group
    fn get_memory_path(&self, group: &str) -> PathBuf {
        self.base_path.join(group).join("context").join("MEMORY.md")
    }

    /// Load memory from file
    pub fn load(&self, group: &str) -> Result<Memory, MemoryError> {
        let path = self.get_memory_path(group);

        if !path.exists() {
            return Ok(Memory::default_memory());
        }

        let content = std::fs::read_to_string(&path)?;
        Self::parse_from_markdown(&content)
    }

    /// Save memory to file
    pub fn save(&self, group: &str, memory: &Memory) -> Result<(), MemoryError> {
        let path = self.get_memory_path(group);

        // Ensure directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let content = Self::format_as_markdown(memory);
        std::fs::write(&path, content)?;

        Ok(())
    }

    /// Remember something (load, modify, save)
    pub fn remember(&self, group: &str, key: &str, content: &str) -> Result<(), MemoryError> {
        let mut memory = self.load(group)?;
        memory.remember(key, content);
        self.save(group, &memory)
    }

    /// Add a preference
    pub fn add_preference(&self, group: &str, preference: &str) -> Result<(), MemoryError> {
        let mut memory = self.load(group)?;
        memory.add_preference(preference);
        self.save(group, &memory)
    }

    /// Parse memory from markdown format
    pub fn parse_from_markdown(content: &str) -> Result<Memory, MemoryError> {
        // Try to parse as YAML with frontmatter first
        let parts: Vec<&str> = content.split("---").collect();

        if parts.len() >= 3 {
            // Has YAML frontmatter - parse just the frontmatter
            let yaml = parts[1].trim();
            let mut memory: Memory =
                serde_yaml::from_str(yaml).map_err(|e| MemoryError::ParseError(e.to_string()))?;

            // Ensure defaults for optional fields
            if memory.preferences.is_empty() {
                memory.preferences = Vec::new();
            }
            if memory.lessons_learned.is_empty() {
                memory.lessons_learned = Vec::new();
            }
            if memory.technical_context.is_empty() {
                memory.technical_context = String::new();
            }

            return Ok(memory);
        }

        // No frontmatter - try parsing whole content as YAML
        serde_yaml::from_str(content).map_err(|e| MemoryError::ParseError(e.to_string()))
    }

    /// Format memory as markdown
    pub fn format_as_markdown(memory: &Memory) -> String {
        let mut result = String::new();

        // Use a simple map to ensure all fields are present
        #[derive(Serialize)]
        struct MemoryYaml {
            last_updated: String,
            version: u32,
            preferences: Vec<String>,
            lessons_learned: Vec<String>,
            technical_context: String,
        }

        let yaml = MemoryYaml {
            last_updated: memory.last_updated.clone(),
            version: memory.version,
            preferences: memory.preferences.clone(),
            lessons_learned: memory.lessons_learned.clone(),
            technical_context: memory.technical_context.clone(),
        };

        // YAML frontmatter
        result.push_str("---\n");
        if let Ok(yaml_str) = serde_yaml::to_string(&yaml) {
            result.push_str(&yaml_str);
        }
        result.push_str("---\n");

        // Body (technical context)
        if !memory.technical_context.is_empty() {
            result.push_str("\n# Technical Context\n");
            result.push_str(&memory.technical_context);
        }

        result
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    // ========== Memory Struct Tests ==========

    #[test]
    fn test_memory_new() {
        let memory = Memory::new();

        assert!(memory.version >= 1);
        assert!(!memory.last_updated.is_empty());
        assert!(memory.preferences.is_empty());
        assert!(memory.lessons_learned.is_empty());
    }

    #[test]
    fn test_memory_default() {
        let memory = Memory::default_memory();

        assert_eq!(memory.version, 1);
        assert!(memory.preferences.is_empty());
    }

    #[test]
    fn test_add_preference() {
        let mut memory = Memory::new();

        memory.add_preference("bullet_points");
        assert!(memory.preferences.contains(&"bullet_points".to_string()));
        assert_eq!(memory.version, 2); // Version incremented
    }

    #[test]
    fn test_add_duplicate_preference() {
        let mut memory = Memory::new();

        memory.add_preference("test_pref");
        memory.add_preference("test_pref");

        // Should only have one
        assert_eq!(
            memory
                .preferences
                .iter()
                .filter(|p| *p == "test_pref")
                .count(),
            1
        );
    }

    #[test]
    fn test_remember() {
        let mut memory = Memory::new();

        memory.remember("test_key", "Test lesson content");

        assert!(!memory.lessons_learned.is_empty());
        assert!(memory.lessons_learned[0].contains("Test lesson content"));
        assert!(memory.version >= 2);
    }

    #[test]
    fn test_remember_deduplication() {
        let mut memory = Memory::new();

        memory.remember("same_key", "First lesson");
        memory.remember("same_key", "Second lesson");

        // Should only have one entry with same key
        assert_eq!(
            memory.lessons_learned.len(),
            1,
            "Deduplication should work based on key content"
        );
        assert!(memory.lessons_learned[0].contains("Second lesson"));
    }

    #[test]
    fn test_set_technical_context() {
        let mut memory = Memory::new();

        memory.set_technical_context("fn main() {}");

        assert!(memory.technical_context.contains("fn main()"));
        assert!(memory.version >= 2);
    }

    // ========== FileMemory Tests ==========

    #[test]
    fn test_file_memory_new() {
        let temp = tempdir().unwrap();
        let fm = FileMemory::new(temp.path());

        assert_eq!(fm.base_path, temp.path());
    }

    #[test]
    fn test_load_nonexistent() {
        let temp = tempdir().unwrap();
        let fm = FileMemory::new(temp.path());

        let memory = fm.load("nonexistent_group").unwrap();

        // Should return default
        assert_eq!(memory.version, 1);
    }

    #[test]
    fn test_save_and_load() {
        let temp = tempdir().unwrap();
        let fm = FileMemory::new(temp.path());

        let mut memory = Memory::new();
        memory.add_preference("test_pref");
        memory.remember("test_key", "Test lesson");

        fm.save("test_group", &memory).unwrap();

        let loaded = fm.load("test_group").unwrap();

        assert!(loaded.preferences.contains(&"test_pref".to_string()));
        assert!(!loaded.lessons_learned.is_empty());
    }

    #[test]
    fn test_remember_file() {
        let temp = tempdir().unwrap();
        let fm = FileMemory::new(temp.path());

        fm.remember("test_group", "my_key", "Remember this")
            .unwrap();

        let loaded = fm.load("test_group").unwrap();
        assert!(loaded
            .lessons_learned
            .iter()
            .any(|l| l.contains("Remember this")));
    }

    #[test]
    fn test_add_preference_file() {
        let temp = tempdir().unwrap();
        let fm = FileMemory::new(temp.path());

        fm.add_preference("group1", "short_responses").unwrap();

        let loaded = fm.load("group1").unwrap();
        assert!(loaded.preferences.contains(&"short_responses".to_string()));
    }

    #[test]
    fn test_multiple_groups_isolated() {
        let temp = tempdir().unwrap();
        let fm = FileMemory::new(temp.path());

        fm.add_preference("group_a", "pref_a").unwrap();
        fm.add_preference("group_b", "pref_b").unwrap();

        let loaded_a = fm.load("group_a").unwrap();
        let loaded_b = fm.load("group_b").unwrap();

        assert!(loaded_a.preferences.contains(&"pref_a".to_string()));
        assert!(!loaded_a.preferences.contains(&"pref_b".to_string()));

        assert!(loaded_b.preferences.contains(&"pref_b".to_string()));
        assert!(!loaded_b.preferences.contains(&"pref_a".to_string()));
    }

    // ========== Edge Case Tests ==========

    #[test]
    fn test_empty_preferences() {
        let memory = Memory::new();

        let formatted = FileMemory::format_as_markdown(&memory);

        // Should have empty preferences in YAML
        assert!(formatted.contains("preferences: []") || formatted.contains("preferences:\n"));
    }

    #[test]
    fn test_special_characters_in_preference() {
        let mut memory = Memory::new();

        memory.add_preference("中文偏好");
        memory.add_preference("emoji: 🎉");

        assert_eq!(memory.preferences.len(), 2);
    }

    #[test]
    fn test_version_increment() {
        let mut memory = Memory::new();
        let initial_version = memory.version;

        memory.add_preference("p1");
        assert_eq!(memory.version, initial_version + 1);

        memory.remember("k1", "l1");
        assert_eq!(memory.version, initial_version + 2);

        memory.set_technical_context("code");
        assert_eq!(memory.version, initial_version + 3);
    }

    // ========== Parse Tests ==========

    #[test]
    fn test_parse_valid_yaml() {
        let content = r#"---
last_updated: "2026-03-20"
version: 3
preferences:
  - bullet_points
  - short_responses
lessons_learned:
  - Don't recommend steak
technical_context: ""
"#;

        let memory = FileMemory::parse_from_markdown(content).unwrap();

        assert_eq!(memory.version, 3);
        assert!(memory.preferences.contains(&"bullet_points".to_string()));
        assert!(memory
            .lessons_learned
            .contains(&"Don't recommend steak".to_string()));
    }

    #[test]
    fn test_parse_invalid_yaml() {
        let content = "not valid yaml at all!!!";

        let result = FileMemory::parse_from_markdown(content);
        assert!(result.is_err());
    }

    #[test]
    fn test_format_roundtrip() {
        let mut memory = Memory::new();
        memory.add_preference("test");
        memory.remember("key", "lesson");

        let formatted = FileMemory::format_as_markdown(&memory);
        let parsed = FileMemory::parse_from_markdown(&formatted).unwrap();

        assert_eq!(parsed.preferences, memory.preferences);
        assert_eq!(parsed.lessons_learned.len(), memory.lessons_learned.len());
    }
}
