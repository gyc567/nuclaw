---
name: wechat-channel-generator
description: "Generate NuClaw WeChat channel module using ilink HTTP gateway. Use when: create a new wechat.rs module, add WeChat support to NuClaw, implement ilink protocol for WeChat. Triggers: generate wechat channel, implement WeChat bot, add wechat support, ilink protocol. Output: complete, compilable Rust wechat.rs following NuClaw conventions."
---

# WeChat Channel Generator

Generate production-quality WeChat channel module for NuClaw using the **ilink HTTP gateway** protocol (same as cc-connect's `openclaw-weixin`).

---

## Reference: ilink HTTP Gateway Protocol

Based on [cc-connect weixin.md](https://github.com/chenhg5/cc-connect/blob/main/docs/weixin.md).

### Core API Endpoints

```
Base URL: https://ilinkai.weixin.qq.com

GET /cgi-bin/getUpdates?timeout=<ms>
  Headers: Authorization: Bearer <token>
  Response: list of message objects

POST /cgi-bin/sendMessage
  Headers: Authorization: Bearer <token>
  Body: {"to_wxid": "...", "content": "...", "msgtype": "text"}
  Response: {"errcode": 0, ...}

GET /cgi-bin/getBotQrcode?bot_type=3
  Headers: Authorization: Bearer <token>
  Response: {"qrcode": "...", "qrcode_url": "..."}

POST /cgi-bin/loginByQrcode
  Body: {"qrcode_id": "..."}
  Response: {"errcode": 0, "token": "...", "account_id": "..."}
```

### Key Concepts

1. **Bearer Token Auth**: All API calls require `Authorization: Bearer <token>` header
2. **Long Polling**: `getUpdates` with `timeout` param (default 35s) — client waits for messages
3. **context_token**: First message includes `context_token` that must be used in replies
4. **allow_from**: Restrict access by user ID (format: `xxx@im.wechat`), comma-separated or `*`
5. **account_id**: Multi-account support via state directory isolation
6. **CDN Media**: Images/files come from CDN with AES-128-ECB encryption

### ilink Message Types

```json
// Incoming text message
{
  "msg_id": "...",
  "from_wxid": "user@im.wechat",
  "to_wxid": "bot@im.wechat",
  "msg_type": 1,
  "content": "hello",
  "context_token": "optional_token_for_first_msg"
}

// Incoming image message
{
  "msg_id": "...",
  "from_wxid": "user@im.wechat",
  "to_wxid": "bot@im.wechat",
  "msg_type": 3,
  "content": "file_id|encrypted_key",
  "cdn_url": "https://..."
}

// Outgoing message
{
  "to_wxid": "user@im.wechat",
  "content": "response text",
  "msgtype": "text",
  "context_token": "use_incoming_context_token_if_present"
}
```

### QR Code Login Flow

```
1. GET /cgi-bin/getBotQrcode?bot_type=3
   → Returns {qrcode: "base64_png_or_ascii", qrcode_url: "https://..."}
   
2. Display QR to user (ASCII art or URL link)

3. POST /cgi-bin/loginByQrcode {"qrcode_id": "..."}
   → Poll repeatedly until {"errcode": 0, "token": "...", "account_id": "..."}
   → Timeout: 480s default

4. Save token + account_id to config
```

---

## NuClaw Architecture Overview

NuClaw is a Rust-based personal AI assistant with these core components:

- **Providers**: LLM API integrations (Anthropic, OpenAI, OpenRouter)
- **Channels**: Messaging integrations (WhatsApp, Telegram, Feishu, **WeChat**)
- **Database**: SQLite persistence with r2d2 connection pooling
- **Container Runner**: Docker/Apple Container management
- **Task Scheduler**: Cron-based scheduled task execution
- **Skills System**: Extensible skill registry

---

## Output Structure

The generated code should be a **single file** `src/wechat.rs` (or module with submodules) that:

1. **Implements the `Channel` trait** from `src/channels.rs`
2. **Uses existing NuClaw types** — don't redefine `NewMessage`, `RegisteredGroup`, `RouterState`
3. **Follows Feishu/Telegram patterns** — similar structure and organization
4. **Handles ilink protocol** — getUpdates, sendMessage, Bearer auth

---

## Import Organization (MUST FOLLOW)

```rust
//! WeChat Integration for NuClaw
//!
//! Provides WeChat connectivity via ilink HTTP gateway (getUpdates + sendMessage).

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::time::Duration;
use tracing::{debug, error, info, warn};

use crate::config::{assistant_name, data_dir};
use crate::db::Database;
use crate::error::{NuClawError, Result};
use crate::types::{NewMessage, RegisteredGroup, RouterState};
use crate::utils::json::{load_json, save_json};
```

**Rule**: 
- `std` imports first
- Then external crates (alphabetically within version groups)
- Then `crate` imports last
- Separate groups with blank lines

---

## Core Module Structure

### 1. Constants

```rust
/// Default ilink API base URL
const ILINK_API_BASE: &str = "https://ilinkai.weixin.qq.com";

/// Default long poll timeout: 35 seconds
const DEFAULT_LONG_POLL_TIMEOUT_MS: u64 = 35000;

/// Default QR code timeout: 480 seconds
const DEFAULT_QR_TIMEOUT_SECS: u64 = 480;

/// Default WeChat poll interval: 2 seconds
const DEFAULT_WEIXIN_POLL_INTERVAL_MS: u64 = 2000;
```

### 2. Error Types

```rust
#[derive(Error, Debug)]
pub enum WeChatError {
    #[error("ilink API error: {message}")]
    Api { message: String },
    
    #[error("Authentication error: {message}")]
    Auth { message: String },
    
    #[error("QR code timeout")]
    QrTimeout,
    
    #[error("Session expired (errcode -14)")]
    SessionExpired,
    
    #[error("Invalid token")]
    InvalidToken,
}
```

### 3. Client State

```rust
pub struct WeChatClient {
    /// ilink API base URL
    api_url: String,
    /// Bearer token for authentication
    token: Option<String>,
    /// Bot account ID (ilink_bot_id)
    account_id: Option<String>,
    /// CDN base URL for media
    cdn_base_url: Option<String>,
    /// Allowed sender IDs (format: user@im.wechat)
    allow_from: Vec<String>,
    /// Registered groups (wechat chats)
    registered_groups: HashMap<String, RegisteredGroup>,
    /// Router state for message deduplication
    router_state: RouterState,
    /// Database connection
    db: Database,
    /// Assistant name for trigger detection
    assistant_name: String,
}
```

### 4. ilink API Request/Response Types

```rust
// GET /cgi-bin/getUpdates response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetUpdatesResponse {
    pub list: Vec<IlinkMessage>,
    pub context_token: Option<String>,
}

// Incoming message from getUpdates
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IlinkMessage {
    pub msg_id: String,
    pub from_wxid: String,
    pub to_wxid: String,
    pub msg_type: i32,
    pub content: String,
    pub context_token: Option<String>,
    pub cdn_url: Option<String>,
    pub file_key: Option<String>,
}

// POST /cgi-bin/sendMessage request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendMessageRequest {
    pub to_wxid: String,
    pub content: String,
    #[serde(rename = "msgtype")]
    pub msg_type: String,
    pub context_token: Option<String>,
}

// POST /cgi-bin/sendMessage response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendMessageResponse {
    pub errcode: i32,
    pub errmsg: Option<String>,
}

// GET /cgi-bin/getBotQrcode response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetQrcodeResponse {
    pub qrcode: Option<String>,
    pub qrcode_url: Option<String>,
    pub qrcode_id: Option<String>,
}

// POST /cgi-bin/loginByQrcode response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginByQrcodeResponse {
    pub errcode: i32,
    pub errmsg: Option<String>,
    pub token: Option<String>,
    pub account_id: Option<String>,
    pub base_url: Option<String>,
}
```

---

## Implementation Pattern: Channel Trait

```rust
#[async_trait]
impl Channel for WeChatClient {
    fn name(&self) -> &str {
        "wechat"
    }

    fn is_enabled(&self) -> bool {
        self.token.is_some()
    }

    async fn send(&self, jid: &str, message: &str) -> Result<()> {
        // Use ilink sendMessage API
        // Include context_token if available
    }

    async fn start(&self) -> Result<()> {
        // Start the getUpdates polling loop
        // Extract trigger, check allow_from, check duplicates
        // Process messages and dispatch to agent
    }
}
```

---

## QR Code Login Implementation

```rust
impl WeChatClient {
    /// Request QR code from ilink gateway
    pub async fn request_qr_code(&mut self) -> Result<Option<String>> {
        let url = format!("{}/cgi-bin/getBotQrcode?bot_type=3", self.api_url);
        
        let response = self.http_get(&url).await?;
        
        let qr: GetQrcodeResponse = serde_json::from_str(&response)?;
        
        // Return QR code URL for display (ASCII or link)
        Ok(qr.qrcode_url.or(qr.qrcode))
    }
    
    /// Poll for QR code scan confirmation
    pub async fn poll_login(&mut self, qrcode_id: &str, timeout_secs: u64) -> Result<()> {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(timeout_secs);
        
        while tokio::time::Instant::now() < deadline {
            let response = self.http_post(
                &format!("{}/cgi-bin/loginByQrcode", self.api_url),
                &serde_json::json!({"qrcode_id": qrcode_id}),
            ).await?;
            
            let login: LoginByQrcodeResponse = serde_json::from_str(&response)?;
            
            match login.errcode {
                0 => {
                    // Success!
                    self.token = login.token;
                    self.account_id = login.account_id;
                    if let Some(base_url) = login.base_url {
                        self.api_url = base_url;
                    }
                    return Ok(());
                }
                -1 | -2 => {
                    // Still scanning / not scanned yet — continue polling
                    tokio::time::sleep(Duration::from_secs(2)).await;
                }
                _ => {
                    return Err(NuClawError::Auth {
                        message: format!("QR login failed: {:?}", login.errmsg),
                    });
                }
            }
        }
        
        Err(NuClawError::Auth {
            message: "QR code scan timeout".to_string(),
        })
    }
}
```

---

## getUpdates Polling Loop

```rust
async fn poll_messages(&self) -> Result<Vec<IlinkMessage>> {
    let url = format!(
        "{}/cgi-bin/getUpdates?timeout={}",
        self.api_url,
        DEFAULT_LONG_POLL_TIMEOUT_MS
    );
    
    let response = self.http_get_with_auth(&url).await?;
    let updates: GetUpdatesResponse = serde_json::from_str(&response)?;
    
    Ok(updates.list)
}

async fn process_message(&self, msg: &IlinkMessage) -> Result<Option<String>> {
    // 1. Check allow_from
    if !is_allowed_sender(&msg.from_wxid, &self.allow_from) {
        return Ok(None);
    }
    
    // 2. Check for trigger
    let (trigger, content) = match extract_trigger(&msg.content, &self.assistant_name) {
        Some((t, c)) => (t, c),
        None => return Ok(None), // No trigger, skip
    };
    
    // 3. Create NewMessage and process via agent
    let new_msg = NewMessage {
        id: msg.msg_id.clone(),
        chat_jid: msg.from_wxid.clone(),
        sender_jid: msg.from_wxid.clone(),
        content,
        timestamp: chrono::Utc::now(),
    };
    
    // 4. Dispatch to agent runner
    // 5. Send response via sendMessage
    
    Ok(Some(response))
}
```

---

## Media Handling (AES-128-ECB Decryption)

```rust
/// Download and decrypt media from WeChat CDN
pub async fn download_media(&self, cdn_url: &str, file_key: &str) -> Result<Vec<u8>> {
    // 1. Download encrypted data
    let encrypted = self.http_get_bytes(cdn_url).await?;
    
    // 2. Extract AES key from file_key (format: "file_id|key")
    let parts: Vec<&str> = file_key.splitn(2, '|').collect();
    if parts.len() != 2 {
        return Err(NuClawError::Api {
            message: "Invalid file_key format".to_string(),
        });
    }
    
    let aes_key = parts[1];
    let key_bytes = hex::decode(aes_key)?;
    
    // 3. Decrypt with AES-128-ECB
    let decrypted = aes_decrypt_ecb(&encrypted, &key_bytes)?;
    
    Ok(decrypted)
}

/// AES-128-ECB decryption using aes crate
fn aes_decrypt_ecb(data: &[u8], key: &[u8]) -> Result<Vec<u8>> {
    use aes::Aes128;
    use aes::cipher::{BlockDecryptMut, KeyInit};
    use aes::Aes128Ecb;
    
    let cipher = Aes128Ecb::new_from_slices(key)
        .map_err(|e| NuClawError::Api {
            message: format!("Invalid AES key: {}", e),
        })?;
    
    // Decrypt in 16-byte blocks
    let mut result = Vec::new();
    for chunk in data.chunks(16) {
        let mut block = [0u8; 16];
        block[..chunk.len()].copy_from_slice(chunk);
        let decrypted = cipher.decrypt_block_mut(&mut block.into());
        result.extend_from_slice(&decrypted[..chunk.len()]);
    }
    
    Ok(result)
}
```

---

## Helper Functions (Pure Functions)

```rust
/// Check if sender is allowed (pure function)
pub fn is_allowed_sender_pure(sender: &str, allow_from: &[String]) -> bool {
    if allow_from.is_empty() || allow_from.iter().any(|w| w == "*") {
        return true;
    }
    allow_from.iter().any(|w| sender.contains(w))
}

/// Extract trigger and content from message (pure function)
pub fn extract_trigger_pure(content: &str, assistant_name: &str) -> Option<(String, String)> {
    // Support formats: "@Andy hello", "Andy hello", etc.
    let patterns = [
        format!("@{}", assistant_name),
        assistant_name.to_string(),
    ];
    
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

/// Check for duplicate message (pure function)
pub fn is_duplicate_message_pure(
    msg: &NewMessage,
    last_message_ids: &HashMap<String, String>,
) -> bool {
    last_message_ids.get(&msg.chat_jid).map(|id| id == &msg.id).unwrap_or(false)
}
```

---

## Configuration (from Environment)

```rust
impl WeChatClient {
    pub fn from_env() -> Self {
        Self {
            api_url: std::env::var("WEIXIN_API_URL")
                .unwrap_or_else(|_| "https://ilinkai.weixin.qq.com".to_string()),
            token: std::env::var("WEIXIN_TOKEN").ok(),
            account_id: std::env::var("WEIXIN_ACCOUNT_ID").ok(),
            cdn_base_url: std::env::var("WEIXIN_CDN_BASE_URL").ok(),
            allow_from: std::env::var("WEIXIN_ALLOW_FROM")
                .ok()
                .map(|s| s.split(',').map(str::to_string).collect())
                .unwrap_or_default(),
            registered_groups: load_registered_groups(),
            router_state: load_router_state(),
            db: Database::new().unwrap(),
            assistant_name: assistant_name(),
        }
    }
}
```

---

## Anti-Patterns (CRITICAL - NEVER DO THESE)

### ❌ NEVER Reimplement Existing Types

```rust
// ❌ BAD: Reimplementing types that already exist in NuClaw
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MyNewMessage {
    pub id: String,
    pub content: String,
    pub timestamp: i64,
}

// ✅ GOOD: Use existing types
use crate::types::{NewMessage, RegisteredGroup, RouterState};
```

### ❌ NEVER Use Empty Error Handling

```rust
// ❌ BAD: Empty catch
let response = client.get(&url).await.ok();

// ✅ GOOD: Proper error mapping
let response = client.get(&url).await
    .map_err(|e| NuClawError::Api {
        message: format!("ilink request failed: {}", e),
    })?;
```

### ❌ NEVER Hardcode Secrets

```rust
// ❌ BAD: Hardcoded token
let token = "sk-123456";

// ✅ GOOD: From environment or config
let token = std::env::var("WEIXIN_TOKEN")
    .ok()
    .filter(|t| !t.is_empty());
```

### ❌ NEVER Skip allow_from Validation

```rust
// ❌ BAD: No access control
async fn process_message(&self, msg: &IlinkMessage) -> Result<()> {
    // Process everything
}
```

### ❌ NEVER Create Custom Error Types — USE NuClawError

```rust
// ❌ BAD: Creating QrLoginError, WeChatError, or any custom error enum
#[derive(Error, Debug)]
pub enum WeChatError {
    #[error("API error: {message}")]
    Api { message: String },
}

#[derive(Error, Debug)]
pub enum QrLoginError {
    #[error("QR timeout")]
    QrTimeout,
}

// ❌ BAD: Even if you think you need specific variants
impl From<reqwest::Error> for WeChatError { ... }
```

The ONLY correct error type is `NuClawError::WeChat { message: String }` from `crate::error::NuClawError`.

NuClawError already has ALL variants you need:
- `NuClawError::Api { message }` — for HTTP/network errors
- `NuClawError::Auth { message }` — for auth failures
- `NuClawError::WeChat { message }` — for WeChat-specific errors (use this for ilink errors)
- `NuClawError::Timeout { operation }` — for timeouts
- `NuClawError::Validation { message }` — for invalid input

For detailed ilink errcode mapping, use a helper function that returns `Option<NuClawError>`:

```rust
fn ilink_errcode_to_error(errcode: i32, errmsg: Option<&str>) -> Option<NuClawError> {
    match errcode {
        0 => None,
        -14 => Some(NuClawError::Auth {
            message: "Session expired (errcode -14). Need re-login.".to_string(),
        }),
        _ => Some(NuClawError::WeChat {
            message: format!("ilink error {}: {}", errcode, errmsg.unwrap_or("unknown")),
        }),
    }
}
```

**All error types in generated code MUST be NuClawError variants.** No custom error enums, no `pub enum WeChatError`, no `pub enum QrLoginError`.

### ❌ NEVER Generate Partial Modules — ALWAYS Include Complete Module Structure

Every generated file/module MUST be a complete, compilable Rust module. If your focus is QR login, you STILL must include:

1. Module documentation (`//!`)
2. All required imports (std → external → crate)
3. Error handling via NuClawError (NEVER custom errors)
4. All ilink API types (at minimum: `IlinkMessage`, `SendMessageRequest`, `GetUpdatesResponse`, `GetQrcodeResponse`, `LoginByQrcodeResponse`)
5. Full WeChatClient struct with ALL fields
6. QR login functions (request_qr_code, poll_login)
7. Media handling (download_media with AES decryption)
8. Security helpers (is_allowed_sender_pure, deduplication)
9. HTTP client methods (http_get_with_auth, http_post_json)
10. Context token management
11. Comprehensive tests for ALL public functions

Do NOT generate "just the QR login" or "just the error handling" as a standalone file. Generate the COMPLETE module that includes ALL features. You can focus on improving one aspect, but the full module structure must be present.

// ✅ GOOD: Validate sender
async fn process_message(&self, msg: &IlinkMessage) -> Result<()> {
    if !is_allowed_sender_pure(&msg.from_wxid, &self.allow_from) {
        return Ok(());
    }
}
```

---

## Naming Conventions

| Type | Convention | Example |
|------|------------|---------|
| Modules | `snake_case` | `wechat` |
| Types | `PascalCase` | `WeChatClient`, `IlinkMessage` |
| Functions | `snake_case` | `is_allowed_sender_pure`, `extract_trigger_pure` |
| Variables | `snake_case` | `account_id`, `cdn_base_url` |
| Constants | `SCREAMING_SNAKE_CASE` | `DEFAULT_LONG_POLL_TIMEOUT_MS` |

---

## Output Format

When generating the WeChat module:

1. **Start with module documentation** (`//! WeChat Integration`)
2. **Follow import order** (std → external → crate)
3. **Define constants first**
4. **Define error types**
5. **Define ilink API types** (request/response structs)
6. **Define client struct**
7. **Implement ilink API methods**
8. **Implement Channel trait**
9. **Add pure helper functions**
10. **Add tests** in `#[cfg(test)]` blocks
11. **Import existing types with `use crate::`** — do NOT duplicate

---

## Testing Requirements

Generated code MUST include tests for:

### Pure Function Tests
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_allowed_sender_open() {
        assert!(is_allowed_sender_pure("user@im.wechat", &[]));
        assert!(is_allowed_sender_pure("user@im.wechat", &["*".to_string()]));
    }

    #[test]
    fn test_is_allowed_sender_allowlist() {
        let allow = vec!["alice@im.wechat".to_string(), "bob@im.wechat".to_string()];
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
        let (trigger, content) = extract_trigger_pure("Andy hello", "Andy").unwrap();
        assert_eq!(trigger, "Andy");
        assert_eq!(content, "hello");
    }

    #[test]
    fn test_extract_trigger_no_match() {
        assert!(extract_trigger_pure("hello world", "Andy").is_none());
    }

    #[test]
    fn test_duplicate_detection() {
        let msg = NewMessage {
            id: "msg123".to_string(),
            chat_jid: "chat1".to_string(),
            sender_jid: "user1".to_string(),
            content: "test".to_string(),
            timestamp: chrono::Utc::now(),
        };
        let last_ids = HashMap::from([("chat1".to_string(), "msg123".to_string())]);
        assert!(is_duplicate_message_pure(&msg, &last_ids));
        assert!(!is_duplicate_message_pure(&msg, &HashMap::new()));
    }
}
```

### Integration Test Notes
- Mock HTTP responses for ilink API calls
- Test QR login flow with timeout
- Test message processing with trigger extraction
- Test media download and decryption

---

## File Header Template

```rust
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

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::time::Duration;
use tracing::{debug, error, info, warn};

use crate::config::{assistant_name, data_dir};
use crate::db::Database;
use crate::error::{NuClawError, Result};
use crate::types::{NewMessage, RegisteredGroup, RouterState};
use crate::utils::json::{load_json, save_json};
// ... rest of implementation
```
