#[cfg(test)]
use serial_test::serial;

use crate::config::{anthropic_api_key, anthropic_base_url, claude_model};
use crate::error::{NuClawError, Result};
use crate::types::{ContainerInput, ContainerOutput};
use crate::workspace_manager::WorkspaceManager;
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use regex::Regex;

// Rig-core imports for LLM integration
use rig::client::CompletionClient;
use rig::completion::Prompt;
use rig::providers::anthropic;

// Tool integration
use crate::skill_to_rig::all_skills_to_tools;
use crate::tool_registry::{ToolRegistry, InMemoryToolRegistry};
use crate::skills::{builtin_skills, SkillRegistry};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AgentRunnerMode {
    #[default]
    Container,
    Api,
    Rig,
}

pub fn agent_runner_mode() -> AgentRunnerMode {
    match std::env::var("AGENT_RUNNER_MODE").as_deref() {
        Ok("api") => AgentRunnerMode::Api,
        Ok("rig") => AgentRunnerMode::Rig,
        _ => AgentRunnerMode::Container,
    }
}

#[async_trait]
pub trait AgentRunner: Send + Sync {
    async fn run(&self, input: ContainerInput) -> Result<ContainerOutput>;
}

#[derive(Debug, Serialize, Clone)]
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

#[derive(Debug, Serialize)]
struct AnthropicStreamingRequest {
    model: String,
    messages: Vec<AnthropicMessage>,
    max_tokens: u32,
    system: Option<String>,
    #[serde(rename = "stream")]
    is_streaming: bool,
}

