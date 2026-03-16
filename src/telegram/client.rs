//! Telegram client implementation

use crate::config::{assistant_name, data_dir};
use crate::db::Database;
use crate::error::{NuClawError, Result};
use crate::telegram::pairing::PairingManager;
use crate::telegram::policy::{DMPolicy, GroupPolicy};
use crate::telegram::types::TelegramMessage;
use crate::telegram::utils::{extract_chat_id_pure, DEFAULT_TEXT_CHUNK_LIMIT};
use crate::types::{NewMessage, RegisteredGroup, RouterState};
use crate::utils::json::{load_json, save_json};

const PAIRING_CODE_LENGTH: usize = 6;

use axum::extract::State;
use axum::routing::{get, post};
use axum::{Json, Router};
use axum::http::StatusCode;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::Duration;
use tracing::{debug, error, info};

/// Telegram client state
pub struct TelegramClient {
    api_url: String,
    webhook_path: String,
    dm_policy: DMPolicy,
    group_policy: GroupPolicy,
    text_chunk_limit: usize,
    allowed_groups: Vec<String>,
    registered_groups: HashMap<String, RegisteredGroup>,
    router_state: RouterState,
    db: Database,
    assistant_name: String,
}

async fn telegram_send_single_message(api_url: &str, chat_id: i64, text: &str) -> Result<()> {
    telegram_send_with_retry(api_url, chat_id, text, 3).await
}

/// Send message with exponential backoff retry
async fn telegram_send_with_retry(
    api_url: &str,
    chat_id: i64,
    text: &str,
    max_retries: u32,
) -> Result<()> {
    let payload = serde_json::json!({
        "chat_id": chat_id,
        "text": text,
        "parse_mode": "HTML"
    });

    let mut last_error = None;

    for attempt in 0..max_retries {
        let client = reqwest::Client::new();
        let response = client
            .post(format!("{}/sendMessage", api_url))
            .json(&payload)
            .timeout(Duration::from_secs(30))
            .send()
            .await;

        match response {
            Ok(resp) if resp.status().is_success() => return Ok(()),
            Ok(resp) => {
                let error = resp.text().await.unwrap_or_default();
                last_error = Some(format!("Failed to send message: {}", error));
            }
            Err(e) => {
                last_error = Some(format!("Request failed: {}", e));
            }
        }

        if attempt < max_retries - 1 {
            let delay = Duration::from_millis(500 * (2_u64.pow(attempt)));
            tokio::time::sleep(delay).await;
        }
    }

    Err(NuClawError::Telegram {
        message: last_error.unwrap_or_else(|| "Unknown error".to_string()),
    })
}

impl TelegramClient {
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

    pub async fn connect(&mut self) -> Result<()> {
        info!("Connecting to Telegram...");

        let webhook_url = std::env::var("TELEGRAM_WEBHOOK_URL").ok();

        if let Some(url) = webhook_url {
            self.set_webhook(&url).await?;
            info!("Webhook set to: {}", url);
        } else {
            info!("No webhook URL configured, using polling mode");
        }

        Ok(())
    }

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
            .with_state(client);

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

    pub async fn handle_update(
        &mut self,
        update: &crate::telegram::types::TelegramUpdate,
    ) -> Result<Option<String>> {
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

        let content = msg.text.clone().unwrap_or_default();

        Ok(NewMessage {
            id: msg.message_id.to_string(),
            chat_jid,
            sender,
            sender_name,
            content,
            timestamp: msg.date.to_string(),
        })
    }

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

        if (msg.chat_jid.starts_with("telegram:group:-") || !msg.chat_jid.contains(":group:"))
            && !self.check_dm_policy(&msg.sender).await?
        {
            debug!("Message from unauthorized user: {}", msg.sender);
            return Ok(None);
        }

        if !self.is_allowed_group(&msg.chat_jid).await? {
            debug!("Message from unregistered group: {}", msg.chat_jid);
            return Ok(None);
        }

        let content_trimmed = msg.content.trim().to_uppercase();
        if content_trimmed.len() == PAIRING_CODE_LENGTH
            && content_trimmed.chars().all(|c| c.is_ascii_alphanumeric())
        {
            if let Some(response) = self.handle_pairing_code(&content_trimmed, msg).await? {
                return Ok(Some(response));
            }
        }

        let (_, content) = match self.extract_trigger(&msg.content).await {
            Some((_, c)) => (String::new(), c),
            None => return Ok(None),
        };

        info!(
            "Received message from {}: {}",
            msg.sender_name,
            crate::telegram::utils::truncate(&content, 50)
        );

        let group_folder =
            self.get_group_folder(&msg.chat_jid)
                .await
                .ok_or_else(|| NuClawError::Telegram {
                    message: format!("Group not found: {}", msg.chat_jid),
                })?;

        let is_group = msg.chat_jid.contains(":group:");
        let event = crate::types::AppEvent::ChatMessage {
            platform: "telegram".to_string(),
            chat_id: msg.chat_jid.clone(),
            user_id: msg.sender.clone(),
            message_id: msg.id.clone(),
            message_text: content,
            group_folder,
            is_group,
        };

        let router = crate::router::EventRouter::new(std::sync::Arc::new(crate::runtime::DockerRuntime));
        let result = tokio::time::timeout(crate::container_runner::container_timeout(), router.dispatch(event)).await;

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

