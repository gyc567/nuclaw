# OpenSpec 提案 - 纯 Rust Claude API Runner

## 执行摘要

本提案提出通过添加纯 Rust Anthropic API 客户端支持来减少对 Node.js 的强制依赖。当前实现依赖 Docker 容器中的 Claude Code CLI（Node.js 包装），本提案将提供两种运行模式：
- **容器模式**（现有）：通过 Docker/Apple Container 运行 Claude Code
- **API 模式**（新增）：直接调用 Anthropic API

## 问题分析

### 当前架构

```
NuClaw → Docker Container → Claude Code (Node.js) → Anthropic API
```

问题：
1. **强制依赖 Node.js**：Claude Code 是 Node.js 应用
2. **资源消耗大**：每个请求启动完整容器
3. **启动延迟高**：容器启动 + Node.js 初始化
4. **维护复杂**：需要管理 Claude Code 版本

### 当前 Node.js 依赖位置

| 文件 | 行号 | 依赖描述 |
|------|------|----------|
| container_runner.rs | 187 | `cat /workspace/input.json \| /usr/local/bin/claude` |

## 解决方案

### 架构设计

```
┌─────────────────────────────────────────────┐
│              NuClaw Application             │
│                                             │
│  ┌─────────────────────────────────────┐   │
│  │        Agent Runner (Trait)          │   │
│  └─────────────────────────────────────┘   │
│            ▲                  ▲            │
│            │                  │            │
│   ┌────────┴─────┐    ┌───────┴────────┐   │
│   │ContainerRunner│    │  ApiRunner    │   │
│   │   (现有)     │    │   (新增)       │   │
│   └──────────────┘    └────────────────┘   │
└─────────────────────────────────────────────┘
```

### 设计模式：Strategy Pattern

```rust
pub trait AgentRunner {
    async fn run(&self, input: ContainerInput) -> Result<ContainerOutput>;
}

pub struct ContainerRunner;
pub struct ApiRunner;
```

### KISS 原则实现

1. **最小新增代码**：只添加必要的 trait 和 ApiRunner
2. **不修改现有逻辑**：ContainerRunner 保持不变
3. **配置驱动**：通过环境变量切换模式

### 高内聚低耦合

- `AgentRunner` trait 定义运行接口
- `ApiRunner` 独立模块，只依赖外部 SDK
- 配置层与实现层分离

## 实施计划

### 阶段一：基础设施（API Runner 模块）

| 任务 | 描述 | 复杂度 |
|------|------|--------|
| 1.1 | 添加 `anthropic_rust` 依赖到 Cargo.toml | 低 |
| 1.2 | 创建 `src/agent_runner.rs` 定义 trait | 低 |
| 1.3 | 实现 `ApiRunner` 结构体 | 中 |
| 1.4 | 添加配置项 `AGENT_RUNNER_MODE` | 低 |

### 阶段二：核心实现

| 任务 | 描述 | 复杂度 |
|------|------|--------|
| 2.1 | 实现 Anthropic API 调用逻辑 | 中 |
| 2.2 | 实现流式响应处理 | 中 |
| 2.3 | 实现工具调用支持（可选） | 高 |
| 2.4 | 添加超时和重试逻辑 | 低 |

### 阶段三：测试

| 任务 | 描述 | 覆盖率目标 |
|------|------|-----------|
| 3.1 | 单元测试：API 请求构建 | 100% |
| 3.2 | 单元测试：响应解析 | 100% |
| 3.3 | 单元测试：配置加载 | 100% |
| 3.4 | Mock 测试：API 交互 | 100% |

### 阶段四：集成

| 任务 | 描述 |
|------|------|
| 4.1 | 更新 main.rs 支持模式切换 |
| 4.2 | 更新部署脚本（如需要） |
| 4.3 | 文档更新 |

## API Runner 详细设计

### 配置文件结构

```rust
// src/config.rs 新增
pub fn agent_runner_mode() -> AgentRunnerMode {
    match std::env::var("AGENT_RUNNER_MODE").as_deref() {
        Ok("api") => AgentRunnerMode::Api,
        Ok("container") | _ => AgentRunnerMode::Container,
    }
}

pub enum AgentRunnerMode {
    Container,  // 默认：使用 Docker/Apple Container
    Api,        // 直接调用 Anthropic API
}
```