impl From<&AnthropicRequest> for AnthropicStreamingRequest {
    fn from(req: &AnthropicRequest) -> Self {
        Self {
            model: req.model.clone(),
            messages: req.messages.clone(),
            max_tokens: req.max_tokens,
            system: req.system.clone(),
            is_streaming: true,
        }
    }
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
    Thinking { thinking: String, text: Option<String>, #[serde(rename = "type")] block_type: Option<String> },
    Error { error: ApiError },
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct ApiError {
    #[serde(rename = "type")]
    error_type: String,
    message: String,
}

#[derive(Debug, Deserialize)]
struct ApiErrorResponse {
    error: ApiError,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum SSEEvent {
    #[serde(rename = "content_block_start")]
    ContentBlockStart,
    #[serde(rename = "content_block_delta")]
    ContentBlockDelta { delta: SSEDelta },
    #[serde(rename = "message_delta")]
    MessageDelta { delta: SSEDelta },
    #[serde(rename = "message_stop")]
    MessageStop,
}

#[derive(Debug, Deserialize)]
struct SSEDelta {
    text: Option<String>,
}

impl SSEEvent {
    fn delta_text(&self) -> Option<String> {
        match self {
            SSEEvent::ContentBlockDelta { delta } => delta.text.clone(),
            SSEEvent::MessageDelta { delta } => delta.text.clone(),
            _ => None,
        }
    }
}

fn extract_urls(text: &str) -> Vec<String> {
    let url_regex = Regex::new(r"https?://[^\s\)]+").unwrap();
    url_regex
        .find_iter(text)
        .map(|m| m.as_str().to_string())
        .collect()
}

/// Fetch URL content
async fn fetch_url_content(url: &str) -> Option<String> {
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .ok()?;
    
    match client.get(url).send().await {
        Ok(resp) if resp.status().is_success() => {
            let text = resp.text().await.ok()?;
            // Truncate to first 10KB to avoid too much context
            let truncated = if text.len() > 10240 {
                format!("{}...\n\n[Content truncated - total {} bytes]", &text[..10240], text.len())
            } else {
                text
            };
            Some(truncated)
        }
        _ => None,
    }
}

/// Pre-process prompt to fetch URL content
async fn preprocess_prompt(prompt: &str) -> String {
    let urls = extract_urls(prompt);
    
    if urls.is_empty() {
        return prompt.to_string();
    }
    
    let mut processed = prompt.to_string();
    
    for url in urls.iter().take(3) {
        // Limit to first 3 URLs
        if let Some(content) = fetch_url_content(url).await {
            processed.push_str(&format!(
                "\n\n[Content fetched from {}]\n{}\n[/Content from {}]",
                url, content, url
            ));
        }
    }
    
    processed
}

pub struct ApiRunner {
    client: Client,
    api_key: String,
    base_url: String,
    model: String,
    session_history: std::sync::Mutex<std::collections::HashMap<String, Vec<AnthropicMessage>>>,
}

impl ApiRunner {
    pub fn new() -> Result<Self> {
        let api_key = anthropic_api_key().ok_or_else(|| NuClawError::Config {
            message: "ANTHROPIC_API_KEY is required for API mode".to_string(),
        })?;

        let base_url =
            anthropic_base_url().unwrap_or_else(|| "https://api.anthropic.com".to_string());
        let model = claude_model().unwrap_or_else(|| "claude-sonnet-4-20250514".to_string());

        let client = Client::new();

        Ok(Self {
            client,
            api_key,
            base_url,
            model,
            session_history: std::sync::Mutex::new(std::collections::HashMap::new()),
        })
    }

    fn add_to_history(&self, session_id: &str, user_msg: String, assistant_msg: String) {
        let mut history = self.session_history.lock().unwrap();
        let messages = history.entry(session_id.to_string()).or_insert_with(Vec::new);
        messages.push(AnthropicMessage { role: "user".to_string(), content: user_msg });
        messages.push(AnthropicMessage { role: "assistant".to_string(), content: assistant_msg });
        if messages.len() > 20 {
            messages.drain(0..4);
        }
    }

    fn get_history(&self, session_id: &str) -> Vec<AnthropicMessage> {
        let history = self.session_history.lock().unwrap();
        history.get(session_id).cloned().unwrap_or_default()
    }

    fn clear_history(&self, session_id: &str) {
        let mut history = self.session_history.lock().unwrap();
        history.remove(session_id);
    }
}

#[async_trait]
impl AgentRunner for ApiRunner {
    async fn run(&self, input: ContainerInput) -> Result<ContainerOutput> {
        let system = build_system_prompt(&input);
        let session_id = input.session_id.as_deref().unwrap_or("default");

        let processed_content = preprocess_prompt(&input.prompt).await;
        let user_message = processed_content.clone();

        let mut messages = self.get_history(session_id);
        messages.push(AnthropicMessage {
            role: "user".to_string(),
            content: processed_content,
        });

        let request = AnthropicRequest {
            model: self.model.clone(),
            messages: messages.clone(),
            max_tokens: 4096,
            system: Some(system),
        };

        let _url = format!("{}/v1/messages", self.base_url);
        let response = self
            .send_with_retry(&request)
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            let error_msg = Self::parse_api_error(&body).unwrap_or_else(|| {
                format!("API request failed with status {}", status)
            });
            return Ok(ContainerOutput {
                status: "error".to_string(),
                result: None,
                new_session_id: input.session_id,
                error: Some(error_msg),
            });
        }

        let anthropic_response: AnthropicResponse =
            response.json().await.map_err(|e| NuClawError::Api {
                message: format!("Failed to parse response: {}", e),
            })?;

        tracing::debug!("API response content: {:?}", anthropic_response.content);

        let content = Self::extract_response_content(anthropic_response);

        if content.trim().is_empty() {
            return Ok(ContainerOutput {
                status: "error".to_string(),
                result: None,
                new_session_id: input.session_id,
                error: Some("Empty response from API".to_string()),
            });
        }

        self.add_to_history(session_id, user_message, content.clone());

        Ok(ContainerOutput {
            status: "success".to_string(),
            result: Some(content),
            new_session_id: input.session_id,
            error: None,
        })
    }
}

impl ApiRunner {
    fn parse_api_error(body: &str) -> Option<String> {
        let err: ApiErrorResponse = serde_json::from_str(body).ok()?;
        Some(err.error.message)
    }

    fn extract_response_content(response: AnthropicResponse) -> String {
        response
            .content
            .into_iter()
            .filter_map(|block| match block {
                ContentBlock::Text { text } => Some(text),
                ContentBlock::Thinking { text, .. } => text,
                ContentBlock::Error { error } => {
                    Some(format!("[API Error: {}]", error.message))
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    async fn send_with_retry(&self, request: &AnthropicRequest) -> Result<reqwest::Response> {
        let url = format!("{}/v1/messages", self.base_url);
        self.do_send_with_retry(&url, request).await
    }

    async fn send_streaming_with_retry(&self, request: &AnthropicStreamingRequest) -> Result<reqwest::Response> {
        let url = format!("{}/v1/messages", self.base_url);
        self.do_send_with_retry(&url, request).await
    }

    async fn do_send_with_retry<T: Serialize>(&self, url: &str, request: &T) -> Result<reqwest::Response> {
        const MAX_ATTEMPTS: u32 = 3;
        let mut attempts = 0;

        loop {
            attempts += 1;

            let resp = self
                .client
                .post(url)
                .header("x-api-key", &self.api_key)
                .header("anthropic-version", "2023-06-01")
                .json(request)
                .send()
                .await;

            match resp {
                Ok(r) => {
                    let status = r.status();
                    if status.is_success() {
                        return Ok(r);
                    }
                    if status.as_u16() >= 500 && attempts < MAX_ATTEMPTS {
                        tracing::warn!(
                            "API returned {}, retrying ({}/{})",
                            status,
                            attempts,
                            MAX_ATTEMPTS
                        );
                        tokio::time::sleep(tokio::time::Duration::from_secs(attempts as u64)).await;
                        continue;
                    }
                    return Ok(r);
                }
                Err(e) => {
                    if attempts >= MAX_ATTEMPTS {
                        return Err(NuClawError::Api {
                            message: format!("Request failed after {} attempts: {}", MAX_ATTEMPTS, e),
                        });
                    }
                    tracing::warn!(
                        "API request failed (attempt {}/{}): {}",
                        attempts,
                        MAX_ATTEMPTS,
                        e
                    );
                    tokio::time::sleep(tokio::time::Duration::from_secs(attempts as u64)).await;
                }
            }
        }
    }

    pub async fn run_streaming<F>(&self, input: &ContainerInput, mut callback: F) -> Result<ContainerOutput>
    where
        F: FnMut(String) + Send,
    {
        let system = build_system_prompt(input);
        let session_id = input.session_id.as_deref().unwrap_or("default");

        let processed_content = preprocess_prompt(&input.prompt).await;

        let mut messages = self.get_history(session_id);
        messages.push(AnthropicMessage {
            role: "user".to_string(),
            content: processed_content.clone(),
        });

        let request = AnthropicRequest {
            model: self.model.clone(),
            messages: messages.clone(),
            max_tokens: 4096,
            system: Some(system),
        };

        let streaming_request = AnthropicStreamingRequest::from(&request);

        let resp = self.send_streaming_with_retry(&streaming_request).await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            let error_msg = Self::parse_api_error(&body).unwrap_or_else(|| {
                format!("Streaming API failed with status {}", status)
            });
            return Ok(ContainerOutput {
                status: "error".to_string(),
                result: None,
                new_session_id: input.session_id.clone(),
                error: Some(error_msg),
            });
        }

        let full_content = self.parse_sse_stream(resp, &mut callback).await?;
        self.add_to_history(session_id, processed_content, full_content.clone());

        Ok(ContainerOutput {
            status: "success".to_string(),
            result: Some(full_content),
            new_session_id: input.session_id.clone(),
            error: None,
        })
    }

    async fn parse_sse_stream<F>(&self, resp: reqwest::Response, callback: &mut F) -> Result<String>
    where
        F: FnMut(String) + Send,
    {
        use futures::StreamExt;
        
        let mut stream = resp.bytes_stream();
        let mut buffer = String::new();
        let mut full_content = String::new();

        while let Some(chunk_result) = stream.next().await {
            let chunk = chunk_result.map_err(|e| NuClawError::Api {
                message: format!("Failed to read streaming response: {}", e),
            })?;

            buffer.push_str(&String::from_utf8_lossy(&chunk));

            while let Some(line_end) = buffer.find('\n') {
                let line = buffer[..line_end].trim().to_string();
                buffer = buffer[line_end + 1..].to_string();

                if line.starts_with("data: ") {
                    let data = &line[6..];
                    if data == "[DONE]" {
                        return Ok(full_content);
                    }
                    if let Ok(event) = serde_json::from_str::<SSEEvent>(data) {
                        if let Some(text) = event.delta_text() {
                            full_content.push_str(&text);
                            callback(text);
                        }
                    }
                }
            }
        }

        Ok(full_content)
    }
}

fn build_system_prompt(input: &ContainerInput) -> String {
    let mut prompt = String::new();

    prompt.push_str("You are Claude, a helpful AI assistant built by Anthropic.\n\n");

    if input.is_main {
        prompt.push_str("You are running in the **main context** - this is a primary conversation.\n");
    } else {
        prompt.push_str("You are running in an **isolated context** - treat this as a separate task.\n");
    }

    if input.is_scheduled_task {
        prompt.push_str("This is a **scheduled task** - execute autonomously without asking for confirmation.\n");
    }

    prompt.push_str(&format!("**Context:** Group folder: `{}`\n", input.group_folder));
    
    prompt.push_str("\n**Guidelines:**\n");
    prompt.push_str("- Provide clear, concise responses\n");
    prompt.push_str("- Format code blocks with language hints\n");
    prompt.push_str("- If unsure, say so rather than guessing\n");
    prompt.push_str("- Break complex tasks into steps\n");

    prompt
}

pub struct RigRunner {
    client: anthropic::Client,
    model: String,
    tool_registry: InMemoryToolRegistry,
}

impl RigRunner {
    pub fn new() -> Result<Self> {
        let api_key = anthropic_api_key().ok_or_else(|| NuClawError::Config {
            message: "ANTHROPIC_API_KEY is required for Rig mode".to_string(),
        })?;

        let model = claude_model().unwrap_or_else(|| "claude-sonnet-4-20250514".to_string());

        let base_url = anthropic_base_url();

        let client = if let Some(url) = base_url {
            anthropic::Client::builder()
                .base_url(url)
                .api_key(&api_key)
                .build()
                .map_err(|e| NuClawError::Config {
                    message: format!("Failed to create Rig client: {}", e),
                })?
        } else {
            anthropic::Client::new(&api_key)
                .map_err(|e| NuClawError::Config {
                    message: format!("Failed to create Rig client: {}", e),
                })?
        };

        let mut tool_registry = InMemoryToolRegistry::new();
        let registry: &dyn SkillRegistry = &builtin_skills();
        let skills = registry.list();
        let tools = all_skills_to_tools(skills);
        for tool in tools {
            let name = tool.definition().name.clone();
            if let Err(e) = tool_registry.register(tool) {
                tracing::warn!("Failed to register tool {}: {}", name, e);
            }
        }

        Ok(Self { client, model, tool_registry })
    }
}

#[async_trait]
impl AgentRunner for RigRunner {
    async fn run(&self, input: ContainerInput) -> Result<ContainerOutput> {
        let system = build_system_prompt(&input);

        let agent = self
            .client
            .agent(&self.model)
            .preamble(&system)
            .build();

        match agent.prompt(&input.prompt).await {
            Ok(response) => Ok(ContainerOutput {
                status: "success".to_string(),
                result: Some(response),
                new_session_id: input.session_id,
                error: None,
            }),
            Err(e) => Ok(ContainerOutput {
                status: "error".to_string(),
                result: None,
                new_session_id: input.session_id,
                error: Some(format!("Rig error: {}", e)),
            }),
        }
    }
}

pub fn create_runner() -> Result<Box<dyn AgentRunner>> {
    match agent_runner_mode() {
        AgentRunnerMode::Api => {
            let runner = ApiRunner::new()?;
            Ok(Box::new(runner))
        }
        AgentRunnerMode::Rig => {
            let runner = RigRunner::new()?;
            Ok(Box::new(runner))
        }
        AgentRunnerMode::Container => Ok(Box::new(ContainerRunnerAdapter::new())),
    }
}

pub struct ContainerRunnerAdapter {
    workspace_manager: WorkspaceManager,
}

impl ContainerRunnerAdapter {
    pub fn new() -> Self {
        Self {
            workspace_manager: WorkspaceManager::new(),
        }
    }
}

impl Default for ContainerRunnerAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AgentRunner for ContainerRunnerAdapter {
    async fn run(&self, mut input: ContainerInput) -> Result<ContainerOutput> {
        // Resolve workspace for this session
        let workspace_path = self
            .workspace_manager
            .get_workspace_path(input.session_id.as_deref(), &input.group_folder)
            .await?;

        // Update input with workspace path info
        input.session_workspace_id = Some(workspace_path.to_string_lossy().to_string());

        // Clone session_id before moving input
        let session_id_clone = input.session_id.clone();

        // Activate workspace if session exists
        if let Some(ref session_id) = input.session_id {
            let _ = self.workspace_manager.activate_workspace(session_id).await;
        }

        // Run container with workspace (clone input to avoid move)
        let result = crate::container_runner::run_container(input.clone()).await;

        // Deactivate workspace after execution
        if let Some(ref session_id) = session_id_clone {
            let _ = self.workspace_manager.deactivate_workspace(session_id).await;
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Use a guard to ensure env var cleanup even on panic
    struct EnvGuard(&'static str);
    impl Drop for EnvGuard {
        fn drop(&mut self) {
            std::env::remove_var(self.0);
        }
    }

    fn with_env_var(key: &'static str, value: &str) -> EnvGuard {
        std::env::remove_var(key);
        std::env::set_var(key, value);
        EnvGuard(key)
    }

    #[test]
    #[serial]
    fn test_agent_runner_mode_default() {
        std::env::remove_var("AGENT_RUNNER_MODE");
        let _guard = with_env_var("AGENT_RUNNER_MODE", "");
        std::env::remove_var("AGENT_RUNNER_MODE");
        assert_eq!(agent_runner_mode(), AgentRunnerMode::Container);
    }

    #[test]
    #[serial]
    fn test_agent_runner_mode_container() {
        std::env::remove_var("AGENT_RUNNER_MODE");
        let _guard = with_env_var("AGENT_RUNNER_MODE", "container");
        assert_eq!(agent_runner_mode(), AgentRunnerMode::Container);
    }

    #[test]
    #[serial]
    fn test_agent_runner_mode_api() {
        std::env::remove_var("AGENT_RUNNER_MODE");
        let _guard = with_env_var("AGENT_RUNNER_MODE", "api");
        assert_eq!(agent_runner_mode(), AgentRunnerMode::Api);
    }

    #[test]
    #[serial]
    fn test_agent_runner_mode_invalid() {
        std::env::remove_var("AGENT_RUNNER_MODE");
        let _guard = with_env_var("AGENT_RUNNER_MODE", "invalid");
        assert_eq!(agent_runner_mode(), AgentRunnerMode::Container);
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
            session_workspace_id: None,
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
            session_workspace_id: None,
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
            session_workspace_id: Some("ws_456".to_string()),
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
    #[serial]
    fn test_api_runner_creation_requires_api_key() {
        std::env::remove_var("ANTHROPIC_API_KEY");
        let result = ApiRunner::new();
        assert!(result.is_err());
    }

    #[test]
    #[serial]
    fn test_api_runner_creation_with_api_key() {
        std::env::remove_var("ANTHROPIC_API_KEY");
        std::env::remove_var("ANTHROPIC_BASE_URL");
        std::env::remove_var("CLAUDE_MODEL");

        std::env::set_var("ANTHROPIC_API_KEY", "test-key-123");
        let result = ApiRunner::new();
        assert!(result.is_ok());

        std::env::remove_var("ANTHROPIC_API_KEY");
    }

    #[test]
    fn test_sse_event_parsing_content_block_delta() {
        let json = r#"{"type":"content_block_delta","delta":{"text":"Hello"}}"#;
        let event: SSEEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event.delta_text(), Some("Hello".to_string()));
    }

    #[test]
    fn test_sse_event_parsing_message_delta() {
        let json = r#"{"type":"message_delta","delta":{"text":"World"}}"#;
        let event: SSEEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event.delta_text(), Some("World".to_string()));
    }

    #[test]
    fn test_sse_event_parsing_content_block_start() {
        let json = r#"{"type":"content_block_start"}"#;
        let event: SSEEvent = serde_json::from_str(json).unwrap();
        assert!(event.delta_text().is_none());
    }

    #[test]
    fn test_sse_event_parsing_message_stop() {
        let json = r#"{"type":"message_stop"}"#;
        let event: SSEEvent = serde_json::from_str(json).unwrap();
        assert!(event.delta_text().is_none());
    }

    #[test]
    fn test_streaming_request_serialization() {
        let request = AnthropicRequest {
            model: "test-model".to_string(),
            messages: vec![AnthropicMessage {
                role: "user".to_string(),
                content: "Hello".to_string(),
            }],
            max_tokens: 1024,
            system: Some("You are helpful.".to_string()),
        };
        let streaming_request = AnthropicStreamingRequest::from(&request);
        let json = serde_json::to_string(&streaming_request).unwrap();
        assert!(json.contains("stream\":true"));
        assert!(json.contains("test-model"));
        assert!(json.contains("You are helpful"));
    }

    #[test]
    fn test_sse_event_parsing_invalid_json() {
        let json = r#"{"type":"unknown_type","delta":{"text":"test"}}"#;
        let result = serde_json::from_str::<SSEEvent>(json);
        assert!(result.is_err());
    }
}
