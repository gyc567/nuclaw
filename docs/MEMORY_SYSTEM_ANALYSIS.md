# NuClaw 记忆体系分析报告

**分析日期**: 2026-03-14  
**项目**: NuClaw v1.0.0

---

## 1. 记忆体系概述

NuClaw 采用**三层记忆体系** + **观察者模式**的混合架构:

```
┌─────────────────────────────────────────────────────────────┐
│                    Memory Architecture                      │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  ┌─────────────┐    ┌─────────────┐    ┌─────────────┐   │
│  │  Hot Tier  │ →  │  Warm Tier │ →  │  Cold Tier │   │
│  │  (In-Mem) │    │  (SQLite)  │    │ (Archive)  │   │
│  └─────────────┘    └─────────────┘    └─────────────┘   │
│                                                             │
│  ┌─────────────────────────────────────────────────┐       │
│  │           Observer Pattern                     │       │
│  │  NoopObserver / LogObserver / MultiObserver│       │
│  └─────────────────────────────────────────────────┘       │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

---

## 2. 记忆层详解

### 2.1 Hot Tier (热记忆)

| 属性 | 值 |
|------|-----|
| 存储位置 | 内存 (In-Memory) |
| 保留时间 | 0-7 天 |
| 最大条目 | 1000 条 |
| 访问方式 | 直接内存访问 |
| 性能 | 最快 |

```rust
// 内存中存储,直接读写
struct HotMemory {
    entries: HashMap<String, TieredMemoryEntry>,
}
```

### 2.2 Warm Tier (温记忆)

| 属性 | 值 |
|------|-----|
| 存储位置 | SQLite (warm_memories.db) |
| 保留时间 | 7-30 天 |
| 存储方式 | 持久化 |
| 性能 | 中等 |

```rust
// SQLite 表结构
CREATE TABLE warm_memories (
    id TEXT PRIMARY KEY,
    key TEXT NOT NULL,
    content TEXT NOT NULL,
    tier TEXT,
    priority TEXT,
    timestamp TEXT,
    accessed_at TEXT,
    access_count INTEGER,
    session_id TEXT,
    tags TEXT
);
```

### 2.3 Cold Tier (冷记忆)

| 属性 | 值 |
|------|-----|
| 存储位置 | SQLite (cold_memories.db) |
| 保留时间 | 30+ 天 |
| 存储方式 | 归档 |
| 性能 | 最慢 |

---

## 3. 优先级系统

| 优先级 | 说明 | 对应类别 |
|--------|------|---------|
| Critical | 核心规则,始终保留 | Core |
| High | 重要任务 | Daily |
| Normal | 常规内容 | Conversation |
| Low | 通用信息 | Custom |

---

## 4. 观察者模式

### 4.1 Observer Trait

```rust
pub trait Observer: Send + Sync {
    fn name(&self) -> &str;
    async fn observe(&self, entry: MemoryEntry);
    async fn flush(&self) -> Result<()>;
}
```

### 4.2 实现类型

| Observer | 用途 |
|----------|------|
| NoopObserver | 空操作,禁用观察 |
| LogObserver | 记录内存操作日志 |
| MultiObserver | 多观察者组合 |

---

## 5. 数据结构

### 5.1 TieredMemoryEntry

```rust
pub struct TieredMemoryEntry {
    pub id: String,
    pub key: String,
    pub content: String,
    pub tier: MemoryTier,
    pub priority: Priority,
    pub timestamp: String,
    pub accessed_at: String,
    pub access_count: u32,
    pub session_id: Option<String>,
    pub tags: Vec<String>,
}
```

### 5.2 MigrationPolicy

```rust
pub struct MigrationPolicy {
    pub hot_to_warm_days: i64,   // 7
    pub warm_to_cold_days: i64,   // 30
    pub max_hot_entries: usize,    // 1000
}
```

---

## 6. 核心功能

### 6.1 记忆操作

| 操作 | 描述 |
|------|------|
| store | 存储新记忆 |
| retrieve | 检索记忆 |
| search | 搜索记忆 |
| delete | 删除记忆 |
| count | 统计数量 |

### 6.2 迁移策略

| 触发条件 | 动作 |
|----------|------|
| 7 天未访问 | Hot → Warm |
| 30 天未访问 | Warm → Cold |
| 访问频繁 | Cold → Warm |
| 1000+ 条目 | LRU 淘汰 |

---

## 7. 代码统计

| 模块 | 行数 | 占比 |
|------|------|------|
| memory.rs | 1,794 | 86% |
| observer.rs | 288 | 14% |
| **总计** | **2,082** | 100% |

---

## 8. 设计模式

| 模式 | 使用位置 |
|------|----------|
| Observer | 事件通知 |
| Tiered Storage | 分层存储 |
| Repository | 数据库访问 |
| Migration | 自动迁移 |

---

## 9. 数据库表

| 表名 | 层 | 用途 |
|------|-----|------|
| memories | Hot | 内存缓存 |
| warm_memories | Warm | 7-30 天记忆 |
| cold_memories | Cold | 归档记忆 |

---

## 10. 性能考虑

### 10.1 优点

- [x] 热数据内存访问,极快
- [x] SQLite 持久化,可恢复
- [x] 自动迁移,无需人工干预
- [x] 观察者模式,解耦日志/监控

### 10.2 可优化点

- [ ] 大数据集考虑压缩
- [ ] 异步批量迁移
- [ ] 缓存热点数据

---

## 11. 总结

NuClaw 的记忆体系是一个完整的**分层缓存系统**:

1. **三层架构**: Hot → Warm → Cold 自动迁移
2. **优先级**: Critical/High/Normal/Low 分类
3. **观察者**: 解耦的事件处理
4. **持久化**: SQLite + 内存双存储

这是一个**生产级**的记忆系统设计,满足长期对话上下文管理需求。

---

*报告生成时间: 2026-03-14*
