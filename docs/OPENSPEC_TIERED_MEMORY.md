# OpenSpec 提案 - NuClaw 三层记忆体系

## 执行摘要

本提案实现三层记忆分级系统，参考人脑记忆模型：
- **P0 (热记忆)**: 工作记忆 - 核心规则、当前任务、最近7天对话
- **P1 (温记忆)**: 经验记忆 - 已完成任务、踩过的坑、学到的教训
- **P2 (冷记忆)**: 归档记忆 - 30天前的历史

## 架构设计

```
┌─────────────────────────────────────────────────────────────┐
│                    TieredMemory (Facade)                     │
│  ┌─────────────────────────────────────────────────────┐    │
│  │              MemoryTier (Trait)                      │    │
│  └─────────────────────────────────────────────────────┘    │
│                            │                                 │
│     ┌──────────────────────┼──────────────────────┐         │
│     ▼                      ▼                      ▼         │
│ ┌──────────┐        ┌──────────┐         ┌──────────┐       │
│ │ HotMemory│        │WarmMemory│         │ColdMemory│       │
│ │   (P0)   │◄──────►│   (P1)   │◄──────►│   (P2)   │       │
│ │  In-Mem  │        │  SQLite  │        │ Archive  │       │
│ └──────────┘        └──────────┘         └──────────┘       │
└─────────────────────────────────────────────────────────────┘
```

## 核心设计原则

### 1. KISS 原则
- 保持三层架构简单直观
- 每层只做一件事
- 最小化接口复杂度

### 2. 高内聚、低耦合
- **MemoryTier** trait 定义统一接口
- 每层独立实现，互不依赖
- 使用 Facade 模式统一入口

### 3. 设计模式
- **Facade**: TieredMemory 统一入口
- **Strategy**: 不同存储策略
- **Template Method**: 统一生命周期管理

## 详细设计

### 1. 数据结构

```rust
/// 记忆条目 - 统一的数据结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub id: String,
    pub key: String,           // 唯一标识
    pub content: String,       // 内容
    pub tier: MemoryTier,      // 当前层级
    pub priority: Priority,    // 优先级
    pub timestamp: String,     // 创建时间
    pub accessed_at: String,   // 最后访问时间
    pub access_count: u32,     // 访问次数
    pub session_id: Option<String>,
    pub tags: Vec<String>,     // 标签
}

/// 记忆层级
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MemoryTier {
    Hot,    // P0: 0-7天
    Warm,   // P1: 7-30天
    Cold,   // P2: 30天+
}

/// 优先级
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Priority {
    Critical,  // 核心规则
    High,      // 重要任务
    Normal,    // 普通内容
    Low,       // 一般信息
}
```

### 2. Trait 定义

```rust
#[async_trait]
pub trait MemoryTier: Send + Sync {
    /// 获取记忆
    async fn get(&self, key: &str) -> Result<Option<MemoryEntry>>;
    
    /// 存储记忆
    async fn store(&self, entry: MemoryEntry) -> Result<()>;
    
    /// 搜索记忆
    async fn search(&self, query: &str, limit: usize) -> Result<Vec<MemoryEntry>>;
    
    /// 列出记忆
    async fn list(&self, filter: Option<&MemoryFilter>) -> Result<Vec<MemoryEntry>>;
    
    /// 删除记忆
    async fn delete(&self, key: &str) -> Result<bool>;
    
    /// 记忆数量
    async fn count(&self) -> Result<usize>;
    
    /// 健康检查
    async fn health_check(&self) -> bool;
    
    /// 获取层级
    fn tier(&self) -> MemoryTier;
}
```

### 3. 各层实现

#### P0 热记忆 (HotMemory)
- **存储**: 内存 (RwLock<HashMap>)
- **容量**: 默认 1000 条
- **淘汰**: LRU 策略
- **数据**: 最近7天、优先级 Critical/High

#### P1 温记忆 (WarmMemory)
- **存储**: SQLite (使用现有 Database)
- **容量**: 无限制
- **数据**: 7-30天
- **用途**: 完成任务、经验教训

#### P2 冷记忆 (ColdMemory)
- **存储**: SQLite 归档表
- **容量**: 无限制
- **数据**: 30天+
- **用途**: 历史归档、可检索

### 4. 记忆迁移

