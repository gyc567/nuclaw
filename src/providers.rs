use std::collections::HashMap;
use std::sync::RwLock;

use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::error::{NuClawError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

impl ChatMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: "system".into(),
            content: content.into(),
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: "user".into(),
            content: content.into(),
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: "assistant".into(),
            content: content.into(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ChatResponse {
    pub text: Option<String>,
}

impl ChatResponse {
    pub fn has_text(&self) -> bool {
        self.text.as_ref().map(|s| !s.is_empty()).unwrap_or(false)
    }

    pub fn text_or_empty(&self) -> &str {
        self.text.as_deref().unwrap_or("")
    }
}

#[async_trait]
pub trait Provider: Send + Sync {
    fn name(&self) -> &str;

    async fn chat(&self, message: &str, model: &str, temperature: f64) -> Result<String>;

    async fn chat_with_system(
        &self,
        _system: Option<&str>,
        message: &str,
        model: &str,
        temperature: f64,
    ) -> Result<String> {
        self.chat(message, model, temperature).await
    }

    fn context_window(&self) -> usize {
        100000
    }

    fn max_output_tokens(&self) -> usize {
        4096
    }
}

#[derive(Debug, Clone)]
pub struct ProviderSpec {
    pub name: &'static str,
    pub api_key_env: &'static str,
    pub base_url_env: &'static str,
    pub default_model: Option<&'static str>,
    pub description: &'static str,
}

impl ProviderSpec {
    pub const fn new(
        name: &'static str,
        api_key_env: &'static str,
        base_url_env: &'static str,
        default_model: Option<&'static str>,
        description: &'static str,
    ) -> Self {
        Self {
            name,
            api_key_env,
            base_url_env,
            default_model,
            description,
        }
    }
}

pub const PROVIDERS: &[ProviderSpec] = &[
    ProviderSpec::new(
        "anthropic",
        "ANTHROPIC_API_KEY",
        "ANTHROPIC_BASE_URL",
        Some("claude-sonnet-4-20250514"),
        "Anthropic Claude API",
    ),
    ProviderSpec::new(
        "openai",
        "OPENAI_API_KEY",
        "OPENAI_BASE_URL",
        Some("gpt-4o"),
        "OpenAI GPT API",
    ),
    ProviderSpec::new(
        "openrouter",
        "OPENROUTER_API_KEY",
        "OPENROUTER_BASE_URL",
        Some("anthropic/claude-sonnet-4-20250514"),
        "OpenRouter - Unified LLM Gateway",
    ),
    ProviderSpec::new(
        "custom",
        "CUSTOM_API_KEY",
        "CUSTOM_BASE_URL",
        None,
        "Custom OpenAI-compatible endpoint",
    ),
];

#[derive(Debug, Clone)]
pub struct ProviderConfig {
    pub name: String,
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    pub model: Option<String>,
}

impl ProviderConfig {
    pub fn from_spec(spec: &ProviderSpec) -> Self {
        let api_key = std::env::var(spec.api_key_env).ok();
        let base_url = std::env::var(spec.base_url_env).ok();

        let model = if spec.default_model.is_some() {
            std::env::var(format!("{}_MODEL", spec.name.to_uppercase()))
                .ok()
                .or_else(|| std::env::var("CLAUDE_MODEL").ok())
                .or_else(|| spec.default_model.map(|s| s.to_string()))
        } else {
            None
        };

        Self {
            name: spec.name.to_string(),
            api_key,
            base_url,
            model,
        }
    }

    pub fn is_configured(&self) -> bool {
        self.api_key.is_some()
    }
}

pub struct ProviderRegistry {
    specs: RwLock<HashMap<String, &'static ProviderSpec>>,
    configs: RwLock<HashMap<String, ProviderConfig>>,
}

impl ProviderRegistry {
    pub fn new() -> Self {
        let specs_map: HashMap<String, &'static ProviderSpec> = PROVIDERS
            .iter()
            .map(|spec| (spec.name.to_string(), spec))
            .collect();
        Self {
            specs: RwLock::new(specs_map),
            configs: RwLock::new(HashMap::new()),
        }
    }

    pub fn register(&self, spec: &'static ProviderSpec) {
        if let Ok(mut specs) = self.specs.write() {
            specs.insert(spec.name.to_string(), spec);
        }
    }

    pub fn get_spec(&self, name: &str) -> Option<&'static ProviderSpec> {
        self.specs.read().ok()?.get(name).copied()
    }

    pub fn list_specs(&self) -> Vec<&'static ProviderSpec> {
        self.specs
            .read()
            .ok()
            .map(|s| s.values().copied().collect())
            .unwrap_or_default()
    }

    pub fn load_config(&self, name: &str) -> Option<ProviderConfig> {
        self.specs
            .read()
            .ok()?
            .get(name)
            .copied()
            .map(ProviderConfig::from_spec)
    }

    pub fn get_config(&self, name: &str) -> Option<ProviderConfig> {
        let configs = self.configs.read().ok()?;
        configs.get(name).cloned()
    }

    pub fn set_config(&self, config: ProviderConfig) {
        if let Ok(mut configs) = self.configs.write() {
            configs.insert(config.name.clone(), config);
        }
    }

    pub fn detect_provider(&self) -> Option<String> {
        for spec in PROVIDERS {
            let config = ProviderConfig::from_spec(spec);
            if config.is_configured() {
                return Some(spec.name.to_string());
            }
        }
        None
    }

    pub fn is_configured(&self, name: &str) -> bool {
        self.load_config(name)
            .map(|c| c.is_configured())
            .unwrap_or(false)
    }
}

