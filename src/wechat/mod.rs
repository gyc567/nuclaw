//! WeChat Integration for NuClaw
//!
//! Provides WeChat connectivity via ilink HTTP gateway (getUpdates long polling + sendMessage).
//!
//! ## Protocol Reference
//! Based on [cc-connect ilink protocol](https://github.com/chenhg5/cc-connect/blob/main/docs/weixin.md).
//!
//! ## Configuration
//! - `WEIXIN_API_URL`: ilink gateway URL (default: https://ilinkai.weixin.qq.com)
//! - `WEIXIN_TOKEN`: Bearer token from QR login
//! - `WEIXIN_ACCOUNT_ID`: Bot account ID for multi-account support
//! - `WEIXIN_CDN_BASE_URL`: CDN URL for media download
//! - `WEIXIN_ALLOW_FROM`: Comma-separated allowed sender IDs, or `*` for all

use crate::config::{assistant_name, data_dir};
use crate::db::Database;
use crate::error::{NuClawError, Result};
use crate::types::{NewMessage, RegisteredGroup, RouterState};
use crate::utils::json::{load_json, save_json};

use async_trait::async_trait;
use hex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;
use tracing::{debug, error, info, warn};

const ILINK_API_BASE: &str = "https://ilinkai.weixin.qq.com";
const DEFAULT_LONG_POLL_TIMEOUT_MS: u64 = 35000;
const DEFAULT_QR_TIMEOUT_SECS: u64 = 480;
const DEFAULT_WEIXIN_POLL_INTERVAL_MS: u64 = 2000;
const DEFAULT_TEXT_CHUNK_LIMIT: usize = 4000;

// =============================================================================
// Error Handling — ALWAYS use NuClawError, NEVER custom error types
// =============================================================================

pub fn ilink_errcode_to_error(errcode: i32, errmsg: Option<&str>) -> Option<NuClawError> {
    match errcode {
        0 => None,
        -14 => Some(NuClawError::Auth {
            message: "Session expired (errcode -14). Need re-login.".to_string(),
        }),
        -1 => Some(NuClawError::Auth {
            message: "QR code still scanning".to_string(),
        }),
        -2 => Some(NuClawError::Auth {
            message: "QR code not scanned yet".to_string(),
        }),
        _ => Some(NuClawError::WeChat {
            message: format!("ilink error {}: {}", errcode, errmsg.unwrap_or("unknown")),
        }),
    }
}