```rust
/// 自动迁移策略
pub struct MigrationPolicy {
    pub hot_to_warm_days: i64,      // 7天
    pub warm_to_cold_days: i64,     // 30天
    pub max_hot_entries: usize,     // 1000
}

/// 迁移触发条件
enum MigrationTrigger {
    TimeBased,    // 时间到了
    CapacityFull, // 容量满了
    AccessBased,  // 访问频率低
}
```

## 数据库 Schema

```sql
-- P1 温记忆表
CREATE TABLE IF NOT EXISTS warm_memories (
    id TEXT PRIMARY KEY,
    key TEXT UNIQUE NOT NULL,
    content TEXT NOT NULL,
    priority TEXT NOT NULL,
    timestamp TEXT NOT NULL,
    accessed_at TEXT NOT NULL,
    access_count INTEGER DEFAULT 1,
    session_id TEXT,
    tags TEXT
);

-- P2 冷记忆表
CREATE TABLE IF NOT EXISTS cold_memories (
    id TEXT PRIMARY KEY,
    key TEXT NOT NULL,
    content TEXT NOT NULL,
    priority TEXT NOT NULL,
    timestamp TEXT NOT NULL,
    archived_at TEXT NOT NULL,
    session_id TEXT,
    tags TEXT
);

-- 索引
CREATE INDEX IF NOT EXISTS idx_warm_priority ON warm_memories(priority);
CREATE INDEX IF NOT EXISTS idx_warm_timestamp ON warm_memories(timestamp);
CREATE INDEX IF NOT EXISTS idx_cold_timestamp ON cold_memories(timestamp);
```

## API 设计

```rust
/// 统一入口
pub struct TieredMemory {
    hot: Arc<HotMemory>,
    warm: Arc<WarmMemory>,
    cold: Arc<ColdMemory>,
    policy: MigrationPolicy,
}

impl TieredMemory {
    /// 存储记忆 - 自动选择层级
    pub async fn remember(&self, key: &str, content: &str, priority: Priority) -> Result<()>;
    
    /// 回忆 - 从热到冷逐层搜索
    pub async fn recall(&self, key: &str) -> Result<Option<MemoryEntry>>;
    
    /// 搜索 - 跨层搜索
    pub async fn search(&self, query: &str, limit: usize) -> Result<Vec<MemoryEntry>>;
    
    /// 列出 - 按条件
    pub async fn list(&self, filter: MemoryFilter) -> Result<Vec<MemoryEntry>>;
    
    /// 遗忘 - 跨层删除
    pub async fn forget(&self, key: &str) -> Result<bool>;
    
    /// 每日维护 - 触发迁移
    pub async fn maintain(&self) -> Result<MaintenanceReport>;
}
```

## 向后兼容

```rust
/// 兼容旧 API
impl Memory for TieredMemory {
    async fn store(&self, key: &str, content: &str, category: MemoryCategory) -> Result<()> {
        let priority = match category {
            MemoryCategory::Core => Priority::Critical,
            MemoryCategory::Daily => Priority::High,
            _ => Priority::Normal,
        };
        self.remember(key, content, priority).await
    }
    // ... 其他方法
}
```

## 实施计划

| 阶段 | 任务 | 描述 |
|------|------|------|
| 1 | 创建数据类型 | Priority, MemoryTier, MemoryEntry |
| 2 | 实现 HotMemory | 内存缓存 + LRU |
| 3 | 实现 WarmMemory | SQLite 表 + 索引 |
| 4 | 实现 ColdMemory | 归档表 |
| 5 | 实现 TieredMemory | Facade + 迁移逻辑 |
| 6 | 维护任务 | 每日迁移触发器 |
| 7 | 测试 | 100% 覆盖率 |
| 8 | 集成 | 替换现有 memory 使用 |

## 测试覆盖率目标

| 模块 | 目标 |
|------|------|
| memory/tier | 100% |
| memory/hot | 100% |
| memory/warm | 100% |
| memory/cold | 100% |

## 验收标准

- [ ] P0 热记忆: 内存缓存，7天数据，LRU 淘汰
- [ ] P1 温记忆: SQLite，7-30天数据
- [ ] P2 冷记忆: 归档表，30天+数据
- [ ] 自动迁移: 每日维护时触发
- [ ] 向后兼容: 现有 Memory trait 兼容
- [ ] 测试覆盖: 100%
- [ ] 零回归: 现有功能不受影响

---

**提案版本**: v1.0  
**生成日期**: 2026-02-20  
**状态**: ⏳ 待实现