### Agent Runner Trait

```rust
use async_trait::async_trait;
use crate::types::{ContainerInput, ContainerOutput};

#[async_trait]
pub trait AgentRunner: Send + Sync {
    async fn run(&self, input: ContainerInput) -> Result<ContainerOutput>;
}
```

### ApiRunner 实现要点

```rust
pub struct ApiRunner {
    client: Anthropic,
    max_retries: u32,
    timeout: Duration,
}

impl ApiRunner {
    pub fn new() -> Result<Self> {
        let api_key = anthropic_api_key().ok_or_else(|| ...)?;
        let client = Anthropic::new(api_key);
        Ok(Self { client, .. })
    }
}

#[async_trait]
impl AgentRunner for ApiRunner {
    async fn run(&self, input: ContainerInput) -> Result<ContainerOutput> {
        // 1. 构建 API 请求
        let request = build_message_request(&input);
        
        // 2. 发送请求（支持流式）
        let response = self.client.messages.create(request).await?;
        
        // 3. 解析响应
        parse_message_response(response)
    }
}
```

### ContainerInput 到 Anthropic 消息格式

```rust
fn build_message_request(input: &ContainerInput) -> MessageRequest {
    let system = build_system_prompt(input);
    let user_message = MessageContent::Text(input.prompt.clone());
    
    MessageRequest::builder("claude-sonnet-4-20250514")
        .system(system)
        .user_message(user_message)
        .max_tokens(4096)
        .build()
}
```

## 测试策略

### 测试覆盖率目标

| 模块 | 目标覆盖率 |
|------|-----------|
| agent_runner.rs | 100% |
| config.rs (新增) | 100% |

### 测试用例清单

```rust
#[cfg(test)]
mod tests {
    // 配置测试
    #[test]
    fn test_agent_runner_mode_container() { ... }
    #[test]
    fn test_agent_runner_mode_api() { ... }
    #[test]
    fn test_agent_runner_mode_invalid() { ... }
    
    // API 请求构建测试
    #[test]
    fn test_build_message_request_basic() { ... }
    #[test]
    fn test_build_message_request_with_model() { ... }
    #[test]
    fn test_build_system_prompt() { ... }
    
    // 响应解析测试
    #[test]
    fn test_parse_message_response_success() { ... }
    #[test]
    fn test_parse_message_response_with_tools() { ... }
    #[test]
    fn test_parse_streaming_response() { ... }
}
```

## 配置项

### 新增环境变量

| 变量 | 默认值 | 描述 |
|------|--------|------|
| `AGENT_RUNNER_MODE` | container | 运行模式：`container` 或 `api` |
| `ANTHROPIC_API_KEY` | - | Anthropic API 密钥（API 模式必需） |
| `ANTHROPIC_BASE_URL` | api.anthropic.com | API 端点 |
| `CLAUDE_MODEL` | claude-sonnet-4-20250514 | 模型名称 |

### 现有配置复用

- `ANTHROPIC_API_KEY` - 已存在
- `ANTHROPIC_BASE_URL` - 已存在  
- `CLAUDE_MODEL` - 已存在

## 风险评估

| 风险 | 影响 | 缓解措施 |
|------|------|----------|
| API 模式不支持工具调用 | 中 | 文档说明，仅支持纯文本对话 |
| API 成本高于容器 | 低 | 用户可选择模式 |
| 引入新依赖 | 低 | 选择成熟稳定的 SDK |
| 破坏现有功能 | 高 | 保持默认模式为容器 |

## 验收标准

- [ ] 新增 `AGENT_RUNNER_MODE` 配置支持
- [ ] ApiRunner 实现 100% 测试覆盖
- [ ] 现有容器模式功能不受影响
- [ ] 代码通过 clippy 检查
- [ ] 文档更新

## 时间估算

| 阶段 | 预计时间 |
|------|---------|
| 基础设施 | 1 小时 |
| 核心实现 | 2-3 小时 |
| 测试 | 1-2 小时 |
| 集成与文档 | 1 小时 |
| **总计** | **5-7 小时** |

---

**提案版本**: v1.0  
**生成日期**: 2026-02-17  
**状态**: ✅ 已实现
