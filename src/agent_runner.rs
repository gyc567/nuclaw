use crate::config::{anthropic_api_key, anthropic_base_url, claude_model};
use crate::error::{NuClawError, Result};
use crate::types::{ContainerInput, ContainerOutput};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AgentRunnerMode {
    #[default]
    Container,
    Api,
}

pub fn agent_runner_mode() -> AgentRunnerMode {
    match std::env::var("AGENT_RUNNER_MODE").as_deref() {
        Ok("api") => AgentRunnerMode::Api,
        _ => AgentRunnerMode::Container,
    }
}

#[async_trait]
pub trait AgentRunner: Send + Sync {
    async fn run(&self, input: ContainerInput) -> Result<ContainerOutput>;
}

#[derive(Debug, Serialize)]
struct AnthropicMessage {
    role: String,
    content: String,
}

#[derive(Debug, Serialize)]
struct AnthropicRequest {
    model: String,
    messages: Vec<AnthropicMessage>,
    max_tokens: u32,
    system: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AnthropicResponse {
    content: Vec<ContentBlock>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
#[allow(dead_code)]
enum ContentBlock {
    Text { text: String },
    Error { error: ApiError },
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct ApiError {
    #[serde(rename = "type")]
    error_type: String,
    message: String,
}

pub struct ApiRunner {
    client: Client,
    api_key: String,
    base_url: String,
    model: String,
}

impl ApiRunner {
    pub fn new() -> Result<Self> {
        let api_key = anthropic_api_key().ok_or_else(|| {
            NuClawError::Config {
                message: "ANTHROPIC_API_KEY is required for API mode".to_string(),
            }
        })?;

        let base_url = anthropic_base_url().unwrap_or_else(|| "https://api.anthropic.com".to_string());
        let model = claude_model().unwrap_or_else(|| "claude-sonnet-4-20250514".to_string());

        let client = Client::new();

        Ok(Self {
            client,
            api_key,
            base_url,
            model,
        })
    }
}

#[async_trait]
impl AgentRunner for ApiRunner {
    async fn run(&self, input: ContainerInput) -> Result<ContainerOutput> {
        let system = build_system_prompt(&input);

        let messages = vec![AnthropicMessage {
            role: "user".to_string(),
            content: input.prompt.clone(),
        }];

        let request = AnthropicRequest {
            model: self.model.clone(),
            messages,
            max_tokens: 4096,
            system: Some(system),
        };

        let url = format!("{}/v1/messages", self.base_url);
        let response = self
            .client
            .post(&url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&request)
            .send()
            .await
            .map_err(|e| NuClawError::Api {
                message: format!("HTTP request failed: {}", e),
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Ok(ContainerOutput {
                status: "error".to_string(),
                result: None,
                new_session_id: input.session_id,
                error: Some(format!("API error ({}): {}", status, body)),
            });
        }

        let anthropic_response: AnthropicResponse = response
            .json()
            .await
            .map_err(|e| NuClawError::Api {
                message: format!("Failed to parse response: {}", e),
            })?;

        let content = anthropic_response
            .content
            .into_iter()
            .filter_map(|block| {
                if let ContentBlock::Text { text } = block {
                    Some(text)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join("\n");

        Ok(ContainerOutput {
            status: "success".to_string(),
            result: Some(content),
            new_session_id: input.session_id,
            error: None,
        })
    }
}

fn build_system_prompt(input: &ContainerInput) -> String {
    let mut prompt = String::new();
    
    prompt.push_str("You are Claude, an AI assistant.\n\n");
    
    if input.is_main {
        prompt.push_str("You are running in the main context.\n");
    } else {
        prompt.push_str("You are running in an isolated context.\n");
    }
    
    if input.is_scheduled_task {
        prompt.push_str("This is a scheduled task.\n");
    }
    
    prompt.push_str(&format!("Group folder: {}\n", input.group_folder));
    
    prompt
}

pub fn create_runner() -> Result<Box<dyn AgentRunner>> {
    match agent_runner_mode() {
        AgentRunnerMode::Api => {
            let runner = ApiRunner::new()?;
            Ok(Box::new(runner))
        }
        AgentRunnerMode::Container => Ok(Box::new(ContainerRunnerAdapter)),
    }
}

pub struct ContainerRunnerAdapter;

#[async_trait]
impl AgentRunner for ContainerRunnerAdapter {
    async fn run(&self, input: ContainerInput) -> Result<ContainerOutput> {
        crate::container_runner::run_container(input).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_runner_mode_default() {
        std::env::remove_var("AGENT_RUNNER_MODE");
        assert_eq!(agent_runner_mode(), AgentRunnerMode::Container);
    }

    #[test]
    fn test_agent_runner_mode_container() {
        std::env::remove_var("AGENT_RUNNER_MODE");
        std::env::set_var("AGENT_RUNNER_MODE", "container");
        assert_eq!(agent_runner_mode(), AgentRunnerMode::Container);
        std::env::remove_var("AGENT_RUNNER_MODE");
    }

    #[test]
    fn test_agent_runner_mode_api() {
        std::env::remove_var("AGENT_RUNNER_MODE");
        std::env::set_var("AGENT_RUNNER_MODE", "api");
        assert_eq!(agent_runner_mode(), AgentRunnerMode::Api);
        std::env::remove_var("AGENT_RUNNER_MODE");
    }

    #[test]
    fn test_agent_runner_mode_invalid() {
        std::env::remove_var("AGENT_RUNNER_MODE");
        std::env::set_var("AGENT_RUNNER_MODE", "invalid");
        assert_eq!(agent_runner_mode(), AgentRunnerMode::Container);
        std::env::remove_var("AGENT_RUNNER_MODE");
    }

    #[test]
    fn test_build_system_prompt_basic() {
        let input = ContainerInput {
            prompt: "Hello".to_string(),
            session_id: None,
            group_folder: "test_group".to_string(),
            chat_jid: "test@chat".to_string(),
            is_main: true,
            is_scheduled_task: false,
        };
        let prompt = build_system_prompt(&input);
        assert!(prompt.contains("main context"));
        assert!(prompt.contains("test_group"));
    }

    #[test]
    fn test_build_system_prompt_scheduled_task() {
        let input = ContainerInput {
            prompt: "Hello".to_string(),
            session_id: None,
            group_folder: "test_group".to_string(),
            chat_jid: "test@chat".to_string(),
            is_main: false,
            is_scheduled_task: true,
        };
        let prompt = build_system_prompt(&input);
        assert!(prompt.contains("scheduled task"));
        assert!(prompt.contains("isolated context"));
    }

    #[test]
    fn test_build_system_prompt_non_main() {
        let input = ContainerInput {
            prompt: "Hello".to_string(),
            session_id: Some("sess_123".to_string()),
            group_folder: "test_group".to_string(),
            chat_jid: "test@chat".to_string(),
            is_main: false,
            is_scheduled_task: false,
        };
        let prompt = build_system_prompt(&input);
        assert!(prompt.contains("isolated context"));
    }

    #[test]
    fn test_anthropic_request_serialization() {
        let request = AnthropicRequest {
            model: "test-model".to_string(),
            messages: vec![AnthropicMessage {
                role: "user".to_string(),
                content: "Hello".to_string(),
            }],
            max_tokens: 1024,
            system: Some("You are helpful.".to_string()),
        };
        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("test-model"));
        assert!(json.contains("You are helpful"));
    }

    #[test]
    fn test_anthropic_response_deserialization() {
        let response_json = r#"{
            "content": [{"text": "Hello, how can I help?"}]
        }"#;
        let response: AnthropicResponse = serde_json::from_str(response_json).unwrap();
        assert_eq!(response.content.len(), 1);
    }

    #[test]
    fn test_api_runner_creation_requires_api_key() {
        std::env::remove_var("ANTHROPIC_API_KEY");
        let result = ApiRunner::new();
        assert!(result.is_err());
    }

    #[test]
    fn test_api_runner_creation_with_api_key() {
        std::env::remove_var("ANTHROPIC_API_KEY");
        std::env::remove_var("ANTHROPIC_BASE_URL");
        std::env::remove_var("CLAUDE_MODEL");
        
        std::env::set_var("ANTHROPIC_API_KEY", "test-key-123");
        let result = ApiRunner::new();
        assert!(result.is_ok());
        
        std::env::remove_var("ANTHROPIC_API_KEY");
    }
}
