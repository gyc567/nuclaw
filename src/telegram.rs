//! Telegram Integration for NuClaw
//!
//! Provides Telegram Bot connectivity via Bot API with webhook support.
//! Follows OpenClaw Telegram specification for message handling.

use crate::config::{assistant_name, data_dir};
use crate::container_runner::run_container;
use crate::db::Database;
use crate::error::{NuClawError, Result};
use crate::types::{ContainerInput, NewMessage, RegisteredGroup, RouterState};
use crate::utils::json::{load_json, save_json};
use axum::routing::{get, post};
use axum::Json;
use axum::Router;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{timeout, Duration};
use tracing::{debug, error, info};

/// Default text chunk limit: 4000 characters
const DEFAULT_TEXT_CHUNK_LIMIT: usize = 4000;

/// DM policy enumeration
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum DMPolicy {
    #[serde(rename = "pairing")]
    Pairing,
    #[serde(rename = "allowlist")]
    Allowlist,
    #[serde(rename = "open")]
    Open,
    #[serde(rename = "disabled")]
    Disabled,
}

/// Group policy enumeration
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum GroupPolicy {
    #[serde(rename = "open")]
    Open,
    #[serde(rename = "allowlist")]
    Allowlist,
    #[serde(rename = "disabled")]
    Disabled,
}

/// Telegram Update object (Telegram Bot API)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelegramUpdate {
    pub update_id: i64,
    pub message: Option<TelegramMessage>,
    pub edited_message: Option<TelegramMessage>,
}

/// Telegram User object
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelegramUser {
    pub id: i64,
    pub is_bot: bool,
    pub first_name: String,
    pub last_name: Option<String>,
    pub username: Option<String>,
}

/// Telegram Chat object
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelegramChat {
    pub id: i64,
    #[serde(rename = "type")]
    pub chat_type: String,
    pub title: Option<String>,
}

/// Telegram Message object
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelegramMessage {
    pub message_id: i64,
    pub from: Option<TelegramUser>,
    pub chat: TelegramChat,
    pub date: i64,
    pub text: Option<String>,
}

/// Telegram client state
pub struct TelegramClient {
    /// API URL
    api_url: String,
    /// Webhook path
    webhook_path: String,
    /// DM policy
    dm_policy: DMPolicy,
    /// Group policy
    group_policy: GroupPolicy,
    /// Text chunk limit
    text_chunk_limit: usize,
    /// Allowed group IDs
    allowed_groups: Vec<String>,
    /// Reference to registered groups
    registered_groups: HashMap<String, RegisteredGroup>,
    /// Router state for message deduplication
    router_state: RouterState,
    /// Database connection
    db: Database,
    /// Assistant name for trigger detection
    assistant_name: String,
}