// =============================================================================
// ilink API Types
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetUpdatesResponse {
    pub list: Vec<IlinkMessage>,
    #[serde(default)]
    pub context_token: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IlinkMessage {
    pub msg_id: String,
    pub from_wxid: String,
    pub to_wxid: String,
    pub msg_type: i32,
    pub content: String,
    #[serde(default)]
    pub context_token: Option<String>,
    #[serde(default)]
    pub cdn_url: Option<String>,
    #[serde(default)]
    pub file_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendMessageRequest {
    pub to_wxid: String,
    pub content: String,
    #[serde(rename = "msgtype")]
    pub msg_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_token: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendMessageResponse {
    pub errcode: i32,
    pub errmsg: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetQrcodeResponse {
    pub qrcode: Option<String>,
    pub qrcode_url: Option<String>,
    pub qrcode_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginByQrcodeResponse {
    pub errcode: i32,
    pub errmsg: Option<String>,
    pub token: Option<String>,
    pub account_id: Option<String>,
    pub base_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaDownloadResponse {
    pub errcode: i32,
    pub errmsg: Option<String>,
}

// =============================================================================
// WeChat Client
// =============================================================================

pub struct WeChatClient {
    api_url: String,
    token: Option<String>,
    account_id: Option<String>,
    cdn_base_url: Option<String>,
    allow_from: Vec<String>,
    registered_chats: HashMap<String, RegisteredGroup>,
    router_state: RouterState,
    db: Database,
    assistant_name: String,
    context_token: Option<String>,
    http_client: reqwest::Client,
}

// =============================================================================
// Pure Functions (Testable)
// =============================================================================

pub fn load_router_state() -> RouterState {
    let state_path = data_dir().join("wechat_router_state.json");
    load_json(&state_path, RouterState::default())
}

pub fn load_registered_chats() -> HashMap<String, RegisteredGroup> {
    let path = data_dir().join("wechat_registered_chats.json");
    load_json(&path, HashMap::new())
}

pub fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let prefix_len = max_len.saturating_sub(3);
        format!("{}...", &s[..prefix_len])
    }
}

pub fn is_allowed_sender_pure(sender: &str, allow_from: &[String]) -> bool {
    allow_from.is_empty() || allow_from.iter().any(|w| w == "*" || sender.contains(w))
}

pub fn extract_trigger_pure(content: &str, assistant_name: &str) -> Option<(String, String)> {
    let patterns = [format!("@{}", assistant_name), assistant_name.to_string()];
    for pattern in &patterns {
        if let Some(idx) = content.find(pattern) {
            let after = &content[idx + pattern.len()..];
            let c = after.trim().to_string();
            if !c.is_empty() {
                return Some((pattern.clone(), c));
            }
        }
    }
    None
}

pub fn is_duplicate_message_pure(
    msg: &NewMessage,
    last_message_ids: &HashMap<String, String>,
) -> bool {
    last_message_ids
        .get(&msg.chat_jid)
        .map(|id| id == &msg.id)
        .unwrap_or(false)
}

pub fn parse_message_type_pure(msg_type: i32) -> &'static str {
    match msg_type {
        1 => "text",
        3 => "image",
        34 => "voice",
        43 => "video",
        47 => "emoji",
        49 => "file",
        10000 => "system",
        _ => "unknown",
    }
}

// =============================================================================
// WeChatClient Implementation
// =============================================================================

impl WeChatClient {
    pub fn new(db: Database) -> Result<Self> {
        let http_client = reqwest::Client::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .map_err(|e| NuClawError::Api {
                message: format!("Failed to create HTTP client: {}", e),
            })?;

        Ok(Self {
            api_url: std::env::var("WEIXIN_API_URL")
                .unwrap_or_else(|_| ILINK_API_BASE.to_string()),
            token: std::env::var("WEIXIN_TOKEN").ok(),
            account_id: std::env::var("WEIXIN_ACCOUNT_ID").ok(),
            cdn_base_url: std::env::var("WEIXIN_CDN_BASE_URL").ok(),
            allow_from: std::env::var("WEIXIN_ALLOW_FROM")
                .ok()
                .map(|s| s.split(',').map(str::to_string).collect())
                .unwrap_or_default(),
            registered_chats: load_registered_chats(),
            router_state: load_router_state(),
            db,
            assistant_name: assistant_name(),
            context_token: None,
            http_client,
        })
    }

    async fn http_get_with_auth(&self, url: &str) -> Result<String> {
        let token = self.token.as_ref().ok_or_else(|| NuClawError::Auth {
            message: "No WEIXIN_TOKEN configured".to_string(),
        })?;
        let response = self
            .http_client
            .get(url)
            .header("Authorization", format!("Bearer {}", token))
            .send()
            .await
            .map_err(|e| NuClawError::Api {
                message: format!("ilink GET failed: {}", e),
            })?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(NuClawError::Api {
                message: format!("ilink API error: {} - {}", status, body),
            });
        }
        response.text().await.map_err(|e| NuClawError::Api {
            message: format!("Failed to read response: {}", e),
        })
    }

    async fn http_post_json<T: Serialize>(&self, url: &str, body: &T) -> Result<String> {
        let token = self.token.as_ref().ok_or_else(|| NuClawError::Auth {
            message: "No WEIXIN_TOKEN configured".to_string(),
        })?;
        let response = self
            .http_client
            .post(url)
            .header("Authorization", format!("Bearer {}", token))
            .json(body)
            .send()
            .await
            .map_err(|e| NuClawError::Api {
                message: format!("ilink POST failed: {}", e),
            })?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(NuClawError::Api {
                message: format!("ilink API error: {} - {}", status, body),
            });
        }
        response.text().await.map_err(|e| NuClawError::Api {
            message: format!("Failed to read response: {}", e),
        })
    }

    // -------------------------------------------------------------------------
    // QR Code Login
    // -------------------------------------------------------------------------

    pub async fn request_qr_code(&mut self) -> Result<Option<(String, String)>> {
        let url = format!("{}/cgi-bin/getBotQrcode?bot_type=3", self.api_url);
        let token = self.token.as_ref().ok_or_else(|| NuClawError::Auth {
            message: "No WEIXIN_TOKEN configured".to_string(),
        })?;
        let response = self
            .http_client
            .get(&url)
            .header("Authorization", format!("Bearer {}", token))
            .send()
            .await
            .map_err(|e| NuClawError::Api {
                message: format!("Failed to request QR code: {}", e),
            })?;
        let qr: GetQrcodeResponse = response.json().await.map_err(|e| NuClawError::Api {
            message: format!("Failed to parse QR code response: {}", e),
        })?;
        if let (Some(qrcode_id), Some(qrcode)) = (&qr.qrcode_id, &qr.qrcode) {
            return Ok(Some((qrcode_id.clone(), qrcode.clone())));
        }
        Ok(qr.qrcode_url.map(|u| ("unknown".to_string(), u)))
    }

    pub async fn poll_login(&mut self, qrcode_id: &str, timeout_secs: u64) -> Result<()> {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(timeout_secs);
        while tokio::time::Instant::now() < deadline {
            let url = format!("{}/cgi-bin/loginByQrcode", self.api_url);
            let body = serde_json::json!({ "qrcode_id": qrcode_id });
            let response = self.http_post_json(&url, &body).await?;
            let login: LoginByQrcodeResponse =
                serde_json::from_str(&response).map_err(|e| NuClawError::Api {
                    message: format!("Failed to parse login response: {}", e),
                })?;
            if let Some(err) = ilink_errcode_to_error(login.errcode, login.errmsg.as_deref()) {
                if matches!(err, NuClawError::Auth { .. }) && login.errcode < 0 && login.errcode != -14 {
                    debug!("Waiting for QR code scan...");
                    tokio::time::sleep(Duration::from_secs(2)).await;
                    continue;
                }
                return Err(err);
            }
            if login.errcode == 0 {
                self.token = login.token;
                self.account_id = login.account_id;
                if let Some(base_url) = login.base_url {
                    self.api_url = base_url;
                }
                info!("WeChat login successful");
                return Ok(());
            }
        }
        Err(NuClawError::Auth {
            message: "QR code scan timeout".to_string(),
        })
    }

    // -------------------------------------------------------------------------
    // getUpdates Long Polling
    // -------------------------------------------------------------------------

    async fn poll_messages(&self) -> Result<Vec<IlinkMessage>> {
        let url = format!(
            "{}/cgi-bin/getUpdates?timeout={}",
            self.api_url, DEFAULT_LONG_POLL_TIMEOUT_MS
        );
        let response = self.http_get_with_auth(&url).await?;
        let updates: GetUpdatesResponse =
            serde_json::from_str(&response).map_err(|e| NuClawError::Api {
                message: format!("Failed to parse getUpdates: {}", e),
            })?;
        Ok(updates.list)
    }

    async fn process_message(&mut self, msg: &IlinkMessage) -> Result<Option<String>> {
        if !is_allowed_sender_pure(&msg.from_wxid, &self.allow_from) {
            debug!(
                "Ignoring message from unauthorized sender: {}",
                msg.from_wxid
            );
            return Ok(None);
        }
        let msg_type_str = parse_message_type_pure(msg.msg_type);
        debug!(
            "Received WeChat message: type={}, from={}, content={}",
            msg_type_str,
            msg.from_wxid,
            truncate(&msg.content, 50)
        );
        if msg.msg_type != 1 {
            return Ok(None);
        }
        let (trigger, content) = match extract_trigger_pure(&msg.content, &self.assistant_name) {
            Some((t, c)) => (t, c),
            None => return Ok(None),
        };
        if let Some(ctx) = &msg.context_token {
            self.context_token = Some(ctx.clone());
        }
        let new_msg = NewMessage {
            id: msg.msg_id.clone(),
            chat_jid: format!("wechat:{}", msg.from_wxid),
            sender: msg.from_wxid.clone(),
            sender_name: msg.from_wxid.clone(),
            content: content.clone(),
            timestamp: chrono::Utc::now().to_rfc3339(),
        };
        if is_duplicate_message_pure(&new_msg, &self.router_state.last_message_ids) {
            debug!("Skipping duplicate message: {}", new_msg.id);
            return Ok(None);
        }
        self.router_state
            .last_message_ids
            .insert(new_msg.chat_jid.clone(), new_msg.id.clone());
        let _ = save_json(
            &data_dir().join("wechat_router_state.json"),
            &self.router_state,
        );
        info!(
            "Processing WeChat message from {}: {}",
            new_msg.sender,
            truncate(&content, 50)
        );
        Ok(Some(content))
    }

    pub async fn start_polling(&self) -> Result<()> {
        if self.token.is_none() {
            return Err(NuClawError::Auth {
                message: "No WEIXIN_TOKEN configured. Run QR login first.".to_string(),
            });
        }
        info!("Starting WeChat message polling loop...");
        let mut client = (*self).clone();
        loop {
            match client.poll_messages().await {
                Ok(messages) => {
                    for msg in messages {
                        if let Err(e) = client.process_message(&msg).await {
                            error!("Failed to process WeChat message: {}", e);
                        }
                    }
                }
                Err(e) => {
                    error!("Polling error: {}", e);
                    tokio::time::sleep(Duration::from_millis(DEFAULT_WEIXIN_POLL_INTERVAL_MS))
                        .await;
                }
            }
        }
    }

    // -------------------------------------------------------------------------
    // Send Messages
    // -------------------------------------------------------------------------

    pub async fn send_message(&mut self, to_wxid: &str, text: &str) -> Result<()> {
        if text.trim().is_empty() {
            warn!("Skipping empty WeChat message");
            return Ok(());
        }
        let chunks: Vec<String> = text
            .chars()
            .collect::<Vec<char>>()
            .chunks(DEFAULT_TEXT_CHUNK_LIMIT)
            .map(|c| c.iter().collect::<String>())
            .collect();
        for chunk in &chunks {
            let url = format!("{}/cgi-bin/sendMessage", self.api_url);
            let request = SendMessageRequest {
                to_wxid: to_wxid.to_string(),
                content: chunk.clone(),
                msg_type: "text".to_string(),
                context_token: self.context_token.clone(),
            };
            let response = self.http_post_json(&url, &request).await?;
            let resp: SendMessageResponse =
                serde_json::from_str(&response).map_err(|e| NuClawError::Api {
                    message: format!("Failed to parse send response: {}", e),
                })?;
            if let Some(err) = ilink_errcode_to_error(resp.errcode, resp.errmsg.as_deref()) {
                return Err(err);
            }
        }
        Ok(())
    }

    pub async fn send_image(&mut self, to_wxid: &str, media_id: &str) -> Result<()> {
        let url = format!("{}/cgi-bin/sendMessage", self.api_url);
        let request = serde_json::json!({
            "to_wxid": to_wxid,
            "content": media_id,
            "msgtype": "image",
            "context_token": self.context_token
        });
        let response = self.http_post_json(&url, &request).await?;
        let resp: SendMessageResponse =
            serde_json::from_str(&response).map_err(|e| NuClawError::Api {
                message: format!("Failed to parse send response: {}", e),
            })?;
        if let Some(err) = ilink_errcode_to_error(resp.errcode, resp.errmsg.as_deref()) {
            return Err(err);
        }
        Ok(())
    }

    // -------------------------------------------------------------------------
    // Media Download & AES Decryption
    // -------------------------------------------------------------------------

    pub async fn download_media(&self, cdn_url: &str, file_key: &str) -> Result<Vec<u8>> {
        let response = self
            .http_client
            .get(cdn_url)
            .send()
            .await
            .map_err(|e| NuClawError::Api {
                message: format!("Failed to download media: {}", e),
            })?;
        let encrypted = response.bytes().await.map_err(|e| NuClawError::Api {
            message: format!("Failed to read media bytes: {}", e),
        })?;
        let parts: Vec<&str> = file_key.splitn(2, '|').collect();
        if parts.len() != 2 {
            return Err(NuClawError::Api {
                message: "Invalid file_key format: expected 'file_id|key'".to_string(),
            });
        }
        let aes_key = hex::decode(parts[1]).map_err(|e| NuClawError::Api {
            message: format!("Invalid hex key: {}", e),
        })?;
        let decrypted = aes_ecb_decrypt(&encrypted, &aes_key)?;
        Ok(decrypted)
    }

    pub fn is_authenticated(&self) -> bool {
        self.token.is_some()
    }
}

fn aes_ecb_decrypt(data: &[u8], key: &[u8]) -> Result<Vec<u8>> {
    use aes::Aes128Dec;
    use cipher::{Block, BlockDecryptMut, KeyInit};

    let mut cipher = Aes128Dec::new_from_slice(key).map_err(|e| NuClawError::Api {
        message: format!("Invalid AES key: {}", e),
    })?;
    let block_size = 16;
    let mut result = Vec::with_capacity(data.len());
    for chunk in data.chunks(block_size) {
        if chunk.len() == block_size {
            let mut block_bytes = [0u8; 16];
            block_bytes.copy_from_slice(chunk);
            let mut block: Block<aes::Aes128> = block_bytes.into();
            cipher.decrypt_block_mut(&mut block);
            result.extend_from_slice(&block);
        } else if !chunk.is_empty() {
            return Err(NuClawError::Api {
                message: format!("Data length {} not multiple of block size", data.len()),
            });
        }
    }
    Ok(result)
}

impl Clone for WeChatClient {
    fn clone(&self) -> Self {
        Self {
            api_url: self.api_url.clone(),
            token: self.token.clone(),
            account_id: self.account_id.clone(),
            cdn_base_url: self.cdn_base_url.clone(),
            allow_from: self.allow_from.clone(),
            registered_chats: self.registered_chats.clone(),
            router_state: self.router_state.clone(),
            db: self.db.clone(),
            assistant_name: self.assistant_name.clone(),
            context_token: self.context_token.clone(),
            http_client: self.http_client.clone(),
        }
    }
}

#[async_trait]
impl crate::channels::Channel for WeChatClient {
    fn name(&self) -> &str {
        "wechat"
    }

    fn is_enabled(&self) -> bool {
        self.token.is_some()
    }

    async fn send(&self, jid: &str, message: &str) -> Result<()> {
        let to_wxid = jid.strip_prefix("wechat:").unwrap_or(jid);
        let mut client = (*self).clone();
        client.send_message(to_wxid, message).await
    }

    async fn start(&self) -> Result<()> {
        self.start_polling().await
    }
}

pub fn should_auto_start_wechat() -> bool {
    std::env::var("WEIXIN_TOKEN").is_ok()
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ilink_errcode_to_error_success() {
        assert!(ilink_errcode_to_error(0, None).is_none());
    }

    #[test]
    fn test_ilink_errcode_session_expired() {
        let err = ilink_errcode_to_error(-14, None).unwrap();
        assert!(matches!(err, NuClawError::Auth { .. }));
    }

    #[test]
    fn test_ilink_errcode_pending() {
        let err = ilink_errcode_to_error(-1, None).unwrap();
        assert!(matches!(err, NuClawError::Auth { .. }));
        let err = ilink_errcode_to_error(-2, None).unwrap();
        assert!(matches!(err, NuClawError::Auth { .. }));
    }

    #[test]
    fn test_ilink_errcode_other() {
        let err = ilink_errcode_to_error(-100, Some("test")).unwrap();
        assert!(matches!(err, NuClawError::WeChat { .. }));
    }

    #[test]
    fn test_is_allowed_sender_open() {
        assert!(is_allowed_sender_pure("user@im.wechat", &[]));
        assert!(is_allowed_sender_pure(
            "user@im.wechat",
            &["*".to_string()]
        ));
    }

    #[test]
    fn test_is_allowed_sender_allowlist() {
        let allow = vec![
            "alice@im.wechat".to_string(),
            "bob@im.wechat".to_string(),
        ];
        assert!(is_allowed_sender_pure("alice@im.wechat", &allow));
        assert!(!is_allowed_sender_pure("charlie@im.wechat", &allow));
    }

    #[test]
    fn test_extract_trigger_with_at() {
        let (trigger, content) = extract_trigger_pure("@Andy hello world", "Andy").unwrap();
        assert_eq!(trigger, "@Andy");
        assert_eq!(content, "hello world");
    }

    #[test]
    fn test_extract_trigger_without_at() {
        let (trigger, content) = extract_trigger_pure("Andy hello world", "Andy").unwrap();
        assert_eq!(trigger, "Andy");
        assert_eq!(content, "hello world");
    }

    #[test]
    fn test_extract_trigger_no_match() {
        assert!(extract_trigger_pure("hello world", "Andy").is_none());
        assert!(extract_trigger_pure("@Other hello", "Andy").is_none());
    }

    #[test]
    fn test_duplicate_detection() {
        let msg = NewMessage {
            id: "msg123".to_string(),
            chat_jid: "chat1".to_string(),
            sender: "user1".to_string(),
            sender_name: "User 1".to_string(),
            content: "test".to_string(),
            timestamp: "2025-01-01T00:00:00Z".to_string(),
        };
        assert!(!is_duplicate_message_pure(&msg, &HashMap::new()));
        let ids = HashMap::from([("chat1".to_string(), "msg123".to_string())]);
        assert!(is_duplicate_message_pure(&msg, &ids));
    }

    #[test]
    fn test_parse_message_type() {
        assert_eq!(parse_message_type_pure(1), "text");
        assert_eq!(parse_message_type_pure(3), "image");
        assert_eq!(parse_message_type_pure(34), "voice");
        assert_eq!(parse_message_type_pure(43), "video");
        assert_eq!(parse_message_type_pure(49), "file");
        assert_eq!(parse_message_type_pure(10000), "system");
        assert_eq!(parse_message_type_pure(999), "unknown");
    }

    #[test]
    fn test_truncate() {
        assert_eq!(truncate("hello", 10), "hello");
        assert_eq!(truncate("hello world", 8), "hello...");
        assert_eq!(truncate("", 5), "");
        assert_eq!(truncate("hello", 5), "hello");
        assert_eq!(truncate("hello", 4), "h...");
    }

    #[test]
    fn test_ilink_message_deserialization() {
        let json = r#"{
            "msg_id": "msg123",
            "from_wxid": "user@im.wechat",
            "to_wxid": "bot@im.wechat",
            "msg_type": 1,
            "content": "hello"
        }"#;
        let msg: IlinkMessage = serde_json::from_str(json).unwrap();
        assert_eq!(msg.msg_id, "msg123");
        assert_eq!(msg.from_wxid, "user@im.wechat");
        assert_eq!(msg.msg_type, 1);
        assert_eq!(msg.content, "hello");
        assert!(msg.context_token.is_none());
    }

    #[test]
    fn test_ilink_message_with_context_token() {
        let json = r#"{
            "msg_id": "msg456",
            "from_wxid": "user@im.wechat",
            "to_wxid": "bot@im.wechat",
            "msg_type": 1,
            "content": "hello",
            "context_token": "ctx_abc123",
            "cdn_url": "https://example.com/file",
            "file_key": "file_id|a1b2c3d4e5f6"
        }"#;
        let msg: IlinkMessage = serde_json::from_str(json).unwrap();
        assert_eq!(msg.context_token, Some("ctx_abc123".to_string()));
        assert_eq!(msg.cdn_url, Some("https://example.com/file".to_string()));
        assert_eq!(msg.file_key, Some("file_id|a1b2c3d4e5f6".to_string()));
    }

    #[test]
    fn test_send_message_request_serialization() {
        let request = SendMessageRequest {
            to_wxid: "user@im.wechat".to_string(),
            content: "hello world".to_string(),
            msg_type: "text".to_string(),
            context_token: Some("ctx123".to_string()),
        };
        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"to_wxid\":\"user@im.wechat\""));
        assert!(json.contains("\"msgtype\":\"text\""));
        assert!(json.contains("\"context_token\":\"ctx123\""));
    }

    #[test]
    fn test_send_message_response_deserialization() {
        let json = r#"{"errcode": 0, "errmsg": "ok"}"#;
        let response: SendMessageResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.errcode, 0);
        assert_eq!(response.errmsg, Some("ok".to_string()));
    }

    #[test]
    fn test_login_response_success() {
        let json = r#"{
            "errcode": 0,
            "token": "bearer_token_123",
            "account_id": "acc_456",
            "base_url": "https://custom.ilink.com"
        }"#;
        let response: LoginByQrcodeResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.errcode, 0);
        assert_eq!(response.token, Some("bearer_token_123".to_string()));
        assert_eq!(response.account_id, Some("acc_456".to_string()));
    }

    #[test]
    fn test_login_response_pending() {
        let json = r#"{"errcode": -1, "errmsg": "pending"}"#;
        let response: LoginByQrcodeResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.errcode, -1);
        assert!(response.token.is_none());
    }

    #[test]
    fn test_should_auto_start_wechat() {
        let original = std::env::var("WEIXIN_TOKEN").ok();
        std::env::remove_var("WEIXIN_TOKEN");
        assert!(!should_auto_start_wechat());
        std::env::set_var("WEIXIN_TOKEN", "test_token");
        assert!(should_auto_start_wechat());
        match original {
            Some(val) => std::env::set_var("WEIXIN_TOKEN", val),
            None => std::env::remove_var("WEIXIN_TOKEN"),
        }
    }

    #[test]
    fn test_aes_ecb_decrypt_basic() {
        use aes::{Aes128Dec, Aes128Enc};
        use cipher::{Block, BlockDecryptMut, BlockEncryptMut, KeyInit};

        let key = [0u8; 16];
        let plaintext_arr = *b"Hello, World!!!!";

        // Encrypt
        let mut enc = Aes128Enc::new_from_slice(&key).unwrap();
        let mut block: Block<aes::Aes128> = plaintext_arr.into();
        enc.encrypt_block_mut(&mut block);
        let encrypted = block.to_vec();

        // Decrypt with aes_ecb_decrypt
        let decrypted = aes_ecb_decrypt(&encrypted, &key).unwrap();
        assert_eq!(&decrypted[..plaintext_arr.len()], &plaintext_arr);
    }

    #[test]
    fn test_end_to_end_message_processing_flow() {
        let msg = IlinkMessage {
            msg_id: "msg_001".to_string(),
            from_wxid: "alice@im.wechat".to_string(),
            to_wxid: "bot@im.wechat".to_string(),
            msg_type: 1,
            content: "@Andy hello world".to_string(),
            context_token: Some("ctx_abc".to_string()),
            cdn_url: None,
            file_key: None,
        };

        assert_eq!(parse_message_type_pure(msg.msg_type), "text");
        assert!(is_allowed_sender_pure(
            "alice@im.wechat",
            &["alice@im.wechat".to_string()]
        ));
        let trigger = extract_trigger_pure("@Andy hello world", "Andy");
        assert!(trigger.is_some());
        let (t, c) = trigger.unwrap();
        assert_eq!(t, "@Andy");
        assert_eq!(c, "hello world");

        let req = SendMessageRequest {
            to_wxid: "alice@im.wechat".to_string(),
            content: "Hello alice!".to_string(),
            msg_type: "text".to_string(),
            context_token: Some("ctx_abc".to_string()),
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"to_wxid\":\"alice@im.wechat\""));
        assert!(json.contains("\"msgtype\":\"text\""));
        assert!(json.contains("\"context_token\":\"ctx_abc\""));
    }

    #[test]
    fn test_deduplication_prevents_replay() {
        let mut last_ids = HashMap::new();

        let msg1 = NewMessage {
            id: "msg1".to_string(),
            chat_jid: "wechat:alice".to_string(),
            sender: "alice".to_string(),
            sender_name: "Alice".to_string(),
            content: "hello".to_string(),
            timestamp: "2025-01-01T00:00:00Z".to_string(),
        };

        assert!(!is_duplicate_message_pure(&msg1, &last_ids));
        last_ids.insert(msg1.chat_jid.clone(), msg1.id.clone());

        let msg2 = NewMessage {
            id: "msg1".to_string(),
            chat_jid: "wechat:alice".to_string(),
            sender: "alice".to_string(),
            sender_name: "Alice".to_string(),
            content: "hello again".to_string(),
            timestamp: "2025-01-01T00:00:01Z".to_string(),
        };

        assert!(is_duplicate_message_pure(&msg2, &last_ids));

        let msg3 = NewMessage {
            id: "msg2".to_string(),
            chat_jid: "wechat:alice".to_string(),
            sender: "alice".to_string(),
            sender_name: "Alice".to_string(),
            content: "different".to_string(),
            timestamp: "2025-01-01T00:00:02Z".to_string(),
        };

        assert!(!is_duplicate_message_pure(&msg3, &last_ids));
    }

    #[test]
    fn test_allow_from_security_model() {
        let allow = vec![
            "alice@im.wechat".to_string(),
            "bob@im.wechat".to_string(),
        ];

        assert!(is_allowed_sender_pure("alice@im.wechat", &allow));
        assert!(is_allowed_sender_pure("bob@im.wechat", &allow));
        assert!(!is_allowed_sender_pure("charlie@im.wechat", &allow));

        let open = vec!["*".to_string()];
        assert!(is_allowed_sender_pure("anyone@im.wechat", &open));

        let empty: Vec<String> = vec![];
        assert!(is_allowed_sender_pure("anyone@im.wechat", &empty));
    }

    #[test]
    fn test_media_message_handling() {
        let msg = IlinkMessage {
            msg_id: "media_001".to_string(),
            from_wxid: "alice@im.wechat".to_string(),
            to_wxid: "bot@im.wechat".to_string(),
            msg_type: 3,
            content: "image_content".to_string(),
            context_token: None,
            cdn_url: Some("https://cdn.example.com/img/123".to_string()),
            file_key: Some("file_123|a1b2c3d4e5f6".to_string()),
        };

        assert_eq!(parse_message_type_pure(msg.msg_type), "image");
        assert!(msg.cdn_url.is_some());
        assert!(msg.file_key.is_some());

        let parts: Vec<&str> = msg.file_key.as_ref().unwrap().splitn(2, '|').collect();
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0], "file_123");
        assert_eq!(parts[1], "a1b2c3d4e5f6");

        let hex_key = hex::decode(parts[1]);
        assert!(hex_key.is_ok());
        assert_eq!(hex_key.unwrap().len(), 6);
    }

    #[test]
    fn test_qr_login_response_parsing() {
        let success = r#"{
            "errcode": 0,
            "token": "ilink_token_abc123",
            "account_id": "bot_account_456",
            "base_url": "https://custom.ilink.com"
        }"#;
        let resp: LoginByQrcodeResponse = serde_json::from_str(success).unwrap();
        assert_eq!(resp.errcode, 0);
        assert_eq!(resp.token.as_deref(), Some("ilink_token_abc123"));
        assert_eq!(resp.account_id.as_deref(), Some("bot_account_456"));
        assert!(ilink_errcode_to_error(resp.errcode, resp.errmsg.as_deref()).is_none());

        let expired = r#"{"errcode": -14, "errmsg": "session expired"}"#;
        let resp: LoginByQrcodeResponse = serde_json::from_str(expired).unwrap();
        assert!(matches!(
            ilink_errcode_to_error(resp.errcode, resp.errmsg.as_deref()),
            Some(NuClawError::Auth { .. })
        ));

        let pending = r#"{"errcode": -1}"#;
        let resp: LoginByQrcodeResponse = serde_json::from_str(pending).unwrap();
        assert!(matches!(
            ilink_errcode_to_error(resp.errcode, None),
            Some(NuClawError::Auth { .. })
        ));
    }

    #[test]
    fn test_channel_enabled_when_token_present() {
        std::env::set_var("WEIXIN_TOKEN", "test_token");
        assert!(should_auto_start_wechat());
        std::env::remove_var("WEIXIN_TOKEN");
        assert!(!should_auto_start_wechat());
    }

    #[test]
    fn test_get_updates_response_multiple_messages() {
        let json = r#"{
            "list": [
                {
                    "msg_id": "msg1",
                    "from_wxid": "alice@im.wechat",
                    "to_wxid": "bot@im.wechat",
                    "msg_type": 1,
                    "content": "hello"
                },
                {
                    "msg_id": "msg2",
                    "from_wxid": "bob@im.wechat",
                    "to_wxid": "bot@im.wechat",
                    "msg_type": 1,
                    "content": "@Andy help"
                }
            ],
            "context_token": "shared_ctx"
        }"#;

        let resp: GetUpdatesResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.list.len(), 2);
        assert_eq!(resp.context_token.as_deref(), Some("shared_ctx"));
        assert_eq!(resp.list[0].msg_id, "msg1");
        assert_eq!(resp.list[1].msg_id, "msg2");
    }

    #[test]
    fn test_send_message_response_various_errcodes() {
        let tests = vec![
            (0, true, "success"),
            (-14, false, "session expired"),
            (-1, false, "pending"),
            (-2, false, "not scanned"),
            (-100, false, "other error"),
        ];

        for (errcode, expect_ok, label) in tests {
            let json = serde_json::json!({
                "errcode": errcode,
                "errmsg": label
            });
            let resp: SendMessageResponse = serde_json::from_str(&json.to_string()).unwrap();
            let result = ilink_errcode_to_error(resp.errcode, resp.errmsg.as_deref());
            if expect_ok {
                assert!(result.is_none(), "errcode {} should be ok", errcode);
            } else {
                assert!(result.is_some(), "errcode {} should be error", errcode);
            }
        }
    }

    #[test]
    fn test_aes_ecb_decrypt_real_world_pattern() {
        let key = [0u8; 16];
        let encrypted = vec![0u8; 16];
        let result = aes_ecb_decrypt(&encrypted, &key);
        assert!(result.is_ok());
    }

}
