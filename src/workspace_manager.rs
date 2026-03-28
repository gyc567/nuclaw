//! Workspace Manager - Manages per-session workspace lifecycle
//!
//! This module provides workspace management including:
//! - Session workspace creation and lifecycle
//! - Workspace activation/deactivation
//! - Fallback to group workspace when session workspace unavailable

use crate::error::{NuClawError, Result};
use crate::workspace::Workspace;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};

/// Workspace resolution order
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceType {
    /// Session-specific workspace (highest priority)
    Session,
    /// Group-level workspace (fallback)
    Group,
    /// Default workspace (last resort)
    Default,
}

/// Workspace resolution result
#[derive(Debug, Clone)]
pub struct ResolvedWorkspace {
    pub workspace: Workspace,
    pub workspace_type: WorkspaceType,
    pub fallback_used: bool,
}

/// WorkspaceManager manages workspace lifecycle for agent sessions
pub struct WorkspaceManager {
    /// Active workspaces indexed by session_id
    workspaces: Arc<RwLock<HashMap<String, Workspace>>>,
    /// Base directory for group workspaces
    group_workspace_base: PathBuf,
    /// Default workspace path
    default_workspace: PathBuf,
}

impl WorkspaceManager {
    /// Create a new WorkspaceManager
    pub fn new() -> Self {
        let nuclaw_home = std::env::var("NUCLAW_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                home::home_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join(".nuclaw")
            });

        let group_workspace_base = nuclaw_home.join("groups");
        let default_workspace = nuclaw_home.join("default");