impl TelegramClient {
    /// Create a new Telegram client
    pub fn new(db: Database) -> Result<Self> {
        let bot_token = std::env::var("TELEGRAM_BOT_TOKEN").map_err(|_| NuClawError::Config {
            message: "TELEGRAM_BOT_TOKEN not set".to_string(),
        })?;

        let api_url = format!("https://api.telegram.org/bot{}", bot_token);

        Ok(Self {
            api_url,
            webhook_path: std::env::var("TELEGRAM_WEBHOOK_PATH")
                .unwrap_or_else(|_| "telegram-webhook".to_string()),
            dm_policy: DMPolicy::parse(
                &std::env::var("TELEGRAM_DM_POLICY").unwrap_or_else(|_| "pairing".to_string()),
            ),
            group_policy: GroupPolicy::parse(
                &std::env::var("TELEGRAM_GROUP_POLICY").unwrap_or_else(|_| "allowlist".to_string()),
            ),
            text_chunk_limit: std::env::var("TELEGRAM_TEXT_CHUNK_LIMIT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(DEFAULT_TEXT_CHUNK_LIMIT),
            allowed_groups: std::env::var("TELEGRAM_WHITELIST_GROUPS")
                .ok()
                .map(|s| s.split(',').map(|v| v.trim().to_string()).collect())
                .unwrap_or_default(),
            registered_groups: load_registered_groups(),
            router_state: load_router_state(),
            db,
            assistant_name: assistant_name(),
        })
    }

    /// Connect to Telegram
    pub async fn connect(&mut self) -> Result<()> {
        info!("Connecting to Telegram...");

        // Check webhook URL
        let webhook_url = std::env::var("TELEGRAM_WEBHOOK_URL").ok();

        if let Some(url) = webhook_url {
            self.set_webhook(&url).await?;
            info!("Webhook set to: {}", url);
        } else {
            info!("No webhook URL configured, using polling mode");
        }

        Ok(())
    }

    /// Set webhook URL
    async fn set_webhook(&self, url: &str) -> Result<()> {
        let full_url = format!("{}/webhook/{}", url, self.webhook_path);
        let response = reqwest::Client::new()
            .post(format!("{}/setWebhook", self.api_url))
            .json(&serde_json::json!({ "url": full_url }))
            .send()
            .await
            .map_err(|e| NuClawError::Telegram {
                message: format!("Failed to set webhook: {}", e),
            })?;

        if response.status() != 200 {
            return Err(NuClawError::Telegram {
                message: format!(
                    "Webhook setup failed: {}",
                    response.text().await.unwrap_or_default()
                ),
            });
        }

        Ok(())
    }

    /// Start webhook server
    pub async fn start_webhook_server(self) -> Result<()> {
        let addr: SocketAddr = std::env::var("TELEGRAM_WEBHOOK_BIND")
            .unwrap_or_else(|_| "0.0.0.0:8787".to_string())
            .parse()
            .map_err(|_| NuClawError::Config {
                message: "Invalid TELEGRAM_WEBHOOK_BIND".to_string(),
            })?;

        let client = Arc::new(Mutex::new(self));
        let webhook_path = client.lock().await.webhook_path.clone();

        let app = Router::new()
            .route(&format!("/{}", webhook_path), post(handle_telegram_webhook))
            .route("/health", get(health_check))
            .with_state(client.clone());

        info!("Starting Telegram webhook server on {}", addr);

        let listener =
            tokio::net::TcpListener::bind(&addr)
                .await
                .map_err(|e| NuClawError::Telegram {
                    message: format!("Failed to bind to {}: {}", addr, e),
                })?;

        axum::serve(listener, app)
            .await
            .map_err(|e| NuClawError::Telegram {
                message: format!("Webhook server error: {}", e),
            })?;

        Ok(())
    }

    /// Handle a Telegram update
    pub async fn handle_update(&mut self, update: &TelegramUpdate) -> Result<Option<String>> {
        let message = match &update.message {
            Some(msg) => msg,
            None => {
                debug!("Received update without message, skipping");
                return Ok(None);
            }
        };

        let new_message = self.parse_telegram_message(message).await?;
        self.handle_message(&new_message).await
    }

    /// Parse Telegram message to unified format
    async fn parse_telegram_message(&self, msg: &TelegramMessage) -> Result<NewMessage> {
        let sender = msg
            .from
            .as_ref()
            .map(|u| u.id.to_string())
            .unwrap_or_default();
        let sender_name = msg
            .from
            .as_ref()
            .map(|u| {
                if let Some(username) = &u.username {
                    username.clone()
                } else {
                    u.first_name.clone()
                }
            })
            .unwrap_or_else(|| "Unknown".to_string());

        let chat_jid = format!("telegram:group:{}", msg.chat.id);

        let content = msg.text.clone().unwrap_or_else(|| "".to_string());

        Ok(NewMessage {
            id: msg.message_id.to_string(),
            chat_jid,
            sender,
            sender_name,
            content,
            timestamp: msg.date.to_string(),
        })
    }

    /// Handle a single message
    pub async fn handle_message(&mut self, msg: &NewMessage) -> Result<Option<String>> {
        if self.is_duplicate_message(msg).await {
            debug!("Skipping duplicate message: {}", msg.id);
            return Ok(None);
        }

        self.update_router_state(msg).await;
        self.store_message(msg).await?;

        // Check if it's a private message
        if msg.chat_jid.starts_with("telegram:group:-") || !msg.chat_jid.contains(":group:") {
            if !self.check_dm_policy(&msg.sender).await? {
                debug!("Message from unauthorized user: {}", msg.sender);
                return Ok(None);
            }
        }

        // Check if registered group
        if !self.is_allowed_group(&msg.chat_jid).await? {
            debug!("Message from unregistered group: {}", msg.chat_jid);
            return Ok(None);
        }

        let (_, content) = match self.extract_trigger(&msg.content).await {
            Some((_, c)) => (String::new(), c),
            None => return Ok(None),
        };

        info!(
            "Received message from {}: {}",
            msg.sender_name,
            truncate(&content, 50)
        );

        let group_folder =
            self.get_group_folder(&msg.chat_jid)
                .await
                .ok_or_else(|| NuClawError::Telegram {
                    message: format!("Group not found: {}", msg.chat_jid),
                })?;

        let session_id = format!("telegram_{}", msg.id);
        let input = ContainerInput {
            prompt: content,
            session_id: Some(session_id.clone()),
            group_folder,
            chat_jid: msg.chat_jid.clone(),
            is_main: true,
            is_scheduled_task: false,
        };

        let result = timeout(Duration::from_secs(300), run_container(input)).await;

        match result {
            Ok(Ok(output)) => {
                if let Some(response) = output.result {
                    let chat_id = self.extract_chat_id(&msg.chat_jid)?;
                    self.send_message(&chat_id.to_string(), &response).await?;
                    return Ok(Some(response));
                }
            }
            Ok(Err(e)) => {
                error!("Container error: {}", e);
                let chat_id = self.extract_chat_id(&msg.chat_jid)?;
                self.send_message(&chat_id.to_string(), &format!("Error: {}", e))
                    .await?;
            }
            Err(_) => {
                error!("Container timeout");
                let chat_id = self.extract_chat_id(&msg.chat_jid)?;
                self.send_message(&chat_id.to_string(), "Sorry, the request timed out.")
                    .await?;
            }
        }

        Ok(None)
    }

    /// Send a message to a chat
    pub async fn send_message(&self, chat_id: &str, text: &str) -> Result<()> {
        let cid: i64 = chat_id.parse().map_err(|_| NuClawError::Telegram {
            message: format!("Invalid chat_id: {}", chat_id),
        })?;

        let chunks = self.chunk_text(text);

        for chunk in chunks {
            let payload = serde_json::json!({
                "chat_id": cid,
                "text": chunk,
                "parse_mode": "HTML"
            });

            let response = reqwest::Client::new()
                .post(&format!("{}/sendMessage", self.api_url))
                .json(&payload)
                .timeout(Duration::from_secs(30))
                .send()
                .await
                .map_err(|e| NuClawError::Telegram {
                    message: format!("Failed to send message: {}", e),
                })?;

            if !response.status().is_success() {
                let error = response.text().await.unwrap_or_default();
                return Err(NuClawError::Telegram {
                    message: format!("Failed to send message: {}", error),
                });
            }
        }

        Ok(())
    }

    /// Chunk text into smaller pieces
    fn chunk_text(&self, text: &str) -> Vec<String> {
        chunk_text_pure(text, self.text_chunk_limit)
    }

    /// Check DM policy
    async fn check_dm_policy(&self, _user_id: &str) -> Result<bool> {
        match self.dm_policy {
            DMPolicy::Disabled => Ok(false),
            DMPolicy::Open => Ok(true),
            DMPolicy::Allowlist | DMPolicy::Pairing => {
                // Allow for now (can be extended with database check)
                Ok(true)
            }
        }
    }

    /// Check if group is allowed
    async fn is_allowed_group(&self, chat_jid: &str) -> Result<bool> {
        match self.group_policy {
            GroupPolicy::Disabled => Ok(false),
            GroupPolicy::Open => Ok(true),
            GroupPolicy::Allowlist => {
                // Extract chat_id from jid
                if let Some(chat_id) = chat_jid.strip_prefix("telegram:group:") {
                    let result = self
                        .allowed_groups
                        .iter()
                        .any(|g| g == chat_id || g == &format!("-{}", chat_id));
                    Ok(result)
                } else {
                    Ok(false)
                }
            }
        }
    }

    /// Get group folder for a chat JID
    async fn get_group_folder(&self, jid: &str) -> Option<String> {
        self.registered_groups.get(jid).map(|g| g.folder.clone())
    }

    /// Extract chat ID from jid
    fn extract_chat_id(&self, jid: &str) -> Result<String> {
        extract_chat_id_pure(jid).ok_or_else(|| NuClawError::Telegram {
            message: format!("Invalid telegram jid format: {}", jid),
        })
    }

    /// Check if message is duplicate
    async fn is_duplicate_message(&self, msg: &NewMessage) -> bool {
        let last_timestamp = &self.router_state.last_timestamp;
        let last_agent = self.router_state.last_agent_timestamp.get(&msg.chat_jid);

        if last_timestamp == &msg.timestamp {
            return true;
        }

        if let Some(agent_ts) = last_agent {
            if agent_ts == &msg.timestamp {
                return true;
            }
        }

        false
    }

    /// Update router state after processing
    async fn update_router_state(&mut self, msg: &NewMessage) {
        self.router_state.last_timestamp = msg.timestamp.clone();
        self.router_state
            .last_agent_timestamp
            .insert(msg.chat_jid.clone(), msg.timestamp.clone());

        let state_path = data_dir().join("router_state.json");
        let _ = save_json(&state_path, &self.router_state);
    }

    /// Store message in database
    async fn store_message(&self, msg: &NewMessage) -> Result<()> {
        let conn = self
            .db
            .get_connection()
            .map_err(|e| NuClawError::Database {
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
        ).map_err(|e| NuClawError::Database {
            message: format!("Failed to store message: {}", e),
        })?;

        Ok(())
    }

    /// Extract trigger and content from message
    async fn extract_trigger(&self, content: &str) -> Option<(String, String)> {
        let trigger_pattern = format!("@{}", self.assistant_name);

        if let Some(idx) = content.find(&trigger_pattern) {
            let after = &content[idx + trigger_pattern.len()..];
            let c = after.trim().to_string();
            return Some((trigger_pattern, c));
        }

        None
    }
}

// Webhook handler
async fn handle_telegram_webhook(
    client: axum::extract::State<Arc<Mutex<TelegramClient>>>,
    Json(update): Json<TelegramUpdate>,
) -> &'static str {
    let mut client = client.lock().await;
    if let Err(e) = client.handle_update(&update).await {
        error!("Failed to handle telegram update: {}", e);
    }
    "OK"
}

async fn health_check() -> &'static str {
    "OK"
}

