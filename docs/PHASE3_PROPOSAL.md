# Phase 3 优化方案

## 当前现状分析

### 已完成 (Phase 1-2)
- ✅ WORKFLOW 配置系统 (workflow 模块)
- ✅ 任务编排系统 (orchestrator 模块)

### 待评估 (Phase 3 候选)

| 候选功能 | 复杂度 | 收益 | 依赖 | 优先级 |
|---------|--------|------|------|--------|
| Per-Session 工作区隔离 | 中 | 高 | container_runner | P1 |
| WORKFLOW.md 热重载 | 低 | 中 | workflow 模块 | P1 |
| Web Dashboard | 高 | 中 | 需要新依赖 | P2 |

---

## 方案评估

### 1. Per-Session 工作区隔离 (推荐 P1)

**当前问题**:
- 多个会话共享同一 group 文件夹
- 会话状态可能互相污染

**改进方案**:
```
当前: groups/{group}/           (共享)
改进: workspaces/{session_id}/   (隔离)
```

**实现方式**:
```rust
// 在 container_runner.rs 中增强
pub struct Workspace {
    pub id: SessionId,
    pub path: PathBuf,
    pub created_at: DateTime<Utc>,
    pub hooks: HookSettings,
}

impl Workspace {
    pub fn create(session_id: &str, config: &WorkflowConfig) -> Result<Self>;
    pub fn cleanup(&self) -> Result<()>;
}
```

**优点**:
- 简洁: 只增强现有 container_runner
- 安全: 完全隔离
- 可测试: 独立模块

### 2. WORKFLOW.md 热重载 (推荐 P1)

**当前问题**:
- 修改 WORKFLOW.md 需要重启服务

**改进方案**:
```rust
use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};

pub struct ConfigWatcher {
    workflow_path: PathBuf,
    last_config: Arc<Mutex<Option<WorkflowConfig>>>,
}

impl ConfigWatcher {
    pub fn watch(&self) -> Result<()>;
    pub fn get_config(&self) -> Option<WorkflowConfig>;
}
```

**优点**:
- 低复杂度: 使用 notify crate
- 实用: 无需重启服务

### 3. Web Dashboard (暂不推荐)

**问题**:
- 需要额外依赖 (axum + 模板引擎)
- 超出 KISS 原则
- 当前日志已足够

---

## 推荐实施计划

### Phase 3.1: Per-Session 隔离

| 任务 | 文件 | 测试 |
|------|------|------|
| Workspace 类型 | workspace.rs (新) | 10 |
| 创建/清理 | workspace.rs | 15 |
| 集成 container_runner | container_runner.rs | 5 |
| **小计** | 200行 | 30 |

### Phase 3.2: 热重载

| 任务 | 文件 | 测试 |
|------|------|------|
| ConfigWatcher | watcher.rs (新) | 12 |
| 集成 loader | loader.rs | 5 |
| **小计** | 150行 | 17 |

---

## 优化后的提案

### 推荐 Phase 3 范围

```
Phase 3 = Per-Session 工作区隔离 + WORKFLOW 热重载
- 不做 Web Dashboard (超出 KISS)
- 保持现有模块独立
- 总代码量: ~350行
- 总测试: ~47个
```

### 理由

1. **KISS**: 两个功能都是增量改进，不需要大重构
2. **高内聚**: workspace.rs 独立于其他模块
3. **低耦合**: watcher 只通知变化，不影响业务逻辑
4. **可测试**: 纯函数为主

---

## 决策

请确认是否按此方案实施:

- [ ] Phase 3.1: Per-Session 工作区隔离
- [ ] Phase 3.2: WORKFLOW.md 热重载
- [ ] 都做
- [ ] 其他建议
