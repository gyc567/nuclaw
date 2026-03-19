//! Skill to Rig Tool Adapter
//!
//! Converts Skill to Rig-compatible Tool

use std::sync::Arc;
use async_trait::async_trait;

use crate::skills::{Skill, SkillType};
use crate::tool_registry::{Tool, ToolDefinition, ToolError, ToolResult};

/// Adapter to convert a Skill to a Rig-compatible Tool
pub struct SkillAsTool {
    skill: Arc<Skill>,
    executor: Arc<dyn SkillExecutor>,
}

/// Skill executor trait - defines how to execute a skill
#[async_trait]
pub trait SkillExecutor: Send + Sync {
    async fn execute_skill(&self, skill: &Skill, args: serde_json::Value) -> Result<ToolResult, ToolError>;
}

/// Default skill executor that runs skills based on their type
pub struct DefaultSkillExecutor;

#[async_trait]
impl SkillExecutor for DefaultSkillExecutor {
    async fn execute_skill(&self, skill: &Skill, args: serde_json::Value) -> Result<ToolResult, ToolError> {
        // For now, return a placeholder - actual execution depends on skill type
        match skill.skill_type {
            SkillType::Text => {
                // Text skills just return their content
                Ok(ToolResult::success(&skill.content))
            }
            SkillType::Tool => {
                // Tool skills would need actual tool execution
                Ok(ToolResult::success(format!(
                    "Tool skill '{}' executed with args: {:?}",
                    skill.name, args
                )))
            }
            SkillType::Wasm => {
                // WASM skills would need Extism runtime
                Ok(ToolResult::error("WASM execution not implemented yet"))
            }
        }
    }
}

impl SkillAsTool {
    /// Create a new SkillAsTool adapter
    pub fn new(skill: Arc<Skill>) -> Self {
        Self {
            skill,
            executor: Arc::new(DefaultSkillExecutor {}) as Arc<dyn SkillExecutor>,
        }
    }
    
    /// Create with custom executor
    pub fn with_executor(skill: Arc<Skill>, executor: Arc<dyn SkillExecutor>) -> Self {
        Self { skill, executor }
    }
}

#[async_trait]
impl Tool for SkillAsTool {
    fn definition(&self) -> ToolDefinition {
        let mut params = Vec::new();
        
        // Add required tools as parameters for Tool type skills
        if self.skill.skill_type == SkillType::Tool {
            for tool_name in &self.skill.tools {
                params.push(crate::tool_registry::ToolParam {
                    name: tool_name.clone(),
                    description: format!("Tool: {}", tool_name),
                    required: false,
                    param_type: "string".to_string(),
                });
            }
        }
        
        // Add config parameters
        for (key, value) in &self.skill.config {
            params.push(crate::tool_registry::ToolParam {
                name: key.clone(),
                description: format!("Config: {} = {:?}", key, value),
                required: false,
                param_type: "string".to_string(),
            });
        }
        
        ToolDefinition {
            name: self.skill.name.clone(),
            description: self.skill.description.clone(),
            params,
        }
    }

    async fn execute(&self, args: serde_json::Value) -> Result<ToolResult, ToolError> {
        self.executor.execute_skill(&self.skill, args).await
    }
}

/// Convert a vector of Skills to Tools
pub fn skills_to_tools(skills: Vec<Arc<Skill>>) -> Vec<Arc<dyn Tool>> {
    skills
        .into_iter()
        .filter(|s| s.is_tool_skill())
        .map(SkillAsTool::new)
        .map(|t| Arc::new(t) as Arc<dyn Tool>)
        .collect()
}

/// Convert all Skills (including Text) to Tools  
pub fn all_skills_to_tools(skills: Vec<Arc<Skill>>) -> Vec<Arc<dyn Tool>> {
    skills
        .into_iter()
        .map(SkillAsTool::new)
        .map(|t| Arc::new(t) as Arc<dyn Tool>)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_skill_as_tool_definition() {
        let skill = Arc::new(Skill::new(
            "test-skill",
            "A test skill for testing",
            "Skill content here",
        ));
        
        let tool = SkillAsTool::new(skill);
        let def = tool.definition();
        
        assert_eq!(def.name, "test-skill");
        assert_eq!(def.description, "A test skill for testing");
    }

    #[test]
    fn test_skill_as_tool_with_tools() {
        let mut skill = Skill::new(
            "tool-skill",
            "A tool skill",
            "Content",
        );
        skill.skill_type = SkillType::Tool;
        skill.tools = vec!["bash".to_string(), "http".to_string()];
        
        let tool = SkillAsTool::new(Arc::new(skill));
        let def = tool.definition();
        
        assert_eq!(def.params.len(), 2);
        assert!(def.params.iter().any(|p| p.name == "bash"));
        assert!(def.params.iter().any(|p| p.name == "http"));
    }

    #[test]
    fn test_skills_to_tools_filters_text() {
        let skills = vec![
            Arc::new(Skill::new("text-skill", "Text", "Content")),
            {
                let mut s = Skill::new("tool-skill", "Tool", "Content");
                s.skill_type = SkillType::Tool;
                s.tools = vec!["bash".to_string()];
                Arc::new(s)
            },
        ];
        
        let tools = skills_to_tools(skills);
        
        // Should only include tool skills
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].definition().name, "tool-skill");
    }

    #[test]
    fn test_all_skills_to_tools() {
        let skills = vec![
            Arc::new(Skill::new("text-skill", "Text", "Content")),
            {
                let mut s = Skill::new("tool-skill", "Tool", "Content");
                s.skill_type = SkillType::Tool;
                Arc::new(s)
            },
        ];
        
        let tools = all_skills_to_tools(skills);
        
        // Should include all skills
        assert_eq!(tools.len(), 2);
    }

    #[tokio::test]
    async fn test_skill_as_tool_execute_text() {
        let skill = Arc::new(Skill::new(
            "test-skill",
            "A test skill",
            "Expected content",
        ));
        
        let tool = SkillAsTool::new(skill);
        let result = tool.execute(serde_json::json!({"test": "args"})).await;
        
        assert!(result.is_ok());
        let result = result.unwrap();
        assert!(result.success);
    }
}