// Helper functions

/// Load router state from file
pub fn load_router_state() -> RouterState {
    let state_path = data_dir().join("router_state.json");
    load_json(
        &state_path,
        RouterState {
            last_timestamp: String::new(),
            last_agent_timestamp: HashMap::new(),
        },
    )
}

/// Load registered groups from file
pub fn load_registered_groups() -> HashMap<String, RegisteredGroup> {
    let path = data_dir().join("registered_groups.json");
    load_json(&path, HashMap::new())
}

/// Helper to truncate strings
fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}

/// Chunk text into smaller pieces (pure function)
pub fn chunk_text_pure(text: &str, chunk_limit: usize) -> Vec<String> {
    if text.len() <= chunk_limit {
        return vec![text.to_string()];
    }

    let mut chunks = Vec::new();
    let mut current = String::new();

    for paragraph in text.split("\n\n") {
        if current.len() + paragraph.len() + 2 > chunk_limit {
            if !current.is_empty() {
                chunks.push(current);
            }
            current = paragraph.to_string();
        } else {
            if !current.is_empty() {
                current.push_str("\n\n");
            }
            current.push_str(paragraph);
        }
    }

    if !current.is_empty() {
        chunks.push(current);
    }

    chunks
}

/// Extract chat ID from jid (pure function)
pub fn extract_chat_id_pure(jid: &str) -> Option<String> {
    jid.strip_prefix("telegram:group:").map(|s| s.to_string())
}

