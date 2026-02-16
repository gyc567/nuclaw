//! WhatsApp Integration for NuClaw
//!
//! Provides WhatsApp connectivity via external WhatsApp MCP Server or HTTP API.

use crate::config::{assistant_name, data_dir, store_dir};
use crate::container_runner::run_container;
use crate::db::Database;
use crate::error::{NuClawError, Result};
use crate::types::{ContainerInput, NewMessage, RegisteredGroup, RouterState};
use crate::utils::json::{load_json, save_json};
use std::collections::HashMap;
use tokio::time::{timeout, Duration};
use tracing::{debug, error, info};

/// Default WhatsApp poll interval: 2 seconds
const DEFAULT_WHATSAPP_POLL_INTERVAL_MS: u64 = 2000;

/// WhatsApp client state
pub struct WhatsAppClient {
    /// Connection status
    pub connected: bool,
    /// Last QR code for authentication
    pub last_qr: Option<String>,
    /// Reference to registered groups
    registered_groups: HashMap<String, RegisteredGroup>,
    /// Router state for message deduplication
    router_state: RouterState,
    /// Database connection
    db: Database,
    /// Assistant name for trigger detection
    assistant_name: String,
}

impl WhatsAppClient {
    /// Create a new WhatsApp client
    pub fn new(db: Database) -> Self {
        Self {
            connected: false,
            last_qr: None,
            registered_groups: load_registered_groups(),
            router_state: load_router_state(),
            db,
            assistant_name: assistant_name(),
        }
    }

    /// Connect to WhatsApp
    pub async fn connect(&mut self) -> Result<()> {
        info!("Connecting to WhatsApp...");

        // Check if we need authentication
        if self.needs_authentication().await? {
            info!("Authentication required, generating QR code...");
            self.request_qr_code().await?;
        } else {
            info!("Using cached authentication");
            self.connected = true;
        }

        Ok(())
    }

    /// Check if authentication is needed
    async fn needs_authentication(&self) -> Result<bool> {
        let creds_path = store_dir().join("auth").join("creds.json");

        // If creds file exists and is valid, no auth needed
        if creds_path.exists() {
            if let Ok(metadata) = std::fs::metadata(&creds_path) {
                let modified = metadata
                    .modified()
                    .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                let duration = modified.elapsed().unwrap_or(std::time::Duration::ZERO);
                if duration.as_secs() < 24 * 60 * 60 {
                    return Ok(false);
                }
            }
        }

        Ok(true)
    }

    /// Request QR code for authentication
    async fn request_qr_code(&mut self) -> Result<()> {
        let mcp_url = get_mcp_url()?;

        let response = reqwest::Client::new()
            .post(format!("{}/auth/qr", mcp_url))
            .timeout(Duration::from_secs(30))
            .send()
            .await
            .map_err(|e| NuClawError::WhatsApp {
                message: format!("Failed to request QR code: {}", e),
            })?;

        if response.status() == 200 {
            let qr_data: serde_json::Value =
                response.json().await.map_err(|e| NuClawError::WhatsApp {
                    message: format!("Failed to parse QR response: {}", e),
                })?;

            if let Some(qr) = qr_data.get("qr").and_then(|v| v.as_str()) {
                self.last_qr = Some(qr.to_string());
                info!("QR code received (display in terminal)");
                info!("Scan with WhatsApp to authenticate");
            }
        }

        Ok(())
    }

    /// Start listening for messages
    pub async fn start_message_listener(&mut self) {
        let mut interval =
            tokio::time::interval(Duration::from_millis(DEFAULT_WHATSAPP_POLL_INTERVAL_MS));

        info!("Starting message listener...");

        loop {
            interval.tick().await;

            if let Err(e) = self.poll_messages().await {
                error!("Error polling messages: {}", e);
            }
        }
    }

    /// Poll for new messages
    async fn poll_messages(&mut self) -> Result<()> {
        let mcp_url = get_mcp_url()?;

        let response = reqwest::Client::new()
            .get(format!("{}/messages", mcp_url))
            .timeout(Duration::from_secs(30))
            .send()
            .await
            .map_err(|e| NuClawError::WhatsApp {
                message: format!("Failed to poll messages: {}", e),
            })?;

        if response.status() == 200 {
            let messages: Vec<NewMessage> =
                response.json().await.map_err(|e| NuClawError::WhatsApp {
                    message: format!("Failed to parse messages: {}", e),
                })?;

            for msg in messages {
                self.handle_message(&msg).await?;
            }
        }

        Ok(())
    }