        Self {
            workspaces: Arc::new(RwLock::new(HashMap::new())),
            group_workspace_base,
            default_workspace,
        }
    }

    /// Create a new session workspace
    pub async fn create_session_workspace(&self, session_id: &str, group_folder: &str) -> Result<Workspace> {
        let workspace_name = format!("session_{}_{}", group_folder, session_id);
        let workspace = Workspace::create_with_session(&workspace_name, session_id)?;
        
        // Store in active workspaces
        let mut workspaces = self.workspaces.write().await;
        workspaces.insert(session_id.to_string(), workspace.clone());
        
        info!("Created session workspace: {} for session: {}", workspace.id, session_id);
        
        Ok(workspace)
    }

    /// Get or create workspace for a session
    pub async fn get_workspace(&self, session_id: Option<&str>, group_folder: &str) -> Result<ResolvedWorkspace> {
        // Try session workspace first
        if let Some(sid) = session_id {
            let workspaces = self.workspaces.read().await;
            if let Some(workspace) = workspaces.get(sid) {
                return Ok(ResolvedWorkspace {
                    workspace: workspace.clone(),
                    workspace_type: WorkspaceType::Session,
                    fallback_used: false,
                });
            }
            drop(workspaces);

            // Try to create new session workspace
            match self.create_session_workspace(sid, group_folder).await {
                Ok(workspace) => {
                    return Ok(ResolvedWorkspace {
                        workspace,
                        workspace_type: WorkspaceType::Session,
                        fallback_used: false,
                    });
                }
                Err(e) => {
                    warn!("Failed to create session workspace: {}, falling back to group workspace", e);
                }
            }
        }

        // Fallback to group workspace
        self.get_group_workspace(group_folder).await
    }

    /// Get group workspace (fallback)
    pub async fn get_group_workspace(&self, group_folder: &str) -> Result<ResolvedWorkspace> {
        let group_path = self.group_workspace_base.join(group_folder);
        
        // Ensure group workspace exists
        if !group_path.exists() {
            std::fs::create_dir_all(&group_path).map_err(|e| NuClawError::FileSystem {
                message: format!("Failed to create group workspace: {}", e),
            })?;
        }

        let workspace = Workspace {
            id: format!("group_{}", group_folder),
            name: group_folder.to_string(),
            path: group_path,
            created_at: chrono::Utc::now(),
            session_id: None,
            active: true,
        };

        Ok(ResolvedWorkspace {
            workspace,
            workspace_type: WorkspaceType::Group,
            fallback_used: true,
        })
    }

    /// Get default workspace (last resort)
    pub async fn get_default_workspace(&self) -> Result<ResolvedWorkspace> {
        // Ensure default workspace exists
        if !self.default_workspace.exists() {
            std::fs::create_dir_all(&self.default_workspace).map_err(|e| NuClawError::FileSystem {
                message: format!("Failed to create default workspace: {}", e),
            })?;
        }

        let workspace = Workspace {
            id: "default".to_string(),
            name: "default".to_string(),
            path: self.default_workspace.clone(),
            created_at: chrono::Utc::now(),
            session_id: None,
            active: true,
        };

        Ok(ResolvedWorkspace {
            workspace,
            workspace_type: WorkspaceType::Default,
            fallback_used: true,
        })
    }

    /// Resolve workspace with full fallback chain
    pub async fn resolve_workspace(&self, session_id: Option<&str>, group_folder: &str) -> Result<ResolvedWorkspace> {
        // First try session workspace
        if session_id.is_some() {
            match self.get_workspace(session_id, group_folder).await {
                Ok(ws) => return Ok(ws),
                Err(e) => {
                    warn!("Session workspace unavailable: {}, trying group workspace", e);
                }
            }
        }

        // Try group workspace
        match self.get_group_workspace(group_folder).await {
            Ok(ws) => return Ok(ws),
            Err(e) => {
                warn!("Group workspace unavailable: {}, trying default workspace", e);
            }
        }

        // Last resort: default workspace
        self.get_default_workspace().await
    }

    /// Activate a session workspace
    pub async fn activate_workspace(&self, session_id: &str) -> Result<()> {
        let mut workspaces = self.workspaces.write().await;
        if let Some(workspace) = workspaces.get_mut(session_id) {
            workspace.activate();
            info!("Activated workspace for session: {}", session_id);
            Ok(())
        } else {
            Err(NuClawError::NotFound {
                message: format!("Workspace not found for session: {}", session_id),
            })
        }
    }

    /// Deactivate a session workspace
    pub async fn deactivate_workspace(&self, session_id: &str) -> Result<()> {
        let mut workspaces = self.workspaces.write().await;
        if let Some(workspace) = workspaces.get_mut(session_id) {
            workspace.deactivate();
            info!("Deactivated workspace for session: {}", session_id);
            Ok(())
        } else {
            // Not found is OK - may have already been cleaned up
            Ok(())
        }
    }

    /// Cleanup a session workspace
    pub async fn cleanup_workspace(&self, session_id: &str) -> Result<()> {
        let mut workspaces = self.workspaces.write().await;
        if let Some(mut workspace) = workspaces.remove(session_id) {
            workspace.cleanup()?;
            info!("Cleaned up workspace for session: {}", session_id);
        }
        Ok(())
    }

    /// Get workspace path for container execution
    pub async fn get_workspace_path(&self, session_id: Option<&str>, group_folder: &str) -> Result<PathBuf> {
        let resolved = self.resolve_workspace(session_id, group_folder).await?;
        Ok(resolved.workspace.path().to_path_buf())
    }
}