impl Default for ProviderRegistry {
    fn default() -> Self {
        Self::new()
    }
}

pub fn provider_registry() -> ProviderRegistry {
    ProviderRegistry::new()
}

pub struct AnthropicProvider {
    client: Client,
    api_key: String,
    base_url: String,
    default_model: String,
}

impl AnthropicProvider {
    pub fn new(api_key: String, base_url: Option<String>, model: Option<String>) -> Self {
        Self {
            client: Client::new(),
            api_key,
            base_url: base_url.unwrap_or_else(|| "https://api.anthropic.com".to_string()),
            default_model: model.unwrap_or_else(|| "claude-sonnet-4-20250514".to_string()),
        }
    }
}

#[async_trait]
impl Provider for AnthropicProvider {
    fn name(&self) -> &str {
        "anthropic"
    }

    async fn chat(&self, message: &str, model: &str, temperature: f64) -> Result<String> {
        self.chat_with_system(None, message, model, temperature).await
    }

    async fn chat_with_system(
        &self,
        system: Option<&str>,
        message: &str,
        model: &str,
        temperature: f64,
    ) -> Result<String> {
        let model = if model.is_empty() { &self.default_model } else { model };

        #[derive(serde::Serialize)]
        struct Request {
            model: String,
            max_tokens: usize,
            temperature: f64,
            system: Option<String>,
            messages: Vec<Message>,
        }

        #[derive(serde::Serialize)]
        struct Message {
            role: String,
            content: String,
        }

        let request = Request {
            model: model.to_string(),
            max_tokens: 4096,
            temperature,
            system: system.map(|s| s.to_string()),
            messages: vec![Message {
                role: "user".to_string(),
                content: message.to_string(),
            }],
        };

        let response = self
            .client
            .post(format!("{}/v1/messages", self.base_url))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&request)
            .send()
            .await
            .map_err(|e| NuClawError::Api {
                message: format!("Request failed: {}", e),
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(NuClawError::Api {
                message: format!("API error {}: {}", status, body),
            }.into());
        }

        #[derive(serde::Deserialize)]
        struct Response {
            content: Vec<ContentBlock>,
        }

        #[derive(serde::Deserialize)]
        struct ContentBlock {
            text: Option<String>,
        }

        let resp: Response = response
            .json()
            .await
            .map_err(|e| NuClawError::Api {
                message: format!("Failed to parse response: {}", e),
            })?;

        resp.content
            .into_iter()
            .find_map(|c| c.text)
            .ok_or_else(|| NuClawError::Api {
                message: "No text in response".to_string(),
            }.into())
    }

