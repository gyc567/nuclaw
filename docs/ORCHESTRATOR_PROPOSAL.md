# NuClaw 任务编排系统实现提案 (Phase 2)

## 设计原则

1. **KISS** - 最小复杂度，最少概念
2. **高内聚低耦合** - 单一职责，独立模块
3. **可测试** - 100% 测试覆盖率

---

## 用户故事

### US1: 任务队列
> 作为用户，我希望任务进入队列而不是立即执行，这样可以实现并发控制

### US2: 并发限制
> 作为用户，我希望限制同时运行的任务数量，避免资源耗尽

### US3: 重试机制
> 作为用户，我希望失败的任务自动重试，提高可靠性

### US4: 任务状态追踪
> 作为用户，我希望知道任务的当前状态（pending/running/completed/failed）

### US5: 指标收集
> 作为用户，我希望收集任务执行指标用于监控

---

## 文件结构

```
src/orchestrator/
├── mod.rs       # 模块导出
├── task.rs      # 任务定义
├── queue.rs     # 任务队列
├── executor.rs  # 执行器
└── metrics.rs  # 指标收集
```

---

## 核心类型

```rust
// task.rs
pub struct Task {
    pub id: TaskId,
    pub source: TaskSource,
    pub input: ContainerInput,
    pub priority: i32,
    pub created_at: Instant,
    pub status: TaskStatus,
}

pub enum TaskSource {
    Telegram,
    WhatsApp,
    Scheduled,
}

pub enum TaskStatus {
    Pending,
    Running,
    Completed,
    Failed,
}
```

---

## 实现计划

| 周 | 任务 | 测试 |
|----|------|------|
| 1 | Task + TaskStatus 类型 | 10 |
| 2 | TaskQueue 优先级队列 | 15 |
| 3 | Executor 并发控制 | 15 |
| 4 | 重试机制 | 10 |
| 5 | Metrics 收集 | 8 |
| 6 | 集成测试 | 12 |
| **总计** | | **70** |