/// Check if message is duplicate (pure function)
pub fn is_duplicate_message_pure(
    msg: &NewMessage,
    last_timestamp: &str,
    last_agent_timestamps: &std::collections::HashMap<String, String>,
) -> bool {
    if last_timestamp == msg.timestamp {
        return true;
    }

    if let Some(agent_ts) = last_agent_timestamps.get(&msg.chat_jid) {
        if agent_ts == &msg.timestamp {
            return true;
        }
    }

    false
}

/// Check if group is allowed (pure function)
pub fn is_allowed_group_pure(
    chat_jid: &str,
    policy: GroupPolicy,
    allowed_groups: &[String],
) -> bool {
    match policy {
        GroupPolicy::Disabled => false,
        GroupPolicy::Open => true,
        GroupPolicy::Allowlist => {
            if let Some(chat_id) = chat_jid.strip_prefix("telegram:group:") {
                allowed_groups
                    .iter()
                    .any(|g| g == chat_id || g == &format!("-{}", chat_id))
            } else {
                false
            }
        }
    }
}

// Trait implementations for enums

impl DMPolicy {
    pub fn parse(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "pairing" => DMPolicy::Pairing,
            "allowlist" => DMPolicy::Allowlist,
            "open" => DMPolicy::Open,
            "disabled" => DMPolicy::Disabled,
            _ => DMPolicy::Pairing,
        }
    }
}