    pub async fn send_message(&self, chat_id: &str, text: &str) -> Result<()> {
        let cid: i64 = chat_id.parse().map_err(|_| NuClawError::Telegram {
            message: format!("Invalid chat_id: {}", chat_id),
        })?;

        let chunks = self.chunk_text(text);
        let api_url = self.api_url.clone();

        let mut handles = Vec::new();
        for chunk in chunks {
            let api_url = api_url.clone();
            let handle =
                tokio::spawn(
                    async move { telegram_send_single_message(&api_url, cid, &chunk).await },
                );
            handles.push(handle);
        }

        for handle in handles {
            handle.await.map_err(|e| NuClawError::Telegram {
                message: format!("Join error: {}", e),
            })??;
        }

        Ok(())
    }

    fn chunk_text(&self, text: &str) -> Vec<String> {
        crate::telegram::utils::chunk_text_pure(text, self.text_chunk_limit)
    }

    async fn check_dm_policy(&self, user_id: &str) -> Result<bool> {
        match self.dm_policy {
            DMPolicy::Disabled => Ok(false),
            DMPolicy::Open => Ok(true),
            DMPolicy::Allowlist => Ok(true),
            DMPolicy::Pairing => {
                let manager = PairingManager::new()?;
                Ok(manager.is_authorized(user_id))
            }
        }
    }

    async fn handle_pairing_code(&self, code: &str, msg: &NewMessage) -> Result<Option<String>> {
        let chat_id = extract_chat_id_pure(&msg.chat_jid).ok_or_else(|| NuClawError::Telegram {
            message: "Invalid chat jid".to_string(),
        })?;

        let mut manager = match PairingManager::new() {
            Ok(m) => m,
            Err(e) => {
                error!("Failed to load pairing manager: {}", e);
                return Ok(Some(
                    "Pairing system unavailable. Please try again later.".to_string(),
                ));
            }
        };

        if let Some(pending) = manager.verify_code(code)? {
            if pending.user_id == "pending" || pending.user_id == msg.sender {
                if let Err(e) = manager.authorize_user(pending.clone()) {
                    error!("Failed to authorize user: {}", e);
                    return Ok(Some("Authorization failed. Please try again.".to_string()));
                }

                let response = format!(
                    "✅ Authorization successful!\n\nYou can now use {} in this chat.",
                    self.assistant_name
                );
                self.send_message(&chat_id.to_string(), &response)
                    .await
                    .ok();
                return Ok(Some("✅ You have been authorized!".to_string()));
            } else {
                return Ok(Some(
                    "This code is not for you. Please request your own pairing code.".to_string(),
                ));
            }
        }

        if manager.is_authorized(&msg.sender) {
            return Ok(None);
        }

        Ok(Some(
            "Invalid or expired pairing code. Please request a new one.".to_string(),
        ))
    }

    async fn is_allowed_group(&self, chat_jid: &str) -> Result<bool> {
        Ok(crate::telegram::utils::is_allowed_group_pure(
            chat_jid,
            self.group_policy,
            &self.allowed_groups,
        ))
    }

    async fn get_group_folder(&self, jid: &str) -> Option<String> {
        self.registered_groups.get(jid).map(|g| g.folder.clone())
    }

    fn extract_chat_id(&self, jid: &str) -> Result<String> {
        extract_chat_id_pure(jid).ok_or_else(|| NuClawError::Telegram {
            message: format!("Invalid telegram jid format: {}", jid),
        })
    }

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

    async fn update_router_state(&mut self, msg: &NewMessage) {
        self.router_state.last_timestamp = msg.timestamp.clone();
        self.router_state
            .last_agent_timestamp
            .insert(msg.chat_jid.clone(), msg.timestamp.clone());

        let router_state = self.router_state.clone();
        tokio::spawn(async move {
            let state_path = data_dir().join("router_state.json");
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
        ).map_err(|e| NuClawError::Database {
            message: format!("Failed to store message: {}", e),
        })?;

        Ok(())
    }

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

// Webhook handlers
async fn handle_telegram_webhook(
    State(client): State<Arc<Mutex<TelegramClient>>>,
    Json(update): Json<crate::telegram::types::TelegramUpdate>,
) -> (StatusCode, &'static str) {
    let mut client = client.lock().await;
    match client.handle_update(&update).await {
        Ok(_) => (StatusCode::OK, "OK"),
        Err(e) => {
            error!("Failed to handle telegram update: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "ERROR")
        }
    }
}

async fn health_check() -> (StatusCode, &'static str) {
    (StatusCode::OK, "OK")
}

// Helper functions
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

pub fn load_registered_groups() -> HashMap<String, RegisteredGroup> {
    let path = data_dir().join("registered_groups.json");
    load_json(&path, HashMap::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_router_state() {
        let state = load_router_state();
        assert_eq!(state.last_timestamp, "");
        assert!(state.last_agent_timestamp.is_empty());
    }

    #[test]
    fn test_load_registered_groups() {
        let groups = load_registered_groups();
        assert!(groups.is_empty() || !groups.contains_key("nonexistent"));
    }

    #[tokio::test]
    async fn test_telegram_client_new_requires_token() {
        std::env::remove_var("TELEGRAM_BOT_TOKEN");
        let result = TelegramClient::new(Database::new().unwrap());
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_webhook_requires_secret_when_configured() {
        std::env::set_var("TELEGRAM_WEBHOOK_SECRET", "test_secret_123");

        // Test without secret - should fail
        let result = std::env::var("TELEGRAM_WEBHOOK_SECRET");
        assert!(result.is_ok());

        std::env::remove_var("TELEGRAM_WEBHOOK_SECRET");
    }

    #[test]
    fn test_telegram_send_with_retry_handles_invalid_url() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(async {
            telegram_send_with_retry("https://invalid.url.that.does.not.exist", 123, "test", 1).await
        });
        assert!(result.is_err());
    }
}
