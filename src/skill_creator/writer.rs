//! Skill Writer Module
//!
//! Generates SKILL.md files from SkillIntent

use std::fs;
use std::path::{Path, PathBuf};

use crate::config::skills_dir;
use crate::error::{NuClawError, Result};
use super::intent::{SkillIntent, SkillIntentType};

/// Skill writer that generates SKILL.md files
pub struct SkillWriter {
    base_path: PathBuf,
}

impl SkillWriter {
    pub fn new() -> Self {
        Self {
            base_path: skills_dir(),
        }
    }

    pub fn with_path(path: PathBuf) -> Self {
        Self { base_path: path }
    }

    /// Generate SKILL.md content from intent
    pub fn generate_content(&self, intent: &SkillIntent) -> String {
        let mut content = String::new();

        // YAML frontmatter
        content.push_str("---\n");
        content.push_str(&format!("name: {}\n", intent.name));
        content.push_str(&format!("description: {}\n", intent.description));
        
        if !intent.tools.is_empty() && intent.skill_type == SkillIntentType::Tool {
            content.push_str("skill-type: tool\n");
            content.push_str("tools:\n");
            for tool in &intent.tools {
                content.push_str(&format!("  - {}\n", tool));
            }
        }
        
        content.push_str("---\n\n");

        // Body
        if !intent.body.is_empty() {
            content.push_str(&intent.body);
        } else {
            // Default body template
            content.push_str(&format!(
                "# {}\n\n{}\n\n## Usage\n\nUse this skill when: {}\n",
                intent.name,
                intent.description,
                intent.description
            ));
        }

        content
    }

    /// Write skill to directory
    pub fn write(&self, intent: &SkillIntent) -> Result<PathBuf> {
        let skill_path = self.base_path.join(&intent.name);
        
        // Create skill directory
        fs::create_dir_all(&skill_path).map_err(|e| {
            NuClawError::FileSystem {
                message: format!("Failed to create skill directory: {}", e),
            }
        })?;

        // Check if SKILL.md already exists
        let skill_file = skill_path.join("SKILL.md");
        if skill_file.exists() {
            return Err(NuClawError::FileSystem {
                message: format!("Skill '{}' already exists", intent.name),
            });
        }

        // Generate content
        let content = self.generate_content(intent);
        
        // Write file
        fs::write(&skill_file, content).map_err(|e| {
            NuClawError::FileSystem {
                message: format!("Failed to write SKILL.md: {}", e),
            }
        })?;

        Ok(skill_path)
    }

    /// Write skill with auto-generated unique name if conflict
    pub fn write_with_fallback(&self, intent: &SkillIntent) -> Result<PathBuf> {
        // Try direct write first
        if let Ok(path) = self.write(intent) {
            return Ok(path);
        }

        // Try with suffix
        for i in 1..100 {
            let mut modified_intent = intent.clone();
            modified_intent.name = format!("{}-{}", intent.name, i);
            if let Ok(path) = self.write(&modified_intent) {
                return Ok(path);
            }
        }

        Err(NuClawError::FileSystem {
            message: "Could not create unique skill name".to_string(),
        })
    }

    /// Get the path where a skill would be written
    pub fn skill_path(&self, name: &str) -> PathBuf {
        self.base_path.join(name).join("SKILL.md")
    }

    /// Check if a skill exists
    pub fn exists(&self, name: &str) -> bool {
        self.skill_path(name).exists()
    }

    /// Delete a skill
    pub fn delete(&self, name: &str) -> Result<()> {
        let skill_path = self.base_path.join(name);
        if !skill_path.exists() {
            return Err(NuClawError::FileSystem {
                message: format!("Skill '{}' does not exist", name),
            });
        }

        fs::remove_dir_all(&skill_path).map_err(|e| {
            NuClawError::FileSystem {
                message: format!("Failed to delete skill: {}", e),
            }
        })
    }
}

