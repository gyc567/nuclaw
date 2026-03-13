# P0: 容器预热与池化 - 技术规范

## 1. 背景与目标

### 问题
当前每次消息处理都需要执行 `docker run` 启动新容器，存在冷启动延迟 (3-10秒)。

### 目标
- 预热容器：程序启动时预先启动容器
- 容器复用：执行完成后不销毁容器，而是放回池中重用
- 减少延迟：将 3-10s 冷启动降至 <1s

## 2. 设计方案

### 2.1 核心组件

```
┌─────────────────────────────────────────────────────────────┐
│                    ContainerPool                             │
├─────────────────────────────────────────────────────────────┤
│  - pool: Vec<Container>          // 可用容器列表             │
│  - max_size: usize              // 最大池大小                │
│  - min_size: usize              // 最小预热数量              │
│  - lock: Arc<Mutex<()>>        // 池操作锁                  │
└─────────────────────────────────────────────────────────────┘
```

### 2.2 工作流程

```
程序启动
    │
    ▼
ContainerPool::new() → 预启动 min_size 个容器
    │
    ▼
消息处理请求
    │
    ├── 从池中获取可用容器 ────▶ 执行任务 ────▶ 归还容器
    │                                    │
    │ (无容器可用)                        ▼
    │    │                         放回池中
    │    ▼                         (而非销毁)
    └── 等待或创建新容器
```

### 2.3 接口设计

```rust
// 池化容器句柄
pub struct PooledContainer {
    id: String,
    group_folder: String,
    // 归还方法
}

impl PooledContainer {
    pub async fn run(self, input: ContainerInput) -> Result<ContainerOutput>;
}

// 容器池
pub struct ContainerPool {
    // 内部状态
}

impl ContainerPool {
    pub fn new(max_size: usize, min_size: usize) -> Result<Self>;
    pub async fn acquire(&self, group_folder: &str) -> Result<PooledContainer>;
    pub fn release(&self, container: PooledContainer);
    pub async fn warmup(&self) -> Result<()>;
}
```

## 3. 实现细节

### 3.1 容器复用机制

- 容器按 group_folder 隔离
- 执行完成后不执行 `docker rm`，而是保留容器
- 通过 `docker exec` 复用已存在的容器

### 3.2 预热策略

- 程序启动时预启动 `min_size` 个容器
- 默认 min_size = 2, max_size = 5

### 3.3 状态管理

```rust
enum ContainerState {
    Idle,           // 可用
    Busy,           // 使用中
    Warming,        // 启动中
    Error,          // 错误，需要重建
}
```

## 4. 回退机制

若容器池出现错误（如容器被意外删除），自动回退到原来的 `docker run` 模式。

## 5. 测试计划

| 测试用例 | 描述 |
|----------|------|
| test_pool_creation | 测试池创建 |
| test_pool_warmup | 测试预热 |
| test_pool_acquire_release | 测试获取/归还 |
| test_pool_max_size | 测试最大容量限制 |
| test_pool_fallback | 测试错误回退 |
| test_container_reuse | 测试容器复用 |

## 6. 兼容性

- 默认禁用池化功能（保持向后兼容）
- 通过环境变量 `CONTAINER_POOL_ENABLED=true` 启用
- 不影响现有的 container runner 逻辑
