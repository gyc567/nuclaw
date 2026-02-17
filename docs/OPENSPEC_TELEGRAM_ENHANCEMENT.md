# OpenSpec 提案 - Telegram Bot API 增强

## 执行摘要

本提案参考 OpenClaw Telegram 实现，对 NuClaw Telegram 模块进行增强。主要改进：

1. **增强访问控制** - 支持 pairing、allowlist、open、disabled 四种 DM/Group 策略
2. **流式预览模式** - 支持 partial/block 模式实现实时消息预览
3. **群组/主题隔离** - 支持 Forum 主题会话隔离
4. **消息分块优化** - 支持 newline 模式优先按段落分割
5. **内联按钮支持** - 支持回调按钮交互
6. **消息操作** - 支持编辑、删除、反应等 API 操作
7. **配置写入** - 支持从 Telegram 事件更新配置

## 架构设计

```
┌─────────────────────────────────────────────────────┐
│              TelegramClient                          │
├─────────────────────────────────────────────────────┤
│  ┌─────────────┐  ┌─────────────┐  ┌───────────┐ │
│  │ DMPolicy    │  │GroupPolicy │  │ChunkMode  │ │
│  │ - Pairing   │  │ - Open     │  │ - Length  │ │
│  │ - Allowlist │  │- Allowlist │  │ - Newline │ │
│  │ - Open      │  │- Disabled  │  │           │ │
│  │ - Disabled  │  │             │  │           │ │
│  └─────────────┘  └─────────────┘  └───────────┘ │
│  ┌─────────────┐  ┌─────────────┐  ┌───────────┐ │
│  │ StreamMode  │  │ ReplyMode  │  │Capability │ │
│  │ - Off      │  │ - Off      │  │ - Inline  │ │
│  │ - Partial  │  │ - First    │  │ - Buttons │ │
│  │ - Block    │  │ - All      │  │           │ │
│  └─────────────┘  └─────────────┘  └───────────┘ │
└─────────────────────────────────────────────────────┘
```

## 详细设计

### 1. 配置结构增强

```rust
// src/telegram.rs 新增

/// Stream mode for message preview
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum StreamMode {
    #[serde(rename = "off")]
    Off,
    #[serde(rename = "partial")]
    Partial,
    #[serde(rename = "block")]
    Block,
}

/// Chunk mode for text splitting
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum ChunkMode {
    #[serde(rename = "length")]
    Length,
    #[serde(rename = "newline")]
    Newline,
}

/// Reply mode for threading
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum ReplyMode {
    #[serde(rename = "off")]
    Off,
    #[serde(rename = "first")]
    First,
    #[serde(rename = "all")]
    All,
}

/// Group configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelegramGroupConfig {
    pub group_policy: Option<GroupPolicy>,
    pub require_mention: Option<bool>,
    pub allow_from: Option<Vec<String>>,
    pub enabled: Option<bool>,
}

/// Topic configuration (for forum supergroups)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelegramTopicConfig {
    pub group_policy: Option<GroupPolicy>,
    pub require_mention: Option<bool>,
    pub enabled: Option<bool>,
}

/// Inline button callback data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InlineButton {
    pub text: String,
    pub callback_data: String,
}

/// Message action types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TelegramAction {
    Send {
        to: String,
        content: String,
        reply_to: Option<String>,
        buttons: Option<Vec<Vec<InlineButton>>>,
    },
    React {
        chat_id: String,
        message_id: String,
        emoji: String,
    },
    Delete {
        chat_id: String,
        message_id: String,
    },
    Edit {
        chat_id: String,
        message_id: String,
        content: String,
    },
}
```

### 2. TelegramClient 增强

```rust
pub struct TelegramClient {
    // 现有字段...
    
    // 新增配置
    pub stream_mode: StreamMode,
    pub chunk_mode: ChunkMode,
    pub reply_mode: ReplyMode,
    pub link_preview: bool,
    pub text_chunk_limit: usize,
    pub draft_chunk_min_chars: usize,
    pub draft_chunk_max_chars: usize,
    
    // 群组配置
    pub groups: HashMap<String, TelegramGroupConfig>,
    pub topics: HashMap<String, HashMap<i64, TelegramTopicConfig>>,
    
    // 内联按钮回调处理
    pub callback_handlers: HashMap<String, Box<dyn TelegramCallbackHandler>>,
    
    // 流式消息状态
    pub streaming_messages: HashMap<String, StreamingMessageState>,
}

struct StreamingMessageState {
    message_id: String,
    chat_id: String,
    preview_message_id: Option<String>,
    content: String,
}
```

### 3. 关键功能实现

#### 3.1 消息分块（支持 Newline 模式）

