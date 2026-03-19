//! Tool Registry Module
//!
//! Provides a unified interface for registering and executing tools

use std::collections::HashMap;
use std::sync::Arc;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Tool parameter definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolParam {
    pub name: String,
    pub description: String,
    pub required: bool,
    #[serde(default)]
    pub param_type: String,
}

/// Tool definition for LLM
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub params: Vec<ToolParam>,
}

/// Tool execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub success: bool,
    pub result: Option<String>,
    pub error: Option<String>,
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,
}

impl ToolResult {
    pub fn success(result: impl Into<String>) -> Self {
        Self {
            success: true,
            result: Some(result.into()),
            error: None,
            metadata: HashMap::new(),
        }
    }

    pub fn error(msg: impl Into<String>) -> Self {
        Self {
            success: false,
            result: None,
            error: Some(msg.into()),
            metadata: HashMap::new(),
        }
    }
}

/// Error type for tool registry
#[derive(Debug, thiserror::Error)]
pub enum ToolError {
    #[error("Tool not found: {0}")]
    NotFound(String),
    
    #[error("Tool execution failed: {0}")]
    ExecutionFailed(String),
    
    #[error("Tool validation failed: {0}")]
    ValidationFailed(String),
    
    #[error("Registry error: {0}")]
    RegistryError(String),
}

/// Tool trait - unified interface for all tools
#[async_trait]
pub trait Tool: Send + Sync {
    /// Tool definition for LLM
    fn definition(&self) -> ToolDefinition;
    
    /// Execute the tool with JSON arguments
    async fn execute(&self, args: serde_json::Value) -> Result<ToolResult, ToolError>;
}

/// Tool registry trait
pub trait ToolRegistry: Send + Sync {
    /// Register a tool
    fn register(&mut self, tool: Arc<dyn Tool>) -> Result<(), ToolError>;
    
    /// Get a tool by name
    fn get(&self, name: &str) -> Option<Arc<dyn Tool>>;
    
    /// List all tool names
    fn list(&self) -> Vec<String>;
    
    /// Get all tool definitions (for LLM context)
    fn definitions(&self) -> Vec<ToolDefinition>;
}

/// In-memory tool registry implementation
pub struct InMemoryToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
}

impl InMemoryToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }
}

impl Default for InMemoryToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolRegistry for InMemoryToolRegistry {
    fn register(&mut self, tool: Arc<dyn Tool>) -> Result<(), ToolError> {
        self.tools.insert(tool.definition().name.clone(), tool);
        Ok(())
    }
    
    fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools.get(name).cloned()
    }
    
    fn list(&self) -> Vec<String> {
        self.tools.keys().cloned().collect()
    }
    
    fn definitions(&self) -> Vec<ToolDefinition> {
        self.tools.values().map(|t| t.definition()).collect()
    }
}

/// Tool execution context
pub struct ToolContext {
    pub session_id: Option<String>,
    pub group_folder: String,
    pub user_id: Option<String>,
}

impl ToolContext {
    pub fn new(group_folder: impl Into<String>) -> Self {
        Self {
            session_id: None,
            group_folder: group_folder.into(),
            user_id: None,
        }
    }
    
    pub fn with_session(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = Some(session_id.into());
        self
    }
    
    pub fn with_user(mut self, user_id: impl Into<String>) -> Self {
        self.user_id = Some(user_id.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct MockTool {
        name: String,
        description: String,
    }

    #[async_trait]
    impl Tool for MockTool {

        fn definition(&self) -> ToolDefinition {
            ToolDefinition {
                name: self.name.clone(),
                description: self.description.clone(),
                params: vec![ToolParam {
                    name: "input".to_string(),
                    description: "Input string".to_string(),
                    required: true,
                    param_type: "string".to_string(),
                }],
            }
        }

        async fn execute(&self, args: serde_json::Value) -> Result<ToolResult, ToolError> {
            Ok(ToolResult::success(format!("Executed with: {:?}", args)))
        }
    }

    #[test]
    fn test_tool_result_success() {
        let result = ToolResult::success("test");
        assert!(result.success);
        assert_eq!(result.result, Some("test".to_string()));
    }

    #[test]
    fn test_tool_result_error() {
        let result = ToolResult::error("error message");
        assert!(!result.success);
        assert_eq!(result.error, Some("error message".to_string()));
    }

    #[test]
    fn test_in_memory_registry_register() {
        let mut registry = InMemoryToolRegistry::new();
        let tool = Arc::new(MockTool {
            name: "test".to_string(),
            description: "Test tool".to_string(),
        });
        
        let result = registry.register(tool);
        assert!(result.is_ok());
    }

    #[test]
    fn test_in_memory_registry_get() {
        let mut registry = InMemoryToolRegistry::new();
        let tool = Arc::new(MockTool {
            name: "test".to_string(),
            description: "Test tool".to_string(),
        });
        
        registry.register(tool).unwrap();
        let retrieved = registry.get("test");
        assert!(retrieved.is_some());
    }

    #[test]
    fn test_in_memory_registry_list() {
        let mut registry = InMemoryToolRegistry::new();
        
        registry.register(Arc::new(MockTool {
            name: "tool1".to_string(),
            description: "Tool 1".to_string(),
        })).unwrap();
        
        registry.register(Arc::new(MockTool {
            name: "tool2".to_string(),
            description: "Tool 2".to_string(),
        })).unwrap();
        
        let names = registry.list();
        assert_eq!(names.len(), 2);
        assert!(names.contains(&"tool1".to_string()));
        assert!(names.contains(&"tool2".to_string()));
    }

    #[test]
    fn test_in_memory_registry_definitions() {
        let mut registry = InMemoryToolRegistry::new();
        
        registry.register(Arc::new(MockTool {
            name: "test".to_string(),
            description: "Test tool".to_string(),
        })).unwrap();
        
        let defs = registry.definitions();
        assert_eq!(defs.len(), 1);
        assert_eq!(defs[0].name, "test");
    }

    #[test]
    fn test_tool_context() {
        let ctx = ToolContext::new("group1")
            .with_session("session123")
            .with_user("user456");
        
        assert_eq!(ctx.group_folder, "group1");
        assert_eq!(ctx.session_id, Some("session123".to_string()));
        assert_eq!(ctx.user_id, Some("user456".to_string()));
    }
}