    /// Handle a single message
    pub async fn handle_message(&mut self, msg: &NewMessage) -> Result<Option<String>> {
        if self.is_duplicate_message(msg).await {
            debug!("Skipping duplicate message: {}", msg.id);
            return Ok(None);
        }

        self.update_router_state(msg).await;
        self.store_message(msg).await?;

        if !self.is_registered_group(&msg.chat_jid).await {
            debug!("Message from unregistered group: {}", msg.chat_jid);
            return Ok(None);
        }

        let (_, content) = match self.extract_trigger(&msg.content).await {
            Some((_, c)) => (String::new(), c),
            None => return Ok(None),
        };

        info!(
            "Received message from {}: {}",
            msg.sender,
            truncate(&content, 50)
        );

        let group_folder =
            self.get_group_folder(&msg.chat_jid)
                .await
                .ok_or_else(|| NuClawError::WhatsApp {
                    message: format!("Group not found: {}", msg.chat_jid),
                })?;

        let session_id = format!("whatsapp_{}", msg.id);
        let input = ContainerInput {
            prompt: content,
            session_id: Some(session_id.clone()),
            group_folder,
            chat_jid: msg.chat_jid.clone(),
            is_main: msg.chat_jid.ends_with("@s.whatsapp.net"),
            is_scheduled_task: false,
        };

        let result = timeout(Duration::from_secs(300), run_container(input)).await;

        match result {
            Ok(Ok(output)) => {
                if let Some(response) = output.result {
                    self.send_message(&msg.chat_jid, &response).await?;
                    return Ok(Some(response));
                }
            }
            Ok(Err(e)) => {
                error!("Container error: {}", e);
                self.send_message(&msg.chat_jid, &format!("Error: {}", e))
                    .await?;
            }
            Err(_) => {
                error!("Container timeout");
                self.send_message(&msg.chat_jid, "Sorry, the request timed out.")
                    .await?;
            }
        }

        Ok(None)
    }

    /// Send a message
    pub async fn send_message(&self, jid: &str, content: &str) -> Result<()> {
        let mcp_url = get_mcp_url()?;

        let payload = serde_json::json!({
            "jid": jid,
            "message": content,
        });

        let response = reqwest::Client::new()
            .post(format!("{}/messages/send", mcp_url))
            .json(&payload)
            .timeout(Duration::from_secs(30))
            .send()
            .await
            .map_err(|e| NuClawError::WhatsApp {
                message: format!("Failed to send message: {}", e),
            })?;

        if !response.status().is_success() {
            return Err(NuClawError::WhatsApp {
                message: format!("Failed to send message: status {}", response.status()),
            });
        }

        Ok(())
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

    /// Check if a chat is a registered group
    async fn is_registered_group(&self, jid: &str) -> bool {
        self.registered_groups.contains_key(jid)
    }

    /// Get group folder for a chat JID
    async fn get_group_folder(&self, jid: &str) -> Option<String> {
        self.registered_groups.get(jid).map(|g| g.folder.clone())
    }

    /// Extract trigger and content from message
    async fn extract_trigger(&self, content: &str) -> Option<(String, String)> {
        extract_trigger_pure(content, &self.assistant_name)
    }
}

// Helper functions

/// Get WhatsApp MCP URL from environment
fn get_mcp_url() -> Result<String> {
    std::env::var("WHATSAPP_MCP_URL").map_err(|_| NuClawError::Config {
        message: "WHATSAPP_MCP_URL not set".to_string(),
    })
}

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

/// Start the authentication flow
pub async fn start_auth_flow() {
    let auth_path = store_dir().join("auth");
    std::fs::create_dir_all(&auth_path).ok();
    info!("Use WHATSAPP_MCP_URL to configure WhatsApp connection");
}

/// Helper to truncate strings
fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}