    fn context_window(&self) -> usize {
        200000
    }

    fn max_output_tokens(&self) -> usize {
        8192
    }
}

pub struct OpenAIProvider {
    client: Client,
    api_key: String,
    base_url: String,
    default_model: String,
}

impl OpenAIProvider {
    pub fn new(api_key: String, base_url: Option<String>, model: Option<String>) -> Self {
        Self {
            client: Client::new(),
            api_key,
            base_url: base_url.unwrap_or_else(|| "https://api.openai.com/v1".to_string()),
            default_model: model.unwrap_or_else(|| "gpt-4o".to_string()),
        }
    }
}

#[async_trait]
impl Provider for OpenAIProvider {
    fn name(&self) -> &str {
        "openai"
    }

    async fn chat(&self, message: &str, model: &str, temperature: f64) -> Result<String> {
        self.chat_with_system(None, message, model, temperature).await
    }

    async fn chat_with_system(
        &self,
        system: Option<&str>,
        message: &str,
        model: &str,
        temperature: f64,
    ) -> Result<String> {
        let model = if model.is_empty() { &self.default_model } else { model };

        #[derive(serde::Serialize)]
        struct Request {
            model: String,
            temperature: f64,
            messages: Vec<Message>,
        }

        #[derive(serde::Serialize)]
        struct Message {
            role: String,
            content: String,
        }

        let mut messages = Vec::new();
        if let Some(sys) = system {
            messages.push(Message {
                role: "system".to_string(),
                content: sys.to_string(),
            });
        }
        messages.push(Message {
            role: "user".to_string(),
            content: message.to_string(),
        });

        let request = Request {
            model: model.to_string(),
            temperature,
            messages,
        };

        let response = self
            .client
            .post(format!("{}/chat/completions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&request)
            .send()
            .await
            .map_err(|e| NuClawError::Api {
                message: format!("Request failed: {}", e),
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(NuClawError::Api {
                message: format!("API error {}: {}", status, body),
            }.into());
        }

        #[derive(serde::Deserialize)]
        struct Response {
            choices: Vec<Choice>,
        }

        #[derive(serde::Deserialize)]
        struct Choice {
            message: ResponseMessage,
        }

        #[derive(serde::Deserialize)]
        struct ResponseMessage {
            content: String,
        }

        let resp: Response = response
            .json()
            .await
            .map_err(|e| NuClawError::Api {
                message: format!("Failed to parse response: {}", e),
            })?;

        resp.choices
            .into_iter()
            .next()
            .map(|c| c.message.content)
            .ok_or_else(|| NuClawError::Api {
                message: "No choices in response".to_string(),
            }.into())
    }

    fn context_window(&self) -> usize {
        128000
    }

    fn max_output_tokens(&self) -> usize {
        16384
    }
}

pub fn create_provider(name: &str, config: &ProviderConfig) -> Option<Box<dyn Provider>> {
    match name {
        "anthropic" => {
            if let Some(api_key) = &config.api_key {
                Some(Box::new(AnthropicProvider::new(
                    api_key.clone(),
                    config.base_url.clone(),
                    config.model.clone(),
                )))
            } else {
                None
            }
        }
        "openai" => {
            if let Some(api_key) = &config.api_key {
                Some(Box::new(OpenAIProvider::new(
                    api_key.clone(),
                    config.base_url.clone(),
                    config.model.clone(),
                )))
            } else {
                None
            }
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_providers_list() {
        let names: Vec<&str> = PROVIDERS.iter().map(|p| p.name).collect();
        assert!(names.contains(&"anthropic"));
        assert!(names.contains(&"openai"));
        assert!(names.contains(&"openrouter"));
    }

    #[test]
    fn test_provider_spec_fields() {
        let spec = &PROVIDERS[0];
        assert_eq!(spec.name, "anthropic");
        assert_eq!(spec.api_key_env, "ANTHROPIC_API_KEY");
        assert!(spec.default_model.is_some());
    }

    #[test]
    fn test_provider_registry_new() {
        let registry = ProviderRegistry::new();
        assert!(!registry.list_specs().is_empty());
    }

    #[test]
    fn test_get_spec() {
        let registry = ProviderRegistry::new();
        let spec = registry.get_spec("anthropic");
        assert!(spec.is_some());
        assert_eq!(spec.unwrap().name, "anthropic");
    }

    #[test]
    fn test_get_spec_nonexistent() {
        let registry = ProviderRegistry::new();
        let spec = registry.get_spec("nonexistent");
        assert!(spec.is_none());
    }

    #[test]
    fn test_load_config() {
        std::env::remove_var("ANTHROPIC_API_KEY");
        let registry = ProviderRegistry::new();
        let config = registry.load_config("anthropic");
        assert!(config.is_some());
    }

    #[test]
    fn test_is_configured_no_key() {
        std::env::remove_var("ANTHROPIC_API_KEY");
        let registry = ProviderRegistry::new();
        assert!(!registry.is_configured("anthropic"));
    }

    #[test]
    fn test_is_configured_with_key() {
        std::env::remove_var("ANTHROPIC_API_KEY");
        std::env::set_var("ANTHROPIC_API_KEY", "test-key");
        let registry = ProviderRegistry::new();
        assert!(registry.is_configured("anthropic"));
        std::env::remove_var("ANTHROPIC_API_KEY");
    }

    #[test]
    fn test_set_config() {
        let registry = ProviderRegistry::new();
        let config = ProviderConfig {
            name: "test".to_string(),
            api_key: Some("key".to_string()),
            base_url: None,
            model: None,
        };
        registry.set_config(config.clone());
        let loaded = registry.get_config("test");
        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap().api_key, Some("key".to_string()));
    }

    #[test]
    fn test_detect_provider() {
        std::env::remove_var("ANTHROPIC_API_KEY");
        std::env::remove_var("OPENAI_API_KEY");

        let registry = ProviderRegistry::new();
        let detected = registry.detect_provider();
        assert!(detected.is_none());

        std::env::set_var("ANTHROPIC_API_KEY", "test-key");
        let detected = registry.detect_provider();
        assert_eq!(detected, Some("anthropic".to_string()));

        std::env::remove_var("ANTHROPIC_API_KEY");
    }

    #[test]
    fn test_provider_config_model() {
        std::env::remove_var("ANTHROPIC_API_KEY");
        std::env::remove_var("ANTHROPIC_MODEL");
        std::env::remove_var("CLAUDE_MODEL");

        let spec = ProviderSpec::new(
            "test",
            "TEST_KEY",
            "TEST_URL",
            Some("default-model"),
            "desc",
        );
        let config = ProviderConfig::from_spec(&spec);
        assert_eq!(config.model, Some("default-model".to_string()));
    }

    #[test]
    fn test_provider_config_env_model_override() {
        std::env::remove_var("ANTHROPIC_API_KEY");
        std::env::remove_var("CLAUDE_MODEL");
        std::env::set_var("ANTHROPIC_MODEL", "custom-model");

        let spec = ProviderSpec::new(
            "anthropic",
            "ANTHROPIC_API_KEY",
            "ANTHROPIC_BASE_URL",
            Some("default"),
            "desc",
        );
        let config = ProviderConfig::from_spec(&spec);
        assert_eq!(config.model, Some("custom-model".to_string()));

        std::env::remove_var("ANTHROPIC_MODEL");
    }

    #[test]
    fn test_list_specs() {
        let registry = ProviderRegistry::new();
        let specs = registry.list_specs();
        assert!(specs.len() >= 4);
    }

    #[test]
    fn test_provider_registry_function() {
        let registry = provider_registry();
        assert!(!registry.list_specs().is_empty());
    }
}
