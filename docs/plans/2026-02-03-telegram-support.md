# Telegram 支持实现计划

## 目标

为 NuClaw 添加 Telegram Bot 支持，参照 OpenClaw 规范实现核心功能。

## OpenClaw Telegram 规范摘要

### 核心功能
- **Bot API**: 通过 webhook 模式集成 Telegram Bot API
- **消息标准化**: 入站消息转换为统一 envelope 格式
- **会话隔离**: `agent:<agentId>:telegram:group:<chatId>` 格式
- **策略控制**: DM策略(配对/白名单/开放/禁用) + 群组策略
- **流式输出**: 支持草稿流式/块流式

### 配置参数
| 参数 | 说明 |
|------|------|
| `TELEGRAM_BOT_TOKEN` | BotFather token |
| `TELEGRAM_WEBHOOK_URL` | Webhook URL |
| `TELEGRAM_WEBHOOK_PATH` | Webhook 路径 |
| `TELEGRAM_DM_POLICY` | DM策略: pairing/allowlist/open/disabled |
| `TELEGRAM_GROUP_POLICY` | 群组策略: open/allowlist/disabled |
| `TELEGRAM_STREAM_MODE` | 流式模式: partial/block/off |
| `TELEGRAM_TEXT_CHUNK_LIMIT` | 文本分块大小 |
| `TELEGRAM_WHITELIST_GROUPS` | 白名单群组ID |

---

## 实现计划

### Phase 1: telegram.rs 核心模块

#### 文件结构
```
src/
├── telegram.rs          # 主模块
├── types.rs             # Telegram 类型定义 (扩展)
└── config.rs            # 添加 Telegram 配置
```

#### Telegram 类型定义 (types.rs 扩展)
```rust
// Telegram 特定类型
pub struct TelegramMessage {
    pub id: String,
    pub chat_jid: String,          // 格式: telegram:group:<chat_id>
    pub chat_type: ChatType,       // private/group/supergroup/channel
    pub thread_id: Option<i64>,    // 论坛主题ID
    pub sender: String,            // sender_id
    pub sender_name: String,
    pub content: String,
    pub timestamp: i64,
    pub reply_to_message_id: Option<String>,
    pub is_from_me: bool,
}

pub enum ChatType {
    Private,
    Group,
    SuperGroup,
    Channel,
}

pub enum DMPolicy {
    Pairing,     // 需配对码
    Allowlist,   // 白名单
    Open,        // 开放
    Disabled,    // 禁用
}

pub enum GroupPolicy {
    Open,
    Allowlist,
    Disabled,
}

pub enum StreamMode {
    Partial,    // 草稿流式
    Block,      // 块流式
    Off,        // 关闭
}
```

#### 主模块结构 (telegram.rs)
```rust
pub struct TelegramClient {
    bot_token: String,
    api_url: String,
    webhook_path: String,
    dm_policy: DMPolicy,
    group_policy: GroupPolicy,
    stream_mode: StreamMode,
    text_chunk_limit: usize,
    registered_groups: HashMap<String, RegisteredGroup>,
    router_state: RouterState,
    db: Database,
    assistant_name: String,
}

impl TelegramClient {
    pub fn new(db: Database) -> Result<Self>
    pub async fn connect(&mut self) -> Result<()>
    pub async fn start_webhook_server(&mut self) -> Result<()>
    pub async fn handle_update(&mut self, update: &TelegramUpdate) -> Result<Option<String>>
    pub async fn send_message(&self, chat_id: &str, text: &str) -> Result<()>
}
```

### Phase 2: HTTP Server (Webhook 模式)

#### Axum Web Server
```rust
// 添加到 Cargo.toml
axum = { version = "0.7", features = ["json"] }
tower = { version = "0.4", features = ["util = { version = "1","] }
hyper features = ["full"] }

async fn start_webhook_server(client: Arc<TelegramClient>) {
    let app = Router::new()
        .route(&format!("/{}", config.telegram.webhook_path), post(handle_webhook))
        .route("/health", get(health_check));

    axum::Server::bind(&"0.0.0.0:8787".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();
}
```

### Phase 3: 消息处理

#### 消息路由
```rust
async fn handle_message(&mut self, msg: &TelegramMessage) -> Result<Option<String>> {
    // 1. 检查DM策略
    if msg.chat_type == ChatType::Private {
        if !self.check_dm_policy(&msg.sender).await? {
            return Ok(None);
        }
    }

    // 2. 检查群组策略
    if msg.is_group() {
        if !self.is_allowed_group(&msg.chat_jid).await? {
            return Ok(None);
        }
    }

    // 3. 提取触发词
    let trigger = self.extract_trigger(&msg.content)?;
    // 4. 调用容器执行
    // 5. 发送响应
}
```

### Phase 4: 工具函数

```rust
// 工具: send_message
pub async fn send_message(&self, chat_id: &str, text: &str) -> Result<()>

// 工具: delete_message
pub async fn delete_message(&self, chat_id: &str, message_id: &str) -> Result<()>

// 工具: get_chat_administrators
pub async fn get_chat_administrators(&self, chat_id: &str) -> Result<Vec<TelegramUser>>
```

### Phase 5: 配置集成

#### config.rs 扩展
```rust
pub fn telegram_bot_token() -> Result<String>
pub fn telegram_webhook_url() -> Option<String>
pub fn telegram_webhook_path() -> String
pub fn telegram_dm_policy() -> DMPolicy
pub fn telegram_group_policy() -> GroupPolicy
pub fn telegram_stream_mode() -> StreamMode
pub fn telegram_text_chunk_limit() -> usize
pub fn telegram_allowed_groups() -> Vec<String>
```

---

## 文件修改清单

| 文件 | 操作 | 说明 |
|------|------|------|
| `src/telegram.rs` | 新建 | Telegram 客户端模块 |
| `src/types.rs` | 修改 | 添加 Telegram 类型定义 |
| `src/config.rs` | 修改 | 添加 Telegram 配置函数 |
| `Cargo.toml` | 修改 | 添加 `axum`, `hyper`, `tower` 依赖 |
| `src/main.rs` | 修改 | 集成 `--telegram` 运行模式 |
| `src/lib.rs` | 修改 | 导出 telegram 模块 |

---

## 测试计划

### 单元测试 (6+ 测试)

| 测试项 | 说明 |
|--------|------|
| `test_parse_telegram_update` | 解析 Telegram Update |
| `test_extract_trigger_telegram` | 触发词提取 |
| `test_dm_policy_check` | DM策略检查 |
| `test_message_chunking` | 文本分块 |
| `test_chat_type_detection` | 聊天类型检测 |
| `test_send_message_format` | 消息发送格式 |

### 集成测试 (可选)

- Webhook 端点测试
- Bot API 模拟测试

---

## 风险和注意事项

| 风险 | 缓解措施 |
|------|----------|
| Webhook HTTPS | 使用 Telegram 要求 HTTPS |
| 消息频率限制 | 实现发送速率控制 |
| 内存泄漏 | 使用 Arc<TelegramClient> 管理状态 |

---

## 成功标准

1. ✅ Telegram 机器人可正常启动
2. ✅ 消息接收和处理流程正常
3. ✅ 群组隔离正确工作
4. ✅ 响应消息正确发送
5. ✅ 6+ 单元测试通过
6. ✅ 遵循 KISS 原则
