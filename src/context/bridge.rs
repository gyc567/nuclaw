//! Memory Bridge - Sync between TieredMemory and file system

use std::path::PathBuf;
use thiserror::Error;

use crate::context::loader::Memory;
use crate::error::Result as NuClawResult;

#[derive(Error, Debug)]
pub enum BridgeError {
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Parse error: {0}")]
    ParseError(String),
}

// ============================================================================
// MemoryBridge
// ============================================================================

/// Bridges between TieredMemory (in-memory) and file system (MEMORY.md)
pub struct MemoryBridge {
    file_root: PathBuf,
}

impl MemoryBridge {
    /// Create a new MemoryBridge
    pub fn new(file_root: PathBuf) -> Self {
        Self { file_root }
    }

    /// Get memory file path for a group
    fn get_memory_path(&self, group: &str) -> PathBuf {
        self.file_root.join(group).join("context").join("MEMORY.md")
    }

    /// Write important memory entry to file
    pub async fn remember_to_file(
        &self,
        group: &str,
        key: &str,
        content: &str,
    ) -> Result<(), BridgeError> {
        let path = self.get_memory_path(group);

        // Ensure directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // Read existing or create new
        let mut memory = if path.exists() {
            let existing = std::fs::read_to_string(&path)?;
            Self::parse_memory(&existing).unwrap_or_else(|_| Memory::default_memory())
        } else {
            Memory::default_memory()
        };

        let lesson = format!(
            "- **{}** [{}]: {}",
            chrono::Utc::now().format("%Y-%m-%d"),
            key,
            content
        );
        memory.lessons_learned.retain(|l| !l.contains(key)); // Remove old entry with same key
        memory.lessons_learned.push(lesson);
        memory.last_updated = chrono::Utc::now().format("%Y-%m-%d").to_string();
        memory.version += 1;

        // Write back
        let content = Self::format_memory(&memory);
        std::fs::write(&path, content)?;

        Ok(())
    }

    /// Load memory from file
    pub async fn load_from_file(&self, group: &str) -> Result<Memory, BridgeError> {
        let path = self.get_memory_path(group);

        if !path.exists() {
            return Ok(Memory::default_memory());
        }

        let content = std::fs::read_to_string(&path)?;
        Self::parse_memory(&content).map_err(|e| BridgeError::ParseError(e.to_string()))
    }

    /// Parse memory from markdown
    fn parse_memory(content: &str) -> Result<Memory, serde_yaml::Error> {
        let parts: Vec<&str> = content.split("---").collect();

        if parts.len() >= 3 {
            serde_yaml::from_str(parts[1].trim())
        } else {
            serde_yaml::from_str(content)
        }
    }

    /// Format memory to markdown
    fn format_memory(memory: &Memory) -> String {
        let mut result = String::new();

        // YAML frontmatter
        result.push_str("---\n");
        result.push_str(&format!("last_updated: \"{}\"\n", memory.last_updated));
        result.push_str(&format!("version: {}\n", memory.version));

        if !memory.preferences.is_empty() {
            result.push_str("preferences:\n");
            for pref in &memory.preferences {
                result.push_str(&format!("  - {}\n", pref));
            }
        }

        if !memory.lessons_learned.is_empty() {
            result.push_str("lessons_learned:\n");
            for lesson in &memory.lessons_learned {
                result.push_str(&format!("  - {}\n", lesson));
            }
        }

        result.push_str("---\n");

        // Body
        if !memory.technical_context.is_empty() {
            result.push_str("\n# Technical Context\n");
            result.push_str(&memory.technical_context);
        }

        result
    }

    /// Add user preference to memory
    pub async fn add_preference(&self, group: &str, preference: &str) -> Result<(), BridgeError> {
        let path = self.get_memory_path(group);

        let mut memory = if path.exists() {
            let existing = std::fs::read_to_string(&path)?;
            Self::parse_memory(&existing).unwrap_or_else(|_| Memory::default_memory())
        } else {
            Memory::default_memory()
        };

        if !memory.preferences.contains(&preference.to_string()) {
            memory.preferences.push(preference.to_string());
            memory.last_updated = chrono::Utc::now().format("%Y-%m-%d").to_string();

            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }

            let content = Self::format_memory(&memory);
            std::fs::write(&path, content)?;
        }

