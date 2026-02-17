# OpenSpec 提案 - 内置技能 + 两步注册 Provider/Channel

## 执行摘要

本提案参考 HKUDS/nanobot 设计，实现：
1. **内置技能系统** - 可复用的 Agent 任务模板
2. **Provider 注册** - 两步注册新 LLM 模型提供商
3. **Channel 注册** - 两步注册新消息渠道

## 架构设计

```
┌─────────────────────────────────────────────────────┐
│                   NuClaw Core                       │
│                                                      │
│  ┌─────────────────────────────────────────────┐   │
│  │            Registry (Trait + Registry)       │   │
│  │  ┌──────────┐  ┌──────────┐  ┌──────────┐  │   │
│  │  │ Provider │  │ Channel  │  │  Skill   │  │   │
│  │  │ Registry │  │ Registry │  │ Registry │  │   │
│  │  └──────────┘  └──────────┘  └──────────┘  │   │
│  └─────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────┘
```

## 1. 内置技能系统 (Skills)

### 概念
技能是预定义的任务模板，让 Agent 可以执行特定操作。每个技能包含：
- `skill.md` - 技能描述和指令
- `system_prompt` - 注入到 Agent 的系统提示

### 技能目录结构
```
skills/
├── github.md      # GitHub 集成技能
├── weather.md     # 天气查询技能
├── search.md      # 网络搜索技能
└── memory.md      # 记忆管理技能
```

### 技能定义格式
```markdown
# Skill: GitHub
# Description: Manage GitHub repositories, issues, PRs

You are a GitHub assistant. You can help users with:
- Creating and managing repositories
- Working with issues and pull requests
- Searching code
```

### Rust 实现

```rust
pub struct Skill {
    pub name: String,
    pub description: String,
    pub content: String,
}

pub trait SkillRegistry: Send + Sync {
    fn get(&self, name: &str) -> Option<&Skill>;
    fn list(&self) -> Vec<&Skill>;
}

pub struct BuiltinSkillRegistry;
```

## 2. Provider 注册 (两步注册)

### 概念
Provider 是 LLM 模型提供商。两步注册参考 nanobot 设计：

**Step 1**: 在 `ProviderSpec` 注册表添加配置
**Step 2**: 在配置结构体添加字段

### 示例：添加新 Provider

```rust
// Step 1: 添加 ProviderSpec
pub struct ProviderSpec {
    pub name: &'static str,
    pub api_key_env: &'static str,
    pub base_url_env: &'static str,
    pub default_model: Option<&'static str>,
}

pub const PROVIDERS: &[ProviderSpec] = &[
    ProviderSpec {
        name: "openrouter",
        api_key_env: "OPENROUTER_API_KEY",
        base_url_env: "OPENROUTER_BASE_URL",
        default_model: Some("anthropic/claude-sonnet-4-20250514"),
    },
    // 添加新 provider 只需在这里添加一行
];

// Step 2: 配置已通过环境变量自动支持
// 无需修改代码！
```

### 现有支持
- `anthropic` - Anthropic API (Claude)
- `openai` - OpenAI API (GPT)
- `openrouter` - OpenRouter 网关
- `custom` - 自定义 OpenAI 兼容端点

### 使用方式
```bash
export ANTHROPIC_API_KEY=sk-xxx
# 或
export OPENROUTER_API_KEY=sk-or-v1-xxx
```

## 3. Channel 注册 (两步注册)

### 概念
Channel 是消息渠道（WhatsApp、Telegram 等）。

### 实现方式

```rust
pub trait Channel: Send + Sync {
    fn name(&self) -> &str;
    async fn send(&self, jid: &str, message: &str) -> Result<()>;
    async fn start(&self) -> Result<()>;
}

pub struct ChannelRegistry {
    channels: HashMap<String, Box<dyn Channel>>,
}

impl ChannelRegistry {
    pub fn register(&mut self, name: String, channel: Box<dyn Channel>);
    pub fn get(&self, name: &str) -> Option<&Box<dyn Channel>>;
}
```

### 两步注册 Channel

```rust
// Step 1: 实现 Channel trait
pub struct MyChannel {
    // 配置字段
}

impl Channel for MyChannel {
    fn name(&self) -> &str { "mychannel" }
    async fn send(&self, jid: &str, msg: &str) -> Result<()> { ... }
    async fn start(&self) -> Result<()> { ... }
}

// Step 2: 注册到 Registry
registry.register("mychannel", Box::new(MyChannel::new()?));
```

### 现有支持
- `whatsapp` - WhatsApp (via MCP)
- `telegram` - Telegram Bot API

## 实施计划

### 阶段一：Skills 模块

| 任务 | 描述 |
|------|------|
| 1.1 | 创建 `src/skills.rs` 模块 |
| 1.2 | 定义 `Skill` 结构体和 `SkillRegistry` trait |
| 1.3 | 实现内置技能加载器 |
| 1.4 | 添加 2-3 个示例技能 |

### 阶段二：Provider 注册

| 任务 | 描述 |
|------|------|
| 2.1 | 创建 `src/providers.rs` 模块 |
| 2.2 | 定义 `ProviderSpec` 和 `ProviderRegistry` |
| 2.3 | 从环境变量自动加载配置 |
| 2.4 | 集成到现有 `agent_runner` |

### 阶段三：Channel 注册

| 任务 | 描述 |
|------|------|
| 3.1 | 创建 `src/channels.rs` 模块 |
| 3.2 | 定义 `Channel` trait 和 `ChannelRegistry` |
| 3.3 | 重构现有 whatsapp/telegram 为注册式 |
| 3.4 | 添加 Channel 切换支持 |

### 阶段四：测试

| 任务 | 描述 |
|------|------|
| 4.1 | Skills 模块单元测试 |
| 4.2 | Provider 注册单元测试 |
| 4.3 | Channel 注册单元测试 |

## 测试覆盖率目标

| 模块 | 目标覆盖率 |
|------|-----------|
| skills.rs | 100% |
| providers.rs | 100% |
| channels.rs | 100% |

## 风险评估

| 风险 | 影响 | 缓解 |
|------|------|------|
| 重构破坏现有功能 | 高 | 保持向后兼容，默认行为不变 |
| 技能注入安全问题 | 中 | 技能内容需审核 |

## 验收标准

- [ ] Skills 模块可加载内置技能
- [ ] Provider 可通过环境变量自动配置
- [ ] Channel 可动态注册
- [ ] 所有新代码 100% 测试覆盖
- [ ] 现有功能不受影响

---

**提案版本**: v1.0  
**生成日期**: 2026-02-17  
**状态**: ✅ 已实现
