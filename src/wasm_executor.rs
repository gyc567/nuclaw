use async_trait::async_trait;
use extism::{Manifest, Plugin, Wasm};

use crate::skills::{Skill, SkillType};
use crate::tool_registry::{ToolError, ToolResult};

use super::SkillExecutor;

pub struct WasmExecutor;

impl WasmExecutor {
    pub fn new() -> Self {
        Self
    }

    fn get_wasm(&self, skill: &Skill) -> Result<Wasm, ToolError> {
        if let Some(wasm_path) = skill.config.get("wasm_path") {
            if let Some(path_str) = wasm_path.as_str() {
                let bytes = std::fs::read(path_str)
                    .map_err(|e| ToolError::ExecutionFailed(format!("Failed to read WASM file: {}", e)))?;
                return Ok(Wasm::data(bytes));
            }
        }

        if let Some(wasm_url) = skill.config.get("wasm_url") {
            if let Some(url_str) = wasm_url.as_str() {
                return Ok(Wasm::url(url_str));
            }
        }

        if skill.content.starts_with("\\x") || skill.content.len() > 100 {
            return Ok(Wasm::data(skill.content.as_bytes().to_vec()));
        }

        Err(ToolError::ExecutionFailed("No valid WASM source found in skill config".to_string()))
    }

    fn get_function_name(&self, skill: &Skill) -> String {
        skill.config
            .get("function")
            .and_then(|v| v.as_str())
            .unwrap_or("run")
            .to_string()
    }
}

impl Default for WasmExecutor {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SkillExecutor for WasmExecutor {
    async fn execute_skill(&self, skill: &Skill, args: serde_json::Value) -> Result<ToolResult, ToolError> {
        if skill.skill_type != SkillType::Wasm {
            return Err(ToolError::ValidationFailed("Not a WASM skill".to_string()));
        }

        let wasm = self.get_wasm(skill)?;
        let function_name = self.get_function_name(skill);

        let manifest = Manifest::new([wasm]);
        let mut plugin = Plugin::new(&manifest, [], true)
            .map_err(|e| ToolError::ExecutionFailed(format!("Failed to load WASM plugin: {}", e)))?;

        let input_json = args.to_string();

        let result = plugin
            .call::<&str, &str>(&function_name, &input_json)
            .map_err(|e| ToolError::ExecutionFailed(format!("WASM function call failed: {}", e)))?;

        Ok(ToolResult::success(result))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wasm_executor_creation() {
        let _executor = WasmExecutor::new();
    }

    #[test]
    fn test_wasm_executor_default() {
        let _executor = WasmExecutor::default();
    }

    #[test]
    fn test_get_function_name_default() {
        let executor = WasmExecutor::new();
        let skill = Skill::new("test", "test", "content");
        
        let name = executor.get_function_name(&skill);
        assert_eq!(name, "run");
    }

    #[test]
    fn test_get_function_name_custom() {
        let executor = WasmExecutor::new();
        let mut skill = Skill::new("test", "test", "content");
        skill.skill_type = SkillType::Wasm;
        skill.config.insert("function".to_string(), serde_json::Value::String("custom_func".to_string()));
        
        let name = executor.get_function_name(&skill);
        assert_eq!(name, "custom_func");
    }
}
