# OpenSpec 提案 - NuClaw 记忆自动维护机制

## 执行摘要

本提案实现记忆自动维护功能：
- **MEMORY.md 自动归档**: 超过 200 行自动归档到历史
- **旧日志自动清理**: 超过 90 天的日志自动删除

## 架构设计

```
┌─────────────────────────────────────────────────────────────┐
│              MemoryMaintenanceScheduler                      │
│  ┌─────────────────────┐  ┌─────────────────────────────┐ │
│  │  ContentArchiver    │  │   LogCleaner               │ │
│  │  - line count check │  │   - age check (>90 days)    │ │
│  │  - archive to hist │  │   - delete old logs         │ │
│  └─────────────────────┘  └─────────────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
```

## 设计原则

### KISS 原则
- 单一职责：每个维护器只做一件事
- 简单触发：基于文件大小/时间戳
- 最小依赖：无外部库依赖

### 高内聚、低耦合
- **ContentArchiver**: 负责内容归档
- **LogCleaner**: 负责日志清理
- **MaintenanceScheduler**: 统一调度

## 详细设计

### 1. ContentArchiver

```rust
pub struct ContentArchiver {
    threshold_lines: usize,  // 默认 200
    archive_dir: PathBuf,
}

impl ContentArchiver {
    pub fn new(archive_dir: PathBuf) -> Self;
    pub fn should_archive(&self, path: &Path) -> bool;
    pub fn archive(&self, path: &Path) -> Result<()>;
}
```

触发条件：
- 文件是 MEMORY.md
- 行数超过 200 行
- 归档到 `groups/{group}/.history/`

### 2. LogCleaner

```rust
pub struct LogCleaner {
    max_age_days: i64,  // 默认 90
    log_dir: PathBuf,
}

impl LogCleaner {
    pub fn new(log_dir: PathBuf) -> Self;
    pub fn should_delete(&self, path: &Path) -> bool;
    pub fn clean(&self) -> Result<usize>;  // 返回删除数量
}
```

触发条件：
- 文件在 logs 目录
- 修改时间超过 90 天

### 3. MaintenanceScheduler

```rust
pub struct MaintenanceScheduler {
    archiver: ContentArchiver,
    cleaner: LogCleaner,
}

impl MaintenanceScheduler {
    pub fn new(archiver: ContentArchiver, cleaner: LogCleaner) -> Self;
    pub fn run_maintenance(&self, group_folder: &str) -> Result<MaintenanceReport>;
}
```

## 数据库 Schema

```sql
-- 维护记录表
CREATE TABLE IF NOT EXISTS maintenance_logs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    maintenance_type TEXT NOT NULL,
    group_folder TEXT,
    target_path TEXT,
    action TEXT NOT NULL,
    lines_archived INTEGER DEFAULT 0,
    logs_cleaned INTEGER DEFAULT 0,
    executed_at TEXT NOT NULL
);
```

## API 设计

```rust
pub struct MaintenanceReport {
    pub archives: Vec<ArchiveRecord>,
    pub cleaned: usize,
    pub errors: Vec<String>,
    pub executed_at: String,
}

pub struct ArchiveRecord {
    pub original_path: String,
    pub archive_path: String,
    pub line_count: usize,
}
```

## 实施计划

| 阶段 | 任务 | 描述 |
|------|------|------|
| 1 | ContentArchiver | 实现 MEMORY.md 归档 |
| 2 | LogCleaner | 实现日志清理 |
| 3 | MaintenanceScheduler | 统一调度器 |
| 4 | 集成 | 集成到主程序 |
| 5 | 测试 | 100% 覆盖率 |

## 验收标准

- [ ] MEMORY.md 超过 200 行自动归档到 .history/
- [ ] logs 目录超过 90 天的文件自动删除
- [ ] 维护记录写入数据库
- [ ] 100% 测试覆盖
- [ ] 零回归

---

**提案版本**: v1.0  
**生成日期**: 2026-02-21  
**状态**: ⏳ 待实现
