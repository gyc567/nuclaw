//! Feishu (飞书) Integration for NuClaw
//!
//! Provides Feishu/Lark Bot connectivity via Bot API with webhook support.

use crate::config::{assistant_name, data_dir};
use crate::db::Database;
use crate::error::{NuClawError, Result};
use crate::types::{NewMessage, RegisteredGroup, RouterState};
use crate::utils::json::{load_json, save_json};

use async_trait::async_trait;
use axum::extract::State;
use axum::routing::{get, post};
use axum::{Json, Router};
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

use tracing::{debug, error, info, warn};

/// Default Feishu API base URL
const FEISHU_API_BASE: &str = "https://open.feishu.cn/open-apis";

/// Default Feishu poll interval: 2 seconds
const DEFAULT_FEISHU_POLL_INTERVAL_MS: u64 = 2000;

/// Default text chunk limit for Feishu messages
pub const DEFAULT_TEXT_CHUNK_LIMIT: usize = 4000;

/// Feishu client state
pub struct FeishuClient {
    /// Feishu App ID
    app_id: String,
    /// Feishu App Secret
    app_secret: String,
    /// Current tenant access token
    tenant_access_token: Option<String>,
    /// Token expiration time
    token_expires_at: Option<std::time::Instant>,
    /// Webhook path for receiving messages
    webhook_path: String,
    /// DM policy
    dm_policy: FeishuDMPolicy,
    /// Allowed groups/chats
    allowed_chats: Vec<String>,
    /// Registered chats
    registered_chats: HashMap<String, RegisteredGroup>,
    /// Router state for message deduplication
    router_state: RouterState,
    /// Database connection
    db: Database,
    /// Assistant name for trigger detection
    assistant_name: String,
}

/// DM policy for Feishu
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FeishuDMPolicy {
    /// Pairing mode - users must use a pairing code
    Pairing,
    /// Allowlist mode - only whitelisted users can interact
    Allowlist,
    /// Open mode - anyone can interact
    Open,
    /// Disabled - no DMs allowed
    Disabled,
}

impl FeishuDMPolicy {
    pub fn parse(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "open" => FeishuDMPolicy::Open,
            "allowlist" => FeishuDMPolicy::Allowlist,
            "pairing" => FeishuDMPolicy::Pairing,
            "disabled" => FeishuDMPolicy::Disabled,
            _ => FeishuDMPolicy::Pairing,
        }
    }
}

/// Group policy for Feishu
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FeishuGroupPolicy {
    /// Open - any group can use the bot
    Open,
    /// Allowlist - only whitelisted groups can use the bot
    Allowlist,
    /// Disabled - group functionality disabled
    Disabled,
}

impl FeishuGroupPolicy {
    pub fn parse(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "open" => FeishuGroupPolicy::Open,
            "allowlist" => FeishuGroupPolicy::Allowlist,
            "disabled" => FeishuGroupPolicy::Disabled,
            _ => FeishuGroupPolicy::Allowlist,
        }
    }
}