impl GroupPolicy {
    pub fn parse(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "open" => GroupPolicy::Open,
            "allowlist" => GroupPolicy::Allowlist,
            "disabled" => GroupPolicy::Disabled,
            _ => GroupPolicy::Allowlist,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_test_dirs() {
        use std::fs;
        use crate::config::store_dir;
        let store = store_dir();
        if !store.exists() {
            let _ = fs::create_dir_all(&store);
        }
    }

    #[test]
    fn test_parse_telegram_update() {
        let json = r#"{
            "update_id": 123,
            "message": {
                "message_id": 456,
                "from": {"id": 789, "is_bot": false, "first_name": "Test"},
                "chat": {"id": -100123, "type": "supergroup", "title": "Test Group"},
                "date": 1234567890,
                "text": "@Andy hello"
            }
        }"#;

        let update: TelegramUpdate = serde_json::from_str(json).unwrap();
        assert_eq!(update.update_id, 123);
        assert!(update.message.is_some());
    }

    #[test]
    fn test_extract_trigger_telegram() {
        setup_test_dirs();
        let client = TelegramClient {
            api_url: "https://api.telegram.org/bottest".to_string(),
            webhook_path: "webhook".to_string(),
            dm_policy: DMPolicy::Pairing,
            group_policy: GroupPolicy::Allowlist,
            text_chunk_limit: 4000,
            allowed_groups: vec![],
            registered_groups: HashMap::new(),
            router_state: RouterState::default(),
            db: Database::new().unwrap(),
            assistant_name: "Andy".to_string(),
        };

        let result = std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(client.extract_trigger("@Andy hello world"))
        })
        .join()
        .unwrap();

        assert!(result.is_some());
        let (trigger, content) = result.unwrap();
        assert_eq!(trigger, "@Andy");
        assert_eq!(content, "hello world");
    }

    #[test]
    fn test_dm_policy_from_str() {
        assert_eq!(DMPolicy::parse("pairing"), DMPolicy::Pairing);
        assert_eq!(DMPolicy::parse("allowlist"), DMPolicy::Allowlist);
        assert_eq!(DMPolicy::parse("open"), DMPolicy::Open);
        assert_eq!(DMPolicy::parse("disabled"), DMPolicy::Disabled);
        assert_eq!(DMPolicy::parse("unknown"), DMPolicy::Pairing);
    }

    #[test]
    fn test_group_policy_from_str() {
        assert_eq!(GroupPolicy::parse("open"), GroupPolicy::Open);
        assert_eq!(GroupPolicy::parse("allowlist"), GroupPolicy::Allowlist);
        assert_eq!(GroupPolicy::parse("disabled"), GroupPolicy::Disabled);
        assert_eq!(GroupPolicy::parse("unknown"), GroupPolicy::Allowlist);
    }

    #[test]
    fn test_text_chunking_short() {
        setup_test_dirs();
        let client = TelegramClient {
            api_url: "https://api.telegram.org/bottest".to_string(),
            webhook_path: "webhook".to_string(),
            dm_policy: DMPolicy::Open,
            group_policy: GroupPolicy::Open,
            text_chunk_limit: 4000,
            allowed_groups: vec![],
            registered_groups: HashMap::new(),
            router_state: RouterState::default(),
            db: Database::new().unwrap(),
            assistant_name: "Andy".to_string(),
        };

        let chunks = client.chunk_text("short text");
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], "short text");
    }

    #[test]
    fn test_text_chunking_long() {
        setup_test_dirs();
        let client = TelegramClient {
            api_url: "https://api.telegram.org/bottest".to_string(),
            webhook_path: "webhook".to_string(),
            dm_policy: DMPolicy::Open,
            group_policy: GroupPolicy::Open,
            text_chunk_limit: 50,
            allowed_groups: vec![],
            registered_groups: HashMap::new(),
            router_state: RouterState::default(),
            db: Database::new().unwrap(),
            assistant_name: "Andy".to_string(),
        };

        // Create a text longer than 50 characters with multiple paragraphs
        let long_text = "This is paragraph one that is longer than fifty characters.\n\nThis is paragraph two that is also quite long and should create multiple chunks.\n\nThis is the third paragraph to ensure we have enough content.";
        let chunks = client.chunk_text(long_text);
        assert!(
            chunks.len() > 1,
            "Expected multiple chunks but got {:?}",
            chunks
        );
    }

    #[test]
    fn test_chunk_text_pure_short() {
        let chunks = chunk_text_pure("short text", 4000);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], "short text");
    }

    #[test]
    fn test_chunk_text_pure_exact_limit() {
        let text = "a".repeat(4000);
        let chunks = chunk_text_pure(&text, 4000);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].len(), 4000);
    }

    #[test]
    fn test_chunk_text_pure_over_limit() {
        let text = "a".repeat(4001);
        let chunks = chunk_text_pure(&text, 4000);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], text);
    }

    #[test]
    fn test_chunk_text_pure_empty() {
        let chunks = chunk_text_pure("", 4000);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], "");
    }

    #[test]
    fn test_extract_chat_id_pure_valid() {
        assert_eq!(
            extract_chat_id_pure("telegram:group:123456"),
            Some("123456".to_string())
        );
        assert_eq!(
            extract_chat_id_pure("telegram:group:-100123"),
            Some("-100123".to_string())
        );
    }

    #[test]
    fn test_extract_chat_id_pure_invalid() {
        assert_eq!(extract_chat_id_pure("invalid:jid"), None);
        assert_eq!(extract_chat_id_pure("whatsapp:group:123"), None);
        assert_eq!(extract_chat_id_pure(""), None);
    }

    #[test]
    fn test_is_duplicate_message_pure() {
        let msg = NewMessage {
            id: "1".to_string(),
            chat_jid: "telegram:group:123".to_string(),
            sender: "user1".to_string(),
            sender_name: "User".to_string(),
            content: "hello".to_string(),
            timestamp: "2025-01-01T00:00:00Z".to_string(),
        };

        let mut agent_ts = std::collections::HashMap::new();
        agent_ts.insert(
            "telegram:group:123".to_string(),
            "2025-01-01T00:00:00Z".to_string(),
        );

        assert!(is_duplicate_message_pure(
            &msg,
            "2025-01-01T00:00:00Z",
            &HashMap::new()
        ));
        assert!(is_duplicate_message_pure(&msg, "old", &agent_ts));
        assert!(!is_duplicate_message_pure(&msg, "old", &HashMap::new()));
    }

    #[test]
    fn test_is_allowed_group_pure() {
        let allowed = vec!["123".to_string(), "-456".to_string()];

        assert!(is_allowed_group_pure(
            "telegram:group:123",
            GroupPolicy::Open,
            &[]
        ));
        assert!(!is_allowed_group_pure(
            "telegram:group:123",
            GroupPolicy::Disabled,
            &[]
        ));
        assert!(is_allowed_group_pure(
            "telegram:group:123",
            GroupPolicy::Allowlist,
            &allowed
        ));
        assert!(is_allowed_group_pure(
            "telegram:group:456",
            GroupPolicy::Allowlist,
            &allowed
        ));
        assert!(!is_allowed_group_pure(
            "telegram:group:789",
            GroupPolicy::Allowlist,
            &allowed
        ));
    }

    #[test]
    fn test_truncate_telegram() {
        assert_eq!(truncate("hello", 10), "hello");
        assert_eq!(truncate("hello world", 8), "hello...");
        assert_eq!(truncate("test", 3), "...");
        assert_eq!(truncate("", 5), "");
    }

    #[test]
    fn test_telegram_structs_serialization() {
        let user = TelegramUser {
            id: 123,
            is_bot: false,
            first_name: "Test".to_string(),
            last_name: Some("User".to_string()),
            username: Some("testuser".to_string()),
        };
        let json = serde_json::to_string(&user).unwrap();
        assert!(json.contains("Test"));

        let chat = TelegramChat {
            id: -100123,
            chat_type: "supergroup".to_string(),
            title: Some("Test Group".to_string()),
        };
        let json = serde_json::to_string(&chat).unwrap();
        assert!(json.contains("supergroup"));

        let msg = TelegramMessage {
            message_id: 456,
            from: Some(user),
            chat,
            date: 1234567890,
            text: Some("hello".to_string()),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("hello"));
    }
}