/// Extract trigger and content from message (pure function)
pub fn extract_trigger_pure(content: &str, assistant_name: &str) -> Option<(String, String)> {
    let trigger_pattern = format!("@{}", assistant_name);

    if let Some(idx) = content.find(&trigger_pattern) {
        let after = &content[idx + trigger_pattern.len()..];
        let c = after.trim().to_string();
        return Some((trigger_pattern, c));
    }

    None
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

/// Check if message is from a private chat
pub fn is_private_chat(jid: &str) -> bool {
    jid.ends_with("@s.whatsapp.net")
}

/// Get group name from JID
pub fn get_group_name_from_jid(jid: &str) -> Option<String> {
    jid.split('@').next().map(|s| s.to_string())
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
    fn test_truncate_short() {
        assert_eq!(truncate("hello", 10), "hello");
    }

    #[test]
    fn test_truncate_long() {
        assert_eq!(truncate("hello world", 8), "hello...");
    }

    #[test]
    fn test_extract_trigger_with_at() {
        setup_test_dirs();
        let client = WhatsAppClient {
            connected: false,
            last_qr: None,
            registered_groups: HashMap::new(),
            router_state: RouterState::default(),
            db: Database::new().unwrap(),
            assistant_name: "Andy".to_string(),
        };

        let result = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(client.extract_trigger("@Andy hello world"));

        assert!(result.is_some());
        let (trigger, content) = result.unwrap();
        assert_eq!(trigger, "@Andy");
        assert_eq!(content, "hello world");
    }

    #[test]
    fn test_extract_trigger_without_at() {
        setup_test_dirs();
        let client = WhatsAppClient {
            connected: false,
            last_qr: None,
            registered_groups: HashMap::new(),
            router_state: RouterState::default(),
            db: Database::new().unwrap(),
            assistant_name: "Andy".to_string(),
        };

        let result = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(client.extract_trigger("hello world"));

        assert!(result.is_none());
    }

    #[test]
    fn test_extract_trigger_pure_basic() {
        let result = extract_trigger_pure("@Andy hello world", "Andy");
        assert!(result.is_some());
        let (trigger, content) = result.unwrap();
        assert_eq!(trigger, "@Andy");
        assert_eq!(content, "hello world");
    }

    #[test]
    fn test_extract_trigger_pure_no_trigger() {
        assert!(extract_trigger_pure("hello world", "Andy").is_none());
        assert!(extract_trigger_pure("", "Andy").is_none());
    }

    #[test]
    fn test_extract_trigger_pure_different_name() {
        let result = extract_trigger_pure("@Bob hello", "Bob");
        assert!(result.is_some());
        let (_, content) = result.unwrap();
        assert_eq!(content, "hello");
    }

    #[test]
    fn test_extract_trigger_pure_with_extra_spaces() {
        let result = extract_trigger_pure("@Andy   hello   world  ", "Andy");
        assert!(result.is_some());
        let (_, content) = result.unwrap();
        assert_eq!(content, "hello   world");
    }

    #[test]
    fn test_extract_trigger_pure_mid_message() {
        let result = extract_trigger_pure("hey @Andy help me", "Andy");
        assert!(result.is_some());
        let (_, content) = result.unwrap();
        assert_eq!(content, "help me");
    }

    #[test]
    fn test_is_duplicate_message_pure_whatsapp() {
        let msg = NewMessage {
            id: "1".to_string(),
            chat_jid: "123@s.whatsapp.net".to_string(),
            sender: "user1".to_string(),
            sender_name: "User".to_string(),
            content: "hello".to_string(),
            timestamp: "2025-01-01T00:00:00Z".to_string(),
        };

        let mut agent_ts = std::collections::HashMap::new();
        agent_ts.insert(
            "123@s.whatsapp.net".to_string(),
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
    fn test_is_private_chat() {
        assert!(is_private_chat("123@s.whatsapp.net"));
        assert!(!is_private_chat("123@g.us"));
        assert!(!is_private_chat("123-456@g.us"));
    }

    #[test]
    fn test_get_group_name_from_jid() {
        assert_eq!(
            get_group_name_from_jid("123@s.whatsapp.net"),
            Some("123".to_string())
        );
        assert_eq!(
            get_group_name_from_jid("mygroup@g.us"),
            Some("mygroup".to_string())
        );
        assert_eq!(get_group_name_from_jid(""), Some("".to_string()));
    }

    #[test]
    fn test_truncate_whatsapp_edge_cases() {
        assert_eq!(truncate("", 5), "");
        assert_eq!(truncate("hi", 2), "hi");
        assert_eq!(truncate("hello", 3), "...");
        assert_eq!(truncate("test", 3), "...");
    }
}
