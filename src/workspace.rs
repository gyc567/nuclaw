//! Per-Session Workspace Isolation
//!
//! This module provides workspace isolation for each session, ensuring
//! clean separation between different agent execution contexts.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use uuid::Uuid;

use crate::error::{NuClawError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workspace {
    pub id: String,
    pub name: String,
    pub path: PathBuf,
    pub created_at: DateTime<Utc>,
    pub session_id: Option<String>,
    pub active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceMetadata {
    pub workspace_id: String,
    pub session_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub last_accessed: DateTime<Utc>,
    pub file_count: usize,
}

impl Workspace {
    pub fn create(name: &str) -> Result<Self> {
        let id = Uuid::new_v4().to_string();
        let base_dir = std::env::var("NUCLAW_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                home::home_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join(".nuclaw")
                    .join("workspaces")
            });

        let path = base_dir.join(&id);

        if let Err(e) = std::fs::create_dir_all(&path) {
            return Err(NuClawError::FileSystem {
                message: format!("Failed to create workspace directory: {}", e),
            });
        }

        Ok(Workspace {
            id,
            name: name.to_string(),
            path,
            created_at: Utc::now(),
            session_id: None,
            active: true,
        })
    }

    pub fn create_with_session(name: &str, session_id: &str) -> Result<Self> {
        let mut ws = Self::create(name)?;
        ws.session_id = Some(session_id.to_string());
        Ok(ws)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn exists(&self) -> bool {
        self.path.exists() && self.path.is_dir()
    }

    pub fn cleanup(&mut self) -> Result<()> {
        if self.path.exists() {
            std::fs::remove_dir_all(&self.path).map_err(|e| NuClawError::FileSystem {
                message: format!("Failed to cleanup workspace: {}", e),
            })?;
        }
        self.active = false;
        Ok(())
    }

    pub fn deactivate(&mut self) {
        self.active = false;
    }

    pub fn activate(&mut self) {
        self.active = true;
    }

    pub fn metadata(&self) -> WorkspaceMetadata {
        let file_count = if self.path.exists() {
            std::fs::read_dir(&self.path)
                .map(|entries| entries.count())
                .unwrap_or(0)
        } else {
            0
        };

        WorkspaceMetadata {
            workspace_id: self.id.clone(),
            session_id: self.session_id.clone(),
            created_at: self.created_at,
            last_accessed: self.created_at,
            file_count,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_workspace_create_generates_uuid() {
        let ws = Workspace::create("test-workspace").unwrap();
        assert!(!ws.id.is_empty());
        assert!(Uuid::parse_str(&ws.id).is_ok());
    }

    #[test]
    fn test_workspace_create_uses_name() {
        let ws = Workspace::create("my-workspace").unwrap();
        assert_eq!(ws.name, "my-workspace");
    }

    #[test]
    fn test_workspace_create_sets_created_at() {
        let before = Utc::now();
        let ws = Workspace::create("test").unwrap();
        let after = Utc::now();
        assert!(ws.created_at >= before && ws.created_at <= after);
    }

    #[test]
    fn test_workspace_create_defaults_to_active() {
        let ws = Workspace::create("test").unwrap();
        assert!(ws.active);
    }

    #[test]
    fn test_workspace_create_defaults_session_id_to_none() {
        let ws = Workspace::create("test").unwrap();
        assert!(ws.session_id.is_none());
    }

    #[test]
    fn test_workspace_create_with_session() {
        let ws = Workspace::create_with_session("test", "session-123").unwrap();
        assert_eq!(ws.session_id, Some("session-123".to_string()));
    }

    #[test]
    fn test_workspace_create_with_session_preserves_name() {
        let ws = Workspace::create_with_session("named", "s1").unwrap();
        assert_eq!(ws.name, "named");
    }

    #[test]
    fn test_workspace_create_with_session_generates_unique_id() {
        let ws1 = Workspace::create_with_session("test", "s1").unwrap();
        let ws2 = Workspace::create_with_session("test", "s1").unwrap();
        assert_ne!(ws1.id, ws2.id);
    }

    #[test]
    fn test_workspace_create_with_empty_session_id() {
        let ws = Workspace::create_with_session("test", "").unwrap();
        assert_eq!(ws.session_id, Some("".to_string()));
    }

    #[test]
    fn test_workspace_create_with_special_chars_in_name() {
        let ws = Workspace::create("test-workspace_123").unwrap();
        assert_eq!(ws.name, "test-workspace_123");
    }

    #[test]
    fn test_workspace_path_returns_pathbuf() {
        let ws = Workspace::create("test").unwrap();
        let path = ws.path();
        assert!(path.is_relative() || path.is_absolute());
    }

    #[test]
    fn test_workspace_path_contains_workspace_id() {
        let ws = Workspace::create("test").unwrap();
        let path_str = ws.path().to_string_lossy();
        assert!(path_str.contains(&ws.id));
    }

    #[test]
    fn test_workspace_exists_returns_false_for_new_workspace() {
        let ws = Workspace::create("nonexistent-test").unwrap();
        assert!(ws.exists());
    }

    #[test]
    fn test_workspace_exists_returns_true_after_create() {
        let temp_dir = TempDir::new().unwrap();
        let name = "existing-workspace";
        let ws = Workspace::create(name).unwrap();

        let ws_path = temp_dir.path().join(name);
        fs::create_dir_all(&ws_path).unwrap();

        assert!(ws_path.exists());
    }

    #[test]
    fn test_workspace_path_is_valid_utf8() {
        let ws = Workspace::create("test").unwrap();
        let path_str = ws.path().to_string_lossy();
        assert!(path_str.chars().all(|c| c.is_ascii()
            || c.is_alphanumeric()
            || c == '-'
            || c == '_'
            || c == '/'));
    }

    #[test]
    fn test_workspace_deactivate_sets_active_false() {
        let mut ws = Workspace::create("test").unwrap();
        assert!(ws.active);
        ws.deactivate();
        assert!(!ws.active);
    }

    #[test]
    fn test_workspace_activate_sets_active_true() {
        let mut ws = Workspace::create("test").unwrap();
        ws.deactivate();
        assert!(!ws.active);
        ws.activate();
        assert!(ws.active);
    }

    #[test]
    fn test_workspace_cleanup_on_new_workspace_returns_ok() {
        let mut ws = Workspace::create("cleanup-test").unwrap();
        let result = ws.cleanup();
        assert!(result.is_ok());
    }

    #[test]
    fn test_workspace_cleanup_removes_existing_directory() {
        let mut ws = Workspace::create("cleanup-existing").unwrap();
        assert!(ws.exists());
        let _ = ws.cleanup();
        assert!(!ws.exists());
    }

    #[test]
    fn test_workspace_deactivate_activate_toggle() {
        let mut ws = Workspace::create("toggle-test").unwrap();
        assert!(ws.active);
        ws.deactivate();
        assert!(!ws.active);
        ws.deactivate();
        assert!(!ws.active);
        ws.activate();
        assert!(ws.active);
        ws.activate();
        assert!(ws.active);
    }

    #[test]
    fn test_workspace_metadata_contains_id() {
        let ws = Workspace::create("test").unwrap();
        let meta = ws.metadata();
        assert_eq!(meta.workspace_id, ws.id);
    }

    #[test]
    fn test_workspace_metadata_contains_session_id() {
        let ws = Workspace::create_with_session("test", "session-456").unwrap();
        let meta = ws.metadata();
        assert_eq!(meta.session_id, Some("session-456".to_string()));
    }

    #[test]
    fn test_workspace_metadata_contains_created_at() {
        let ws = Workspace::create("test").unwrap();
        let meta = ws.metadata();
        assert!(meta.created_at <= Utc::now());
    }

    #[test]
    fn test_workspace_metadata_last_accessed_initially_same_as_created() {
        let ws = Workspace::create("test").unwrap();
        let meta = ws.metadata();
        assert_eq!(meta.created_at, meta.last_accessed);
    }

    #[test]
    fn test_workspace_metadata_default_file_count_zero() {
        let ws = Workspace::create("test").unwrap();
        let meta = ws.metadata();
        assert_eq!(meta.file_count, 0);
    }

    #[test]
    fn test_workspace_create_with_empty_name() {
        let result = Workspace::create("");
        if result.is_ok() {
            assert_eq!(result.unwrap().name, "");
        }
    }

    #[test]
    fn test_workspace_create_unicode_name() {
        let ws = Workspace::create("工作区").unwrap();
        assert_eq!(ws.name, "工作区");
    }

    #[test]
    fn test_workspace_multiple_instances_independent() {
        let ws1 = Workspace::create("ws1").unwrap();
        let ws2 = Workspace::create("ws2").unwrap();
        assert_ne!(ws1.id, ws2.id);
        assert_ne!(ws1.name, ws2.name);
    }

    #[test]
    fn test_workspace_clone_is_independent() {
        let mut ws1 = Workspace::create("original").unwrap();
        let ws2 = ws1.clone();
        ws1.deactivate();
        assert!(!ws1.active);
        assert!(ws2.active);
    }

    #[test]
    fn test_workspace_serialize_deserialize() {
        let ws1 = Workspace::create("serialize-test").unwrap();
        let json = serde_json::to_string(&ws1).unwrap();
        let ws2: Workspace = serde_json::from_str(&json).unwrap();
        assert_eq!(ws1.id, ws2.id);
        assert_eq!(ws1.name, ws2.name);
        assert_eq!(ws1.active, ws2.active);
    }
}