```rust
/// Chunk text with newline preference
pub fn chunk_text_advanced(text: &str, chunk_limit: usize, mode: ChunkMode) -> Vec<String> {
    match mode {
        ChunkMode::Length => chunk_text_pure(text, chunk_limit),
        ChunkMode::Newline => chunk_text_by_newline(text, chunk_limit),
    }
}

fn chunk_text_by_newline(text: &str, max_len: usize) -> Vec<String> {
    let mut chunks = Vec::new();
    let mut current = String::new();
    
    for paragraph in text.split("\n\n") {
        if current.is_empty() {
            current = paragraph.to_string();
        } else if current.len() + paragraph.len() + 2 <= max_len {
            current.push_str(&format!("\n\n{}", paragraph));
        } else {
            if !current.is_empty() {
                chunks.push(current);
            }
            current = paragraph.to_string();
        }
    }
    
    if !current.is_empty() {
        chunks.push(current);
    }
    
    chunks
}
```

#### 3.2 流式预览消息

```rust
impl TelegramClient {
    /// Send streaming preview message
    async fn send_streaming_preview(
        &mut self,
        chat_id: &str,
        initial_content: &str,
    ) -> Result<StreamingMessageState> {
        let message = self.send_message(
            chat_id,
            initial_content,
            None,
            None,
        ).await?;
        
        let state = StreamingMessageState {
            message_id: message.id,
            chat_id: chat_id.to_string(),
            preview_message_id: Some(message.id),
            content: initial_content.to_string(),
        };
        
        self.streaming_messages.insert(message.id.clone(), state);
        Ok(state)
    }
    
    /// Update streaming message
    async fn update_streaming_message(
        &mut self,
        state: &mut StreamingMessageState,
        new_content: &str,
    ) -> Result<()> {
        state.content = new_content.to_string();
        
        if let Some(preview_id) = &state.preview_message_id {
            self.edit_message(&state.chat_id, preview_id, new_content).await?;
        }
        
        Ok(())
    }
    
    /// Finalize streaming message
    async fn finalize_streaming_message(
        &mut self,
        state: StreamingMessageState,
    ) -> Result<()> {
        self.streaming_messages.remove(&state.message_id);
        
        // Keep preview message (no cleanup for text-only)
        Ok(())
    }
}
```

#### 3.3 群组/主题隔离

```rust
impl TelegramClient {
    /// Get session key for a chat (includes topic for forums)
    fn get_session_key(&self, chat_id: &str, message_thread_id: Option<i64>) -> String {
        match message_thread_id {
            Some(thread_id) if thread_id != 1 => {
                format!("{}:topic:{}", chat_id, thread_id)
            }
            _ => chat_id.to_string(),
        }
    }
    
    /// Check if group is enabled
    async fn is_group_enabled(&self, chat_id: &str) -> bool {
        match self.groups.get(chat_id) {
            Some(config) => config.enabled.unwrap_or(true),
            None => true, // Default: enabled
        }
    }
    
    /// Get effective group policy (with inheritance)
    fn get_effective_group_policy(
        &self,
        chat_id: &str,
        thread_id: Option<i64>,
    ) -> GroupPolicy {
        // Check topic config first
        if let Some(thread_id) = thread_id {
            if let Some(topics) = self.topics.get(chat_id) {
                if let Some(topic_config) = topics.get(&thread_id) {
                    if let Some(policy) = topic_config.group_policy {
                        return policy;
                    }
                }
            }
        }
        
        // Check group config
        if let Some(config) = self.groups.get(chat_id) {
            if let Some(policy) = config.group_policy {
                return policy;
            }
        }
        
        // Default to allowlist
        GroupPolicy::Allowlist
    }
}
```

#### 3.4 内联按钮回调处理

```rust
#[async_trait]
pub trait TelegramCallbackHandler: Send + Sync {
    async fn handle(&self, callback_id: &str, data: &str, message: &TelegramMessage) -> Result<()>;
}

/// Handle callback query
async fn handle_callback_query(
    client: &TelegramClient,
    callback: &TelegramCallback,
) -> Result<()> {
    if let Some(handler) = client.callback_handlers.get(&callback.data) {
        handler.handle(&callback.id, &callback.data, &callback.message).await?;
    }
    
    // Answer callback to remove loading state
    client.answer_callback_query(&callback.id).await?;
    
    Ok(())
}
```

### 4. 环境变量配置

| 变量 | 默认值 | 描述 |
|------|--------|------|
| `TELEGRAM_BOT_TOKEN` | - | Bot token (必需) |
| `TELEGRAM_DM_POLICY` | pairing | DM 策略 |
| `TELEGRAM_GROUP_POLICY` | allowlist | 群组策略 |
| `TELEGRAM_GROUPS` | - | 允许的群组 ID 列表 |
| `TELEGRAM_REQUIRE_MENTION` | true | 是否需要 @mention |
| `TELEGRAM_STREAM_MODE` | partial | 流式预览模式 |
| `TELEGRAM_CHUNK_MODE` | length | 消息分块模式 |
| `TELEGRAM_TEXT_CHUNK_LIMIT` | 4000 | 最大块大小 |
| `TELEGRAM_LINK_PREVIEW` | true | 启用链接预览 |
| `TELEGRAM_WEBHOOK_URL` | - | Webhook 模式 URL |
| `TELEGRAM_WEBHOOK_SECRET` | - | Webhook 密钥 |
| `TELEGRAM_WEBHOOK_PATH` | telegram-webhook | Webhook 路径 |
| `TELEGRAM_WEBHOOK_BIND` | 0.0.0.0:8787 | Webhook 绑定地址 |