        Ok(())
    }
}

// ============================================================================
// UnifiedMemory - Combines TieredMemory and MemoryBridge
// ============================================================================

use crate::memory::{MigrationPolicy, Priority, TieredMemory, TieredMemoryEntry};

/// Unified memory that combines TieredMemory and MemoryBridge
pub struct UnifiedMemory {
    tiered: TieredMemory,
    bridge: MemoryBridge,
}

impl UnifiedMemory {
    pub fn new(
        db_path: impl AsRef<std::path::Path>,
        file_root: impl AsRef<std::path::Path>,
    ) -> NuClawResult<Self> {
        let tiered = TieredMemory::new(db_path, MigrationPolicy::default())?;
        let bridge = MemoryBridge::new(file_root.as_ref().to_path_buf());

        Ok(Self { tiered, bridge })
    }

    pub async fn remember(
        &self,
        group: &str,
        key: &str,
        content: &str,
        priority: Priority,
    ) -> NuClawResult<()> {
        self.tiered.remember(key, content, priority).await?;

        if let Err(e) = self.bridge.remember_to_file(group, key, content).await {
            tracing::warn!("Failed to write to file: {}", e);
        }

        Ok(())
    }

    pub async fn recall(&self, key: &str) -> NuClawResult<Option<TieredMemoryEntry>> {
        self.tiered.recall(key).await
    }

    pub async fn search(&self, query: &str, limit: usize) -> NuClawResult<Vec<TieredMemoryEntry>> {
        self.tiered.search(query, limit).await
    }

    pub async fn load_from_file(&self, group: &str) -> NuClawResult<Memory> {
        let file_memory = match self.bridge.load_from_file(group).await {
            Ok(m) => m,
            Err(_) => Memory::default_memory(),
        };

        for pref in &file_memory.preferences {
            self.tiered
                .remember(&format!("preference:{}", pref), pref, Priority::High)
                .await?;
        }

        for lesson in &file_memory.lessons_learned {
            self.tiered
                .remember(&format!("lesson:{}", lesson), lesson, Priority::Normal)
                .await?;
        }

        Ok(file_memory)
    }

    pub async fn add_preference(&self, group: &str, preference: &str) -> NuClawResult<()> {
        self.bridge
            .add_preference(group, preference)
            .await
            .map_err(|e| crate::NuClawError::FileSystem {
                message: e.to_string(),
            })?;

        self.tiered
            .remember(
                &format!("preference:{}", preference),
                preference,
                Priority::High,
            )
            .await?;

        Ok(())
    }

    /// Get the underlying tiered memory
    pub fn tiered(&self) -> &TieredMemory {
        &self.tiered
    }

    /// Get the underlying bridge
    pub fn bridge(&self) -> &MemoryBridge {
        &self.bridge
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_remember_to_file() {
        let temp = tempdir().expect("Failed to create temp dir");
        let bridge = MemoryBridge::new(temp.path().to_path_buf());

        bridge
            .remember_to_file("test_group", "test_key", "Test lesson")
            .await
            .unwrap();

        let path = temp
            .path()
            .join("test_group")
            .join("context")
            .join("MEMORY.md");
        assert!(path.exists());

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("test_key"));
    }

    #[tokio::test]
    async fn test_unified_memory_new() {
        let temp = tempdir().expect("Failed to create temp dir");

        std::fs::create_dir_all(temp.path().join("db")).unwrap();

        let unified =
            UnifiedMemory::new(temp.path().join("db"), temp.path().join("files")).unwrap();

        assert!(unified.tiered().hot().health_check());
    }

    #[tokio::test]
    async fn test_unified_memory_remember() {
        let temp = tempdir().expect("Failed to create temp dir");

        std::fs::create_dir_all(temp.path().join("db")).unwrap();

        let unified =
            UnifiedMemory::new(temp.path().join("db"), temp.path().join("files")).unwrap();

        unified
            .remember("test_group", "test_key", "Test content", Priority::Normal)
            .await
            .unwrap();

        let result = unified.recall("test_key").await.unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap().content, "Test content");