impl Default for WorkspaceManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup_test_env() -> TempDir {
        let temp_dir = TempDir::new().unwrap();
        std::env::set_var("NUCLAW_HOME", temp_dir.path());
        temp_dir
    }

    #[test]
    fn test_workspace_manager_creation() {
        let _temp = setup_test_env();
        let manager = WorkspaceManager::new();
        // Just verify it creates without panic
        assert!(!manager.group_workspace_base.to_string_lossy().is_empty());
    }

    #[tokio::test]
    async fn test_create_session_workspace() {
        let temp = setup_test_env();
        let manager = WorkspaceManager::new();
        
        let result = manager.create_session_workspace("test-session", "test-group").await;
        assert!(result.is_ok());
        
        let workspace = result.unwrap();
        assert!(workspace.session_id.is_some());
        assert_eq!(workspace.session_id.unwrap(), "test-session");
        assert!(workspace.active);
    }

    #[tokio::test]
    async fn test_get_workspace_for_existing_session() {
        let temp = setup_test_env();
        let manager = WorkspaceManager::new();
        
        // Create workspace first
        manager.create_session_workspace("test-session", "test-group").await.unwrap();
        
        // Get existing workspace
        let result = manager.get_workspace(Some("test-session"), "test-group").await;
        assert!(result.is_ok());
        
        let resolved = result.unwrap();
        assert_eq!(resolved.workspace_type, WorkspaceType::Session);
        assert!(!resolved.fallback_used);
    }

    #[tokio::test]
    async fn test_get_workspace_fallback_to_group() {
        let temp = setup_test_env();
        let manager = WorkspaceManager::new();
        
        // Get workspace without session (should fallback to group)
        let result = manager.get_workspace(None, "test-group").await;
        assert!(result.is_ok());
        
        let resolved = result.unwrap();
        assert_eq!(resolved.workspace_type, WorkspaceType::Group);
        assert!(resolved.fallback_used);
    }

    #[tokio::test]
    async fn test_activate_workspace() {
        let temp = setup_test_env();
        let manager = WorkspaceManager::new();
        
        // Create workspace
        manager.create_session_workspace("test-session", "test-group").await.unwrap();
        
        // Deactivate first
        manager.deactivate_workspace("test-session").await.unwrap();
        
        // Activate
        let result = manager.activate_workspace("test-session").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_activate_nonexistent_workspace_returns_error() {
        let temp = setup_test_env();
        let manager = WorkspaceManager::new();
        
        let result = manager.activate_workspace("nonexistent").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_deactivate_workspace() {
        let temp = setup_test_env();
        let manager = WorkspaceManager::new();
        
        // Create and activate workspace
        manager.create_session_workspace("test-session", "test-group").await.unwrap();
        
        // Deactivate
        let result = manager.deactivate_workspace("test-session").await;
        assert!(result.is_ok());
        
        // Verify it's deactivated
        let workspaces = manager.workspaces.read().await;
        let workspace = workspaces.get("test-session").unwrap();
        assert!(!workspace.active);
    }

    #[tokio::test]
    async fn test_cleanup_workspace() {
        let temp = setup_test_env();
        let manager = WorkspaceManager::new();
        
        // Create workspace
        let workspace = manager.create_session_workspace("test-session", "test-group").await.unwrap();
        
        // Verify workspace exists in manager
        {
            let workspaces = manager.workspaces.read().await;
            assert!(workspaces.contains_key("test-session"));
        }
        
        // Cleanup
        let result = manager.cleanup_workspace("test-session").await;
        assert!(result.is_ok());
        
        // Verify workspace is removed
        {
            let workspaces = manager.workspaces.read().await;
            assert!(!workspaces.contains_key("test-session"));
        }
        
        // Verify directory is cleaned up
        assert!(!workspace.path().exists());
    }

    #[tokio::test]
    async fn test_get_workspace_path() {
        let temp = setup_test_env();
        let manager = WorkspaceManager::new();
        
        let path = manager.get_workspace_path(Some("test-session"), "test-group").await;
        assert!(path.is_ok());
        
        let resolved_path = path.unwrap();
        // Path should exist and be a valid path
        assert!(resolved_path.exists());
        assert!(resolved_path.is_dir());
    }

    #[tokio::test]
    async fn test_resolve_workspace_with_session() {
        let temp = setup_test_env();
        let manager = WorkspaceManager::new();
        
        let result = manager.resolve_workspace(Some("test-session"), "test-group").await;
        assert!(result.is_ok());
        
        let resolved = result.unwrap();
        assert_eq!(resolved.workspace_type, WorkspaceType::Session);
    }

    #[tokio::test]
    async fn test_resolve_workspace_without_session() {
        let temp = setup_test_env();
        let manager = WorkspaceManager::new();
        
        let result = manager.resolve_workspace(None, "test-group").await;
        assert!(result.is_ok());
        
        let resolved = result.unwrap();
        assert_eq!(resolved.workspace_type, WorkspaceType::Group);
    }

    #[tokio::test]
    async fn test_get_default_workspace() {
        let temp = setup_test_env();
        let manager = WorkspaceManager::new();
        
        let result = manager.get_default_workspace().await;
        assert!(result.is_ok());
        
        let resolved = result.unwrap();
        assert_eq!(resolved.workspace_type, WorkspaceType::Default);
    }
}