## 测试计划

### 测试用例

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_chunk_text_by_newline() {
        let text = "Para1\n\nPara2\n\nPara3";
        let chunks = chunk_text_by_newline(text, 20);
        assert_eq!(chunks.len(), 3);
    }
    
    #[test]
    fn test_chunk_text_by_newline_combines_short() {
        let text = "A\n\nB";
        let chunks = chunk_text_by_newline(text, 100);
        assert_eq!(chunks.len(), 1);
    }
    
    #[test]
    fn test_session_key_with_topic() {
        let client = TelegramClient::default();
        assert_eq!(
            client.get_session_key("-100123", Some(5)),
            "-100123:topic:5"
        );
    }
    
    #[test]
    fn test_session_key_without_topic() {
        let client = TelegramClient::default();
        assert_eq!(
            client.get_session_key("-100123", None),
            "-100123"
        );
    }
    
    #[test]
    fn test_session_key_general_topic() {
        let client = TelegramClient::default();
        assert_eq!(
            client.get_session_key("-100123", Some(1)),
            "-100123"
        );
    }
    
    #[test]
    fn test_dm_policy_parse() {
        assert_eq!(DMPolicy::parse("pairing"), DMPolicy::Pairing);
        assert_eq!(DMPolicy::parse("allowlist"), DMPolicy::Allowlist);
        assert_eq!(DMPolicy::parse("open"), DMPolicy::Open);
        assert_eq!(DMPolicy::parse("disabled"), DMPolicy::Disabled);
    }
    
    #[test]
    fn test_group_policy_parse() {
        assert_eq!(GroupPolicy::parse("open"), GroupPolicy::Open);
        assert_eq!(GroupPolicy::parse("allowlist"), GroupPolicy::Allowlist);
        assert_eq!(GroupPolicy::parse("disabled"), GroupPolicy::Disabled);
    }
    
    #[test]
    fn test_stream_mode_parse() {
        assert_eq!(StreamMode::parse("off"), StreamMode::Off);
        assert_eq!(StreamMode::parse("partial"), StreamMode::Partial);
        assert_eq!(StreamMode::parse("block"), StreamMode::Block);
    }
    
    #[test]
    fn test_chunk_mode_parse() {
        assert_eq!(ChunkMode::parse("length"), ChunkMode::Length);
        assert_eq!(ChunkMode::parse("newline"), ChunkMode::Newline);
    }
    
    #[test]
    fn test_reply_mode_parse() {
        assert_eq!(ReplyMode::parse("off"), ReplyMode::Off);
        assert_eq!(ReplyMode::parse("first"), ReplyMode::First);
        assert_eq!(ReplyMode::parse("all"), ReplyMode::All);
    }
    
    #[test]
    fn test_inline_button_serialization() {
        let button = InlineButton {
            text: "Yes".to_string(),
            callback_data: "yes".to_string(),
        };
        let json = serde_json::to_string(&button).unwrap();
        assert!(json.contains("Yes"));
        assert!(json.contains("yes"));
    }
    
    #[test]
    fn test_truncate_with_newline_mode() {
        let text = "Short\n\nMedium length paragraph\n\nAnother short";
        let chunks = chunk_text_by_newline(text, 50);
        // Should respect paragraph boundaries
        for chunk in &chunks {
            assert!(chunk.len() <= 50);
        }
    }
}
```

### 覆盖率目标

| 模块 | 目标覆盖率 |
|------|-----------|
| telegram.rs | 95%+ |

## 验收标准

- [ ] 支持四种 DM/Group 策略
- [ ] 支持流式预览 (partial/block)
- [ ] 支持群组/主题会话隔离
- [ ] 支持 newline 模式分块
- [ ] 支持内联按钮
- [ ] 支持消息操作 (send/react/delete/edit)
- [ ] 测试覆盖率 95%+
- [ ] 通过 clippy 检查
- [ ] 向后兼容现有配置

## 风险评估

| 风险 | 影响 | 缓解 |
|------|------|------|
| 向后兼容问题 | 中 | 默认值保持现有行为 |
| Webhook 安全性 | 高 | 添加 webhook secret 验证 |
| 主题配置复杂度 | 低 | 渐进式实现 |

---

**提案版本**: v1.0  
**生成日期**: 2026-02-17  
**状态**: ✅ 已实现（部分）