        let path = temp
            .path()
            .join("files")
            .join("test_group")
            .join("context")
            .join("MEMORY.md");
        assert!(path.exists(), "File should exist at {:?}", path);
    }

    #[tokio::test]
    async fn test_unified_memory_load_from_file() {
        let temp = tempdir().expect("Failed to create temp dir");
        std::fs::create_dir_all(temp.path().join("db")).unwrap();

        let bridge = MemoryBridge::new(temp.path().join("files").to_path_buf());
        bridge
            .remember_to_file("test_group", "key1", "lesson content")
            .await
            .unwrap();

        let unified =
            UnifiedMemory::new(temp.path().join("db"), temp.path().join("files")).unwrap();

        let memory = unified.load_from_file("test_group").await.unwrap();

        assert!(memory.version >= 1, "Should load memory with version");
    }

    #[tokio::test]
    async fn test_unified_memory_add_preference() {
        let temp = tempdir().expect("Failed to create temp dir");
        std::fs::create_dir_all(temp.path().join("db")).unwrap();

        let unified =
            UnifiedMemory::new(temp.path().join("db"), temp.path().join("files")).unwrap();

        unified
            .add_preference("test_group", "new_pref")
            .await
            .unwrap();

        let tiered_result = unified.recall("preference:new_pref").await.unwrap();
        assert!(
            tiered_result.is_some(),
            "Preference should be stored in tiered memory"
        );
    }

    #[tokio::test]
    async fn test_unified_memory_search() {
        let temp = tempdir().expect("Failed to create temp dir");
        std::fs::create_dir_all(temp.path().join("db")).unwrap();

        let unified =
            UnifiedMemory::new(temp.path().join("db"), temp.path().join("files")).unwrap();

        unified
            .remember("test_group", "key1", "hello world", Priority::Normal)
            .await
            .unwrap();

        let results = unified.search("hello", 10).await.unwrap();

        assert!(!results.is_empty());
        assert!(results[0].content.contains("hello"));
    }

    #[tokio::test]
    async fn test_load_from_file() {
        let temp = tempdir().expect("Failed to create temp dir");
        let bridge = MemoryBridge::new(temp.path().to_path_buf());

        // Create a memory file
        let path = temp
            .path()
            .join("test_group")
            .join("context")
            .join("MEMORY.md");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(
            &path,
            r#"---
last_updated: "2026-03-19"
version: 2
preferences:
  - bullet_points
---

# Context
"#,
        )
        .unwrap();

        let memory = bridge.load_from_file("test_group").await.unwrap();

        assert_eq!(memory.version, 2);
        assert!(memory.preferences.contains(&"bullet_points".to_string()));
    }

    #[tokio::test]
    async fn test_load_nonexistent() {
        let temp = tempdir().expect("Failed to create temp dir");
        let bridge = MemoryBridge::new(temp.path().to_path_buf());

        let memory = bridge.load_from_file("nonexistent").await.unwrap();

        // Should return default
        assert_eq!(memory.version, 1);
    }

    #[tokio::test]
    async fn test_add_preference() {
        let temp = tempdir().expect("Failed to create temp dir");
        let bridge = MemoryBridge::new(temp.path().to_path_buf());

        bridge
            .add_preference("test_group", "new_preference")
            .await
            .unwrap();

        let memory = bridge.load_from_file("test_group").await.unwrap();

        assert!(memory.preferences.contains(&"new_preference".to_string()));
    }

    #[tokio::test]
    async fn test_add_duplicate_preference() {
        let temp = tempdir().expect("Failed to create temp dir");
        let bridge = MemoryBridge::new(temp.path().to_path_buf());

        bridge
            .add_preference("test_group", "dup_preference")
            .await
            .unwrap();
        bridge
            .add_preference("test_group", "dup_preference")
            .await
            .unwrap();

        let memory = bridge.load_from_file("test_group").await.unwrap();

        // Should only have one
        assert_eq!(
            memory
                .preferences
                .iter()
                .filter(|p| *p == "dup_preference")
                .count(),
            1
        );
    }
}
