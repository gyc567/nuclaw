# NuClaw WORKFLOW 配置系统实现提案

## 1. 设计原则

### 1.1 KISS 原则
- **简单**: 最少概念，最少代码
- **可测试**: 每个函数都可单独测试
- **可逆**: 配置错误不影响服务启动

### 1.2 高内聚低耦合
- `WorkflowLoader`: 单一职责 - 加载配置
- `WorkflowConfig`: 纯数据 - 无副作用
- `HookRunner`: 独立模块 - 可替换

---

## 2. 文件结构

```
src/
├── workflow.rs       # 核心: 配置加载 + 类型定义
├── workflow/         # 目录
│   ├── mod.rs       # 模块入口
│   ├── config.rs    # 配置类型 (来自 workflow.rs)
│   ├── loader.rs    # 加载器 (来自 workflow.rs)
│   └── hooks.rs     # Hook 执行器 (新增)
```

---

## 3. 配置格式 (WORKFLOW.md)

```yaml
---
# 通道配置
channels:
  telegram:
    enabled: true
    bot_token: $TELEGRAM_BOT_TOKEN
  
  whatsapp:
    enabled: true
    mcp_url: $WHATSAPP_MCP_URL

# Agent 配置
agent:
  max_concurrent: 5
  timeout_ms: 300000
  max_retries: 3
  retry_backoff_ms: 60000

# 容器配置
container:
  image: "anthropic/codex:latest"
  workspace_root: ~/nuclaw/workspaces
  pool_enabled: true
  pool_min_size: 2
  pool_max_size: 5

# Hooks (可选)
hooks:
  after_create: |
    echo "Workspace created"
  before_run: |
    npm install
  after_run: |
    echo "Done"
  before_remove: |
    echo "Cleaning up"

# 消息模板 (可选)
prompt_template: |
  你是一个助手，帮助用户解决问题。
  用户说: {{ message }}
---

# 默认提示词
你是一个智能助手。
```

---

## 4. 实现细节

### 4.1 类型定义 (config.rs)

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WorkflowConfig {
    pub channels: ChannelSettings,
    pub agent: AgentSettings,
    pub container: ContainerSettings,
    pub hooks: HookSettings,
    #[serde(default)]
    pub prompt_template: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ChannelSettings {
    #[serde(default)]
    pub telegram: Option<ChannelConfig>,
    #[serde(default)]
    pub whatsapp: Option<ChannelConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelConfig {
    pub enabled: bool,
    #[serde(default)]
    pub bot_token: Option<String>,
    #[serde(default)]
    pub mcp_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSettings {
    #[serde(default = "default_max_concurrent")]
    pub max_concurrent: usize,
    
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,
    
    #[serde(default = "default_max_retries")]
    pub max_retries: usize,
    
    #[serde(default = "default_retry_backoff_ms")]
    pub retry_backoff_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ContainerSettings {
    #[serde(default)]
    pub image: Option<String>,
    
    #[serde(default)]
    pub workspace_root: Option<String>,
    
    #[serde(default)]
    pub pool_enabled: Option<bool>,
    
    #[serde(default)]
    pub pool_min_size: Option<usize>,
    
    #[serde(default)]
    pub pool_max_size: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HookSettings {
    #[serde(default)]
    pub after_create: Option<String>,
    #[serde(default)]
    pub before_run: Option<String>,
    #[serde(default)]
    pub after_run: Option<String>,
    #[serde(default)]
    pub before_remove: Option<String>,
}
```

### 4.2 加载器 (loader.rs)

```rust
pub struct WorkflowLoader;

impl WorkflowLoader {
    pub fn load(path: &Path) -> Result<WorkflowConfig> {
        let content = fs::read_to_string(path)?;
        
        // 解析 YAML front matter
        let (front_matter, body) = Self::parse_front_matter(&content)?;
        
        // 合并配置
        let mut config: WorkflowConfig = serde_yaml::from_str(&front_matter)
            .map_err(|e| NuClawError::Config {
                message: format!("Failed to parse WORKFLOW.md: {}", e),
            })?;
        
        // 如果没有 prompt_template，使用 body
        if config.prompt_template.is_empty() && !body.is_empty() {
            config.prompt_template = body;
        }
        
        // 解析环境变量
        Self::resolve_env_vars(&mut config)?;
        
        Ok(config)
    }
    
    fn parse_front_matter(content: &str) -> Result<(String, String)> {
        if content.starts_with("---") {
            // 找到结束标记
            // ...
        }
        Ok((String::new(), content.to_string()))
    }
    
    fn resolve_env_vars(config: &mut WorkflowConfig) -> Result<()> {
        // 递归解析 $VAR 和 ${VAR}
    }
}
```

### 4.3 Hook 执行器 (hooks.rs)

```rust
pub struct HookRunner;

impl HookRunner {
    pub async fn run_hook(hook: &str, workspace: &Path) -> Result<()> {
        if hook.trim().is_empty() {
            return Ok(());
        }
        
        let output = Command::new("bash")
            .args(["-c", hook])
            .current_dir(workspace)
            .output()
            .map_err(|e| NuClawError::Config {
                message: format!("Failed to run hook: {}", e),
            })?;
            
        if !output.status.success() {
            return Err(NuClawError::Config {
                message: format!("Hook failed: {}", String::from_utf8_lossy(&output.stderr)),
            });
        }
        
        Ok(())
    }
}
```

---

## 5. 测试计划

### 5.1 单元测试

| 模块 | 测试用例 | 覆盖率目标 |
|------|----------|------------|
| `config` | 默认值、序列化、反序列化 | 100% |
| `loader` | 正常解析、错误处理、env 解析 | 100% |
| `hooks` | 执行、超时、错误处理 | 100% |

### 5.2 测试用例

```rust
// config tests
#[test]
fn test_default_agent_settings() { ... }

#[test]
fn test_channel_config_serialization() { ... }

#[test]
fn test_container_settings_defaults() { ... }

// loader tests
#[test]
fn test_load_workflow_with_front_matter() { ... }

#[test]
fn test_load_workflow_without_front_matter() { ... }

#[test]
fn test_resolve_env_vars() { ... }

#[test]
fn test_missing_required_fields() { ... }

// hooks tests
#[test]
fn test_run_simple_hook() { ... }

#[test]
fn test_run_empty_hook() { ... }

#[test]
fn test_hook_timeout() { ... }

#[test]
fn test_hook_failure() { ... }
```

---

## 6. 向后兼容性

### 现有配置迁移
- 环境变量仍然有效
- WORKFLOW.md 配置覆盖环境变量
- 默认值保持不变

### API 兼容性
- 现有 `config.rs` 函数保持不变
- 新增 `workflow::load()` 函数
- `run_container()` 接受可选的 `WorkflowConfig`

---

## 7. 实施计划

| 任务 | 文件 | 行数 | 测试 |
|------|------|------|------|
| 类型定义 | `workflow/config.rs` | 150 | 20 |
| 配置加载 | `workflow/loader.rs` | 200 | 30 |
| Hook 执行 | `workflow/hooks.rs` | 100 | 15 |
| 模块入口 | `workflow/mod.rs` | 30 | - |
| 集成测试 | `tests/workflow.rs` | 200 | 25 |
| **总计** | | **680** | **90** |

---

## 8. 风险控制

| 风险 | 缓解措施 |
|------|----------|
| 破坏现有功能 | 增量添加，不修改现有 API |
| 配置错误 | 默认值 + 错误日志 |
| Hook 超时 | 添加超时控制 |
| 性能问题 | 懒加载 + 缓存 |