// Feishu API Types

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeishuTokenResponse {
    pub code: i32,
    pub msg: String,
    pub tenant_access_token: Option<String>,
    pub expire: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeishuMessageContent {
    pub text: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeishuMessage {
    pub msg_type: String,
    pub content: FeishuMessageContent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeishuSendMessageRequest {
    pub receive_id: String,
    pub msg_type: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeishuSendMessageResponse {
    pub code: i32,
    pub msg: String,
    pub data: Option<FeishuSendMessageData>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeishuSendMessageData {
    pub message_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct FeishuReceiveMessage {
    pub schema: String,
    pub header: FeishuMessageHeader,
    pub event: Option<FeishuEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeishuMessageHeader {
    pub event_id: String,
    pub event_type: String,
    pub create_time: String,
    pub token: String,
    pub app_id: String,
    pub tenant_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct FeishuEvent {
    pub sender: Option<FeishuSender>,
    pub chat_id: Option<String>,
    pub message: Option<FeishuMessageBody>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct FeishuSender {
    pub sender_id: FeishuSenderId,
    pub sender_type: String,
    pub tenant_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeishuSenderId {
    pub open_id: Option<String>,
    pub user_id: Option<String>,
    pub union_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct FeishuMessageBody {
    pub message_id: String,
    pub root_id: Option<String>,
    pub parent_id: Option<String>,
    pub create_time: String,
    pub chat_id: String,
    pub chat_type: String,
    pub message_type: String,
    pub content: String,
}

// Helper functions

/// Load router state from disk
pub fn load_router_state() -> RouterState {
    let state_path = data_dir().join("feishu_router_state.json");
    load_json(&state_path, RouterState::default())
}

/// Load registered chats from file
pub fn load_registered_chats() -> HashMap<String, RegisteredGroup> {
    let path = data_dir().join("feishu_registered_chats.json");
    load_json(&path, HashMap::new())
}

/// Truncate text for logging
pub fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}

/// Extract trigger and content from message (pure function)
pub fn extract_trigger_pure(content: &str, assistant_name: &str) -> Option<(String, String)> {
    let trigger_pattern = format!("@_user_{}", assistant_name);
    
    // Check for Feishu mention format: @_user_xxx
    if let Some(idx) = content.find(&trigger_pattern) {
        let after = &content[idx + trigger_pattern.len()..];
        let c = after.trim().to_string();
        return Some((trigger_pattern, c));
    }
    
    // Also check for simple @name format
    let simple_trigger = format!("@{}", assistant_name);
    if let Some(idx) = content.find(&simple_trigger) {
        let after = &content[idx + simple_trigger.len()..];
        let c = after.trim().to_string();
        return Some((simple_trigger, c));
    }
    
    None
}

/// Check if message is duplicate (pure function)
pub fn is_duplicate_message_pure(
    msg: &NewMessage,
    last_message_ids: &std::collections::HashMap<String, String>,
) -> bool {
    if let Some(last_id) = last_message_ids.get(&msg.chat_jid) {
        if last_id == &msg.id {
            return true;
        }
    }
    false
}

/// Check if chat is allowed based on policy
pub fn is_allowed_chat_pure(
    chat_jid: &str,
    policy: FeishuGroupPolicy,
    allowed_chats: &[String],
) -> bool {
    match policy {
        FeishuGroupPolicy::Disabled => false,
        FeishuGroupPolicy::Open => true,
        FeishuGroupPolicy::Allowlist => {
            allowed_chats.is_empty() || allowed_chats.iter().any(|c| chat_jid.contains(c))
        }
    }
}

impl FeishuClient {
    /// Create a new Feishu client
    pub fn new(db: Database) -> Result<Self> {
        let app_id = std::env::var("FEISHU_APP_ID").map_err(|_| NuClawError::Config {
            message: "FEISHU_APP_ID not set".to_string(),
        })?;

        let app_secret = std::env::var("FEISHU_APP_SECRET").map_err(|_| NuClawError::Config {
            message: "FEISHU_APP_SECRET not set".to_string(),
        })?;

        Ok(Self {
            app_id,
            app_secret,
            tenant_access_token: None,
            token_expires_at: None,
            webhook_path: std::env::var("FEISHU_WEBHOOK_PATH")
                .unwrap_or_else(|_| "feishu-webhook".to_string()),
            dm_policy: FeishuDMPolicy::parse(
                &std::env::var("FEISHU_DM_POLICY").unwrap_or_else(|_| "pairing".to_string()),
            ),
            allowed_chats: std::env::var("FEISHU_WHITELIST_CHATS")
                .ok()
                .map(|s| s.split(',').map(|v| v.trim().to_string()).collect())
                .unwrap_or_default(),
            registered_chats: load_registered_chats(),
            router_state: load_router_state(),
            db,
            assistant_name: assistant_name(),
        })
    }

    /// Connect to Feishu and get access token
    pub async fn connect(&mut self) -> Result<()> {
        info!("Connecting to Feishu...");
        self.refresh_token().await?;
        Ok(())
    }

    /// Refresh the tenant access token
    pub async fn refresh_token(&mut self) -> Result<()> {
        info!("Refreshing Feishu access token...");

        let url = format!("{}/auth/v3/tenant_access_token/internal", FEISHU_API_BASE);
        let payload = serde_json::json!({
            "app_id": self.app_id,
            "app_secret": self.app_secret,
        });

        let response = reqwest::Client::new()
            .post(&url)
            .json(&payload)
            .timeout(Duration::from_secs(30))
            .send()
            .await
            .map_err(|e| NuClawError::Feishu {
                message: format!("Failed to refresh token: {}", e),
            })?;

        let token_response: FeishuTokenResponse = response
            .json()
            .await
            .map_err(|e| NuClawError::Feishu {
                message: format!("Failed to parse token response: {}", e),
            })?;

        if token_response.code != 0 {
            return Err(NuClawError::Feishu {
                message: format!("Token refresh failed: {} - {}", token_response.code, token_response.msg),
            });
        }

        self.tenant_access_token = token_response.tenant_access_token;
        self.token_expires_at = Some(
            std::time::Instant::now() + Duration::from_secs(token_response.expire.unwrap_or(7200) as u64),
        );

        info!("Feishu access token refreshed successfully");
        Ok(())
    }

    /// Check if token needs refresh (refresh 5 minutes before expiration)
    async fn ensure_valid_token(&mut self) -> Result<()> {
        if let Some(expires_at) = self.token_expires_at {
            if expires_at
                .checked_sub(Duration::from_secs(300))
                .map(|t| t < std::time::Instant::now())
                .unwrap_or(true)
            {
                self.refresh_token().await?;
            }
        } else {
            self.refresh_token().await?;
        }
        Ok(())
    }

    /// Start webhook server for receiving Feishu events
    pub async fn start_webhook_server(self) -> Result<()> {
        let addr: SocketAddr = std::env::var("FEISHU_WEBHOOK_BIND")
            .unwrap_or_else(|_| "0.0.0.0:8788".to_string())
            .parse()
            .map_err(|_| NuClawError::Config {
                message: "Invalid FEISHU_WEBHOOK_BIND".to_string(),
            })?;

        let client = Arc::new(Mutex::new(self));
        let webhook_path = client.lock().await.webhook_path.clone();

        let app = Router::new()
            .route(&format!("/{}", webhook_path), post(handle_feishu_webhook))
            .route("/health", get(health_check))
            .with_state(client);

        info!("Starting Feishu webhook server on {}", addr);

        let listener =
            tokio::net::TcpListener::bind(&addr)
                .await
                .map_err(|e| NuClawError::Feishu {
                    message: format!("Failed to bind to {}: {}", addr, e),
                })?;

        axum::serve(listener, app)
            .await
            .map_err(|e| NuClawError::Feishu {
                message: format!("Webhook server error: {}", e),
            })?;

        Ok(())
    }

    /// Handle a received Feishu message
    pub async fn handle_event(&mut self, event: &FeishuReceiveMessage) -> Result<Option<String>> {
        let event_data = match &event.event {
            Some(e) => e,
            None => {
                debug!("Received event without data, skipping");
                return Ok(None);
            }
        };

        // Only handle message events
        if event.header.event_type != "im.message.receive_v1" {
            debug!("Ignoring non-message event: {}", event.header.event_type);
            return Ok(None);
        }

        let message = match &event_data.message {
            Some(m) => m,
            None => {
                debug!("Received message event without message body, skipping");
                return Ok(None);
            }
        };

        // Only handle text messages
        if message.message_type != "text" {
            debug!("Ignoring non-text message type: {}", message.message_type);
            return Ok(None);
        }

        let new_message = self.parse_feishu_message(message).await?;
        self.handle_message(&new_message).await
    }

    async fn parse_feishu_message(&self, msg: &FeishuMessageBody) -> Result<NewMessage> {
        // Parse the content JSON
        let content: serde_json::Value = serde_json::from_str(&msg.content)
            .map_err(|e| NuClawError::Feishu {
                message: format!("Failed to parse message content: {}", e),
            })?;

        let text = content
            .get("text")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();

        let sender_id = msg
            .create_time
            .clone(); // Use time as fallback, actual sender extraction below

        // Extract sender info from event
        let (_sender, sender_name) = ("unknown".to_string(), "Feishu User".to_string());

        let chat_jid = if msg.chat_type == "p2p" {
            format!("feishu:user:{}", msg.chat_id)
        } else {
            format!("feishu:chat:{}", msg.chat_id)
        };

        debug!(
            "parse_feishu_message: chat_jid={}, chat_type={}, content={}",
            chat_jid,
            msg.chat_type,
            truncate(&text, 50)
        );

        Ok(NewMessage {
            id: msg.message_id.clone(),
            chat_jid,
            sender: sender_id,
            sender_name,
            content: text,
            timestamp: msg.create_time.clone(),
        })
    }

    /// Handle a parsed message
    pub async fn handle_message(&mut self, msg: &NewMessage) -> Result<Option<String>> {
        if self.is_duplicate_message(msg).await {
            debug!("Skipping duplicate message: {}", msg.id);
            return Ok(None);
        }

        self.update_router_state(msg).await;

        let db = Arc::new(self.db.clone());
        let msg_clone = msg.clone();
        tokio::spawn(async move {
            if let Err(e) = Self::store_message_background(&db, &msg_clone).await {
                error!("Failed to store message: {}", e);
            }
        });

        // Check DM policy for p2p chats
        if !msg.chat_jid.contains(":chat:") {
            if !self.check_dm_policy(&msg.sender).await? {
                debug!("Message from unauthorized user: {}", msg.sender);
                return Ok(None);
            }
        }

        // Check group policy
        if msg.chat_jid.contains(":chat:") && !self.is_allowed_chat(&msg.chat_jid).await? {
            debug!("Message from unauthorized chat: {}", msg.chat_jid);
            return Ok(None);
        }

        let content = msg.content.trim().to_string();
        if content.is_empty() {
            return Ok(None);
        }

        info!(
            "Received Feishu message from {}: {}",
            msg.sender_name,
            truncate(&content, 50)
        );

        let _is_group = msg.chat_jid.contains(":chat:");
        let chat_id = self.extract_chat_id(&msg.chat_jid)?;

        // Build the response
        let response = format!("Feishu message received: {}", content);

        // Send response back
        self.send_message(&chat_id, &response).await?;

        Ok(Some(response))
    }

    /// Send a message to a Feishu chat
    pub async fn send_message(&self, receive_id: &str, text: &str) -> Result<()> {
        if text.trim().is_empty() {
            warn!("Skipping empty message");
            return Ok(());
        }

        debug!(
            "send_message called: receive_id={}, text_len={}",
            receive_id,
            text.len()
        );

        // Get token from shared state (requires mutable access to refresh if needed)
        // This is a simplified version - in production, you'd want proper token management
        let token = self.tenant_access_token.as_ref().ok_or_else(|| NuClawError::Feishu {
            message: "Not connected to Feishu".to_string(),
        })?;

        let url = format!("{}/im/v1/messages", FEISHU_API_BASE);
        
        // Determine receive_id_type based on the format of receive_id
        let receive_id_type = if receive_id.starts_with("oc_") || receive_id.starts_with("chat_") {
            "chat_id"
        } else {
            "open_id"
        };

        let payload = FeishuSendMessageRequest {
            receive_id: receive_id.to_string(),
            msg_type: "text".to_string(),
            content: serde_json::to_string(&serde_json::json!({ "text": text }))
                .map_err(|e| NuClawError::Feishu {
                    message: format!("Failed to serialize message: {}", e),
                })?,
        };

        let response = reqwest::Client::new()
            .post(&url)
            .header("Authorization", format!("Bearer {}", token))
            .query(&[("receive_id_type", receive_id_type)])
            .json(&payload)
            .timeout(Duration::from_secs(30))
            .send()
            .await
            .map_err(|e| NuClawError::Feishu {
                message: format!("Failed to send message: {}", e),
            })?;

        let response_body: FeishuSendMessageResponse = response
            .json()
            .await
            .map_err(|e| NuClawError::Feishu {
                message: format!("Failed to parse send response: {}", e),
            })?;

        if response_body.code != 0 {
            return Err(NuClawError::Feishu {
                message: format!("Send message failed: {} - {}", response_body.code, response_body.msg),
            });
        }

        Ok(())
    }

    async fn check_dm_policy(&self, _user_id: &str) -> Result<bool> {
        match self.dm_policy {
            FeishuDMPolicy::Disabled => Ok(false),
            FeishuDMPolicy::Open => Ok(true),
            FeishuDMPolicy::Allowlist => Ok(true), // In production, check against user allowlist
            FeishuDMPolicy::Pairing => Ok(true), // TODO: Implement pairing logic
        }
    }

    async fn is_allowed_chat(&self, chat_jid: &str) -> Result<bool> {
        Ok(is_allowed_chat_pure(
            chat_jid,
            FeishuGroupPolicy::Allowlist,
            &self.allowed_chats,
        ))
    }

    fn extract_chat_id(&self, jid: &str) -> Result<String> {
        // Extract the actual chat/user ID from the jid
        if let Some(id) = jid.strip_prefix("feishu:chat:") {
            Ok(id.to_string())
        } else if let Some(id) = jid.strip_prefix("feishu:user:") {
            Ok(id.to_string())
        } else {
            Ok(jid.to_string())
        }
    }

    async fn is_duplicate_message(&self, msg: &NewMessage) -> bool {
        is_duplicate_message_pure(msg, &self.router_state.last_message_ids)
    }

    async fn update_router_state(&mut self, msg: &NewMessage) {
        self.router_state
            .last_message_ids
            .insert(msg.chat_jid.clone(), msg.id.clone());

        let router_state = self.router_state.clone();
        tokio::spawn(async move {
            let state_path = data_dir().join("feishu_router_state.json");
            let _ = save_json(&state_path, &router_state);
        });
    }

    async fn store_message_background(db: &Database, msg: &NewMessage) -> Result<()> {
        let conn = db.get_connection().map_err(|e| NuClawError::Database {
            message: e.to_string(),
        })?;

        conn.execute(
            "INSERT OR REPLACE INTO messages (id, chat_jid, sender, sender_name, content, timestamp, is_from_me)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
            rusqlite::params![
                msg.id,
                msg.chat_jid,
                msg.sender,
                msg.sender_name,
                msg.content,
                msg.timestamp,
                if msg.id.starts_with("self") { 1 } else { 0 },
            ],
        )
        .map_err(|e| NuClawError::Database {
            message: format!("Failed to store message: {}", e),
        })?;

        Ok(())
    }
}

// Channel trait implementation for registry integration
#[async_trait]
impl crate::channels::Channel for FeishuClient {
    fn name(&self) -> &str {
        "feishu"
    }

    async fn send(&self, jid: &str, message: &str) -> Result<()> {
        self.send_message(jid, message).await
    }

    async fn start(&self) -> Result<()> {
        // This would be handled separately via start_webhook_server
        Ok(())
    }

    fn is_enabled(&self) -> bool {
        std::env::var("FEISHU_APP_ID").is_ok() && std::env::var("FEISHU_APP_SECRET").is_ok()
    }
}

// Webhook handlers
async fn handle_feishu_webhook(
    State(client): State<Arc<Mutex<FeishuClient>>>,
    Json(event): Json<FeishuReceiveMessage>,
) -> (StatusCode, &'static str) {
    let mut client = client.lock().await;
    match client.handle_event(&event).await {
        Ok(_) => (StatusCode::OK, "OK"),
        Err(e) => {
            error!("Failed to handle Feishu event: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "ERROR")
        }
    }
}

async fn health_check() -> (StatusCode, &'static str) {
    (StatusCode::OK, "OK")
}

/// Determines whether Feishu should auto-start based on environment configuration.
pub fn should_auto_start_feishu() -> bool {
    std::env::var("FEISHU_APP_ID").is_ok() && std::env::var("FEISHU_APP_SECRET").is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_feishu_dm_policy_parse() {
        assert_eq!(FeishuDMPolicy::parse("open"), FeishuDMPolicy::Open);
        assert_eq!(FeishuDMPolicy::parse("allowlist"), FeishuDMPolicy::Allowlist);
        assert_eq!(FeishuDMPolicy::parse("pairing"), FeishuDMPolicy::Pairing);
        assert_eq!(FeishuDMPolicy::parse("disabled"), FeishuDMPolicy::Disabled);
        assert_eq!(FeishuDMPolicy::parse("OPEN"), FeishuDMPolicy::Open);
        assert_eq!(FeishuDMPolicy::parse("unknown"), FeishuDMPolicy::Pairing);
    }

    #[test]
    fn test_feishu_group_policy_parse() {
        assert_eq!(FeishuGroupPolicy::parse("open"), FeishuGroupPolicy::Open);
        assert_eq!(FeishuGroupPolicy::parse("allowlist"), FeishuGroupPolicy::Allowlist);
        assert_eq!(FeishuGroupPolicy::parse("disabled"), FeishuGroupPolicy::Disabled);
        assert_eq!(FeishuGroupPolicy::parse("OPEN"), FeishuGroupPolicy::Open);
        assert_eq!(FeishuGroupPolicy::parse("unknown"), FeishuGroupPolicy::Allowlist);
    }

    #[test]
    fn test_truncate() {
        assert_eq!(truncate("hello", 10), "hello");
        assert_eq!(truncate("hello world", 8), "hello...");
        assert_eq!(truncate("", 5), "");
        assert_eq!(truncate("hi", 2), "hi");
    }

    #[test]
    fn test_extract_trigger_pure() {
        // Test with Feishu mention format: @_user_xxx
        let result = extract_trigger_pure("@_user_Andy hello world", "Andy");
        assert!(result.is_some());
        let (trigger, content) = result.unwrap();
        assert_eq!(trigger, "@_user_Andy");
        assert_eq!(content, "hello world");

        // Test with simple @name format
        let result = extract_trigger_pure("@Andy hello world", "Andy");
        assert!(result.is_some());
        let (trigger, content) = result.unwrap();
        assert_eq!(trigger, "@Andy");
        assert_eq!(content, "hello world");

        // Test without trigger
        let result = extract_trigger_pure("hello world", "Andy");
        assert!(result.is_none());

        // Test with empty string
        let result = extract_trigger_pure("", "Andy");
        assert!(result.is_none());
    }

    #[test]
    fn test_is_duplicate_message_pure() {
        let msg = NewMessage {
            id: "1".to_string(),
            chat_jid: "feishu:chat:123".to_string(),
            sender: "user1".to_string(),
            sender_name: "User".to_string(),
            content: "hello".to_string(),
            timestamp: "2025-01-01T00:00:00Z".to_string(),
        };

        assert!(!is_duplicate_message_pure(&msg, &HashMap::new()));
        
        let mut ids = HashMap::new();
        ids.insert("feishu:chat:123".to_string(), "1".to_string());
        assert!(is_duplicate_message_pure(&msg, &ids));
        
        ids.insert("feishu:chat:123".to_string(), "0".to_string());
        assert!(!is_duplicate_message_pure(&msg, &ids));
    }

    #[test]
    fn test_is_allowed_chat_pure() {
        let allowed = vec!["chat_123".to_string(), "chat_456".to_string()];

        // Open policy
        assert!(is_allowed_chat_pure(
            "chat_999",
            FeishuGroupPolicy::Open,
            &allowed
        ));

        // Disabled policy
        assert!(!is_allowed_chat_pure(
            "chat_123",
            FeishuGroupPolicy::Disabled,
            &allowed
        ));

        // Allowlist policy with match
        assert!(is_allowed_chat_pure(
            "chat_123",
            FeishuGroupPolicy::Allowlist,
            &allowed
        ));

        // Allowlist policy without match
        assert!(!is_allowed_chat_pure(
            "chat_999",
            FeishuGroupPolicy::Allowlist,
            &allowed
        ));

        // Empty allowlist means all allowed in allowlist mode
        assert!(is_allowed_chat_pure(
            "any_chat",
            FeishuGroupPolicy::Allowlist,
            &[]
        ));
    }

    #[test]
    fn test_should_auto_start_feishu() {
        // Save original values
        let original_app_id = std::env::var("FEISHU_APP_ID").ok();
        let original_app_secret = std::env::var("FEISHU_APP_SECRET").ok();

        // Remove variables
        std::env::remove_var("FEISHU_APP_ID");
        std::env::remove_var("FEISHU_APP_SECRET");
        assert!(!should_auto_start_feishu());

        // Set only app_id
        std::env::set_var("FEISHU_APP_ID", "test_app_id");
        assert!(!should_auto_start_feishu());

        // Set both
        std::env::set_var("FEISHU_APP_SECRET", "test_secret");
        assert!(should_auto_start_feishu());

        // Restore original values
        match original_app_id {
            Some(val) => std::env::set_var("FEISHU_APP_ID", val),
            None => std::env::remove_var("FEISHU_APP_ID"),
        }
        match original_app_secret {
            Some(val) => std::env::set_var("FEISHU_APP_SECRET", val),
            None => std::env::remove_var("FEISHU_APP_SECRET"),
        }
    }

    #[test]
    fn test_load_router_state() {
        use std::env;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        env::set_var("NUCLAW_HOME", temp_dir.path());

        let state = load_router_state();
        assert!(state.last_message_ids.is_empty());

        env::remove_var("NUCLAW_HOME");
    }

    #[test]
    fn test_load_registered_chats() {
        let chats = load_registered_chats();
        assert!(chats.is_empty());
    }
}