impl Default for SkillWriter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    fn temp_skills_dir() -> PathBuf {
        env::temp_dir().join("nuclaw-test-skills")
    }

    #[test]
    fn test_generate_content_basic() {
        let writer = SkillWriter::new();
        let intent = SkillIntent {
            name: "test-skill".to_string(),
            description: "A test skill for parsing JSON".to_string(),
            body: String::new(),
            test_cases: vec![],
            skill_type: SkillIntentType::Text,
            tools: vec![],
        };

        let content = writer.generate_content(&intent);
        assert!(content.contains("name: test-skill"));
        assert!(content.contains("description: A test skill for parsing JSON"));
        assert!(content.contains("---"));
    }

    #[test]
    fn test_generate_content_with_body() {
        let writer = SkillWriter::new();
        let intent = SkillIntent {
            name: "json-parser".to_string(),
            description: "Parse JSON files".to_string(),
            body: "# Custom Body\n\nThis is custom content".to_string(),
            test_cases: vec![],
            skill_type: SkillIntentType::Text,
            tools: vec![],
        };

        let content = writer.generate_content(&intent);
        assert!(content.contains("# Custom Body"));
    }

    #[test]
    fn test_generate_content_tool_type() {
        let writer = SkillWriter::new();
        let intent = SkillIntent {
            name: "bash-runner".to_string(),
            description: "Run bash commands".to_string(),
            body: String::new(),
            test_cases: vec![],
            skill_type: SkillIntentType::Tool,
            tools: vec!["bash".to_string(), "glob".to_string()],
        };

        let content = writer.generate_content(&intent);
        assert!(content.contains("skill-type: tool"));
        assert!(content.contains("  - bash"));
        assert!(content.contains("  - glob"));
    }

    #[test]
    fn test_skill_path() {
        let writer = SkillWriter::new();
        let path = writer.skill_path("my-skill");
        assert!(path.to_string_lossy().contains("my-skill"));
        assert!(path.to_string_lossy().contains("SKILL.md"));
    }

    #[test]
    fn test_skill_exists() {
        let temp_dir = temp_skills_dir();
        let writer = SkillWriter::with_path(temp_dir.clone());
        
        // Clean up first
        let _ = writer.delete("nonexistent");
        
        assert!(!writer.exists("nonexistent"));
    }

    #[test]
    fn test_write_and_delete() {
        let temp_dir = temp_skills_dir();
        let writer = SkillWriter::with_path(temp_dir.clone());
        
        let intent = SkillIntent {
            name: "temp-test-skill".to_string(),
            description: "A temporary test skill".to_string(),
            body: "Test content".to_string(),
            test_cases: vec![],
            skill_type: SkillIntentType::Text,
            tools: vec![],
        };

        // Write
        let path = writer.write(&intent).unwrap();
        assert!(path.exists());
        assert!(writer.exists("temp-test-skill"));

        // Delete
        writer.delete("temp-test-skill").unwrap();
        assert!(!writer.exists("temp-test-skill"));
    }

    #[test]
    fn test_write_duplicate_fails() {
        let temp_dir = temp_skills_dir();
        let writer = SkillWriter::with_path(temp_dir.clone());
        
        let intent = SkillIntent {
            name: "duplicate-test-skill".to_string(),
            description: "Test".to_string(),
            body: "Test".to_string(),
            test_cases: vec![],
            skill_type: SkillIntentType::Text,
            tools: vec![],
        };

        // First write succeeds
        writer.write(&intent).unwrap();
        
        // Second write fails
        let result = writer.write(&intent);
        assert!(result.is_err());

        // Clean up
        writer.delete("duplicate-test-skill").ok();
    }

    #[test]
    fn test_write_with_fallback() {
        let temp_dir = temp_skills_dir();
        let writer = SkillWriter::with_path(temp_dir.clone());
        
        let intent = SkillIntent {
            name: "fallback-test".to_string(),
            description: "Test".to_string(),
            body: "Test".to_string(),
            test_cases: vec![],
            skill_type: SkillIntentType::Text,
            tools: vec![],
        };

        // First write
        let path1 = writer.write(&intent).unwrap();
        
        // Second write with fallback
        let path2 = writer.write_with_fallback(&intent).unwrap();
        assert!(path2.to_string_lossy().contains("fallback-test-1"));

        // Clean up
        writer.delete("fallback-test").ok();
        writer.delete("fallback-test-1").ok();
    }
}
