use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::path::Path;
use std::sync::{Arc, Mutex, RwLock};

use crate::error::{NuClawError, Result};

/// Memory tier levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MemoryTier {
    /// P0: Hot memory - 0-7 days, in-memory
    Hot,
    /// P1: Warm memory - 7-30 days, SQLite
    Warm,
    /// P2: Cold memory - 30+ days, archive
    Cold,
}

impl std::fmt::Display for MemoryTier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Hot => write!(f, "hot"),
            Self::Warm => write!(f, "warm"),
            Self::Cold => write!(f, "cold"),
        }
    }
}

impl std::fmt::Display for Priority {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Critical => write!(f, "critical"),
            Self::High => write!(f, "high"),
            Self::Normal => write!(f, "normal"),
            Self::Low => write!(f, "low"),
        }
    }
}

/// Priority levels for memory entries
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Priority {
    /// Critical - Core rules, always keep
    Critical,
    /// High - Important tasks
    High,
    /// Normal - Regular content
    Normal,
    /// Low - General information
    Low,
}

impl Priority {
    /// Convert from legacy MemoryCategory
    pub fn from_category(category: &MemoryCategory) -> Self {
        match category {
            MemoryCategory::Core => Self::Critical,
            MemoryCategory::Daily => Self::High,
            MemoryCategory::Conversation => Self::Normal,
            MemoryCategory::Custom(_) => Self::Normal,
        }
    }

    /// Parse from string
    pub fn from_str(s: &str) -> Self {
        match s {
            "critical" => Self::Critical,
            "high" => Self::High,
            "normal" => Self::Normal,
            "low" => Self::Low,
            _ => Self::Normal,
        }
    }
}

/// Memory entry with tier support
#[derive(Debug, Clone, Serialize, Deserialize)]
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

impl TieredMemoryEntry {
    /// Create new entry
    pub fn new(key: String, content: String, priority: Priority) -> Self {
        let now = Utc::now().to_rfc3339();
        Self {
            id: format!("mem_{}", uuid::Uuid::new_v4()),
            key,
            content,
            tier: MemoryTier::Hot,
            priority,
            timestamp: now.clone(),
            accessed_at: now,
            access_count: 1,
            session_id: None,
            tags: Vec::new(),
        }
    }

    /// Check if entry should be promoted to warm (7 days)
    pub fn should_promote_to_warm(&self) -> bool {
        if let Ok(created) = DateTime::parse_from_rfc3339(&self.timestamp) {
            let age = Utc::now().signed_duration_since(created.with_timezone(&Utc));
            age > Duration::days(7)
        } else {
            false
        }
    }

    /// Check if entry should be archived to cold (30 days)
    pub fn should_archive_to_cold(&self) -> bool {
        if let Ok(created) = DateTime::parse_from_rfc3339(&self.timestamp) {
            let age = Utc::now().signed_duration_since(created.with_timezone(&Utc));
            age > Duration::days(30)
        } else {
            false
        }
    }

    /// Convert to legacy MemoryEntry
    pub fn to_legacy(&self) -> MemoryEntry {
        MemoryEntry {
            id: self.id.clone(),
            key: self.key.clone(),
            content: self.content.clone(),
            category: match self.priority {
                Priority::Critical => MemoryCategory::Core,
                Priority::High => MemoryCategory::Daily,
                _ => MemoryCategory::Conversation,
            },
            timestamp: self.timestamp.clone(),
            session_id: self.session_id.clone(),
            score: Some(self.access_count as f64),
        }
    }
}

/// Migration policy configuration
#[derive(Debug, Clone)]
pub struct MigrationPolicy {
    /// Days before promoting to warm
    pub hot_to_warm_days: i64,
    /// Days before archiving to cold
    pub warm_to_cold_days: i64,
    /// Maximum hot memory entries
    pub max_hot_entries: usize,
}

impl Default for MigrationPolicy {
    fn default() -> Self {
        Self {
            hot_to_warm_days: 7,
            warm_to_cold_days: 30,
            max_hot_entries: 1000,
        }
    }
}

/// Maintenance report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaintenanceReport {
    pub hot_to_warm_migrated: usize,
    pub warm_to_cold_migrated: usize,
    pub cold_to_warm_promoted: usize,
    pub hot_evicted: usize,
    pub total_hot: usize,
    pub total_warm: usize,
    pub total_cold: usize,
}

/// Memory entry (legacy)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub id: String,
    pub key: String,
    pub content: String,
    pub category: MemoryCategory,
    pub timestamp: String,
    pub session_id: Option<String>,
    pub score: Option<f64>,
}

/// Memory category (legacy)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MemoryCategory {
    Core,
    Daily,
    Conversation,
    Custom(String),
}

impl std::fmt::Display for MemoryCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Core => write!(f, "core"),
            Self::Daily => write!(f, "daily"),
            Self::Conversation => write!(f, "conversation"),
            Self::Custom(name) => write!(f, "{}", name),
        }
    }
}

impl MemoryCategory {
    pub fn from_str(s: &str) -> Self {
        match s {
            "core" => MemoryCategory::Core,
            "daily" => MemoryCategory::Daily,
            "conversation" => MemoryCategory::Conversation,
            other => MemoryCategory::Custom(other.to_string()),
        }
    }
}

pub struct HotMemory {
    cache: RwLock<HashMap<String, TieredMemoryEntry>>,
    access_order: RwLock<VecDeque<String>>,
    max_entries: usize,
}

impl HotMemory {
    pub fn new(max_entries: usize) -> Self {
        Self {
            cache: RwLock::new(HashMap::new()),
            access_order: RwLock::new(VecDeque::new()),
            max_entries,
        }
    }

    pub fn get(&self, key: &str) -> Option<TieredMemoryEntry> {
        let mut cache = self.cache.write().ok()?;
        let entry = cache.get(key)?.clone();
        
        if let Ok(mut order) = self.access_order.write() {
            order.retain(|k| k != key);
            order.push_back(key.to_string());
        }
        
        Some(entry)
    }

    pub fn store(&self, entry: TieredMemoryEntry) {
        let key = entry.key.clone();
        let mut cache = self.cache.write().unwrap();
        let mut order = self.access_order.write().unwrap();
        
        while cache.len() >= self.max_entries {
            if let Some(oldest) = order.pop_front() {
                cache.remove(&oldest);
            } else {
                break;
            }
        }
        
        order.retain(|k| k != &key);
        cache.insert(key.clone(), entry);
        order.push_back(key);
    }

    pub fn remove(&self, key: &str) -> bool {
        let mut cache = self.cache.write().unwrap();
        let mut order = self.access_order.write().unwrap();
        
        order.retain(|k| k != key);
        cache.remove(key).is_some()
    }

    pub fn get_all(&self) -> Vec<TieredMemoryEntry> {
        let cache = self.cache.read().unwrap();
        cache.values().cloned().collect()
    }

    pub fn get_entries_for_promotion(&self) -> Vec<TieredMemoryEntry> {
        let cache = self.cache.read().unwrap();
        cache
            .values()
            .filter(|e| e.should_promote_to_warm() && e.priority != Priority::Critical)
            .cloned()
            .collect()
    }

    pub fn count(&self) -> usize {
        self.cache.read().unwrap().len()
    }

    pub fn search(&self, query: &str, limit: usize) -> Vec<TieredMemoryEntry> {
        let cache = self.cache.read().unwrap();
        let query_lower = query.to_lowercase();
        
        cache
            .values()
            .filter(|e| e.content.to_lowercase().contains(&query_lower))
            .take(limit)
            .cloned()
            .collect()
    }

    pub fn health_check(&self) -> bool {
        self.cache.read().is_ok() && self.access_order.read().is_ok()
    }
}

pub struct WarmMemory {
    conn: RwLock<Connection>,
}

impl WarmMemory {
    pub fn new(path: impl AsRef<Path>) -> Result<Self> {
        let conn = Connection::open(path)
            .map_err(|e| NuClawError::Database { message: e.to_string() })?;
        
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS warm_memories (
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
            CREATE INDEX IF NOT EXISTS idx_warm_key ON warm_memories(key);
            CREATE INDEX IF NOT EXISTS idx_warm_priority ON warm_memories(priority);
            CREATE INDEX IF NOT EXISTS idx_warm_timestamp ON warm_memories(timestamp);"
        ).map_err(|e| NuClawError::Database { message: e.to_string() })?;

        Ok(Self { conn: RwLock::new(conn) })
    }

    pub fn get(&self, key: &str) -> Result<Option<TieredMemoryEntry>> {
        let conn = self.conn.read().unwrap();
        
        let mut stmt = conn.prepare(
            "SELECT id, key, content, priority, timestamp, accessed_at, access_count, session_id, tags 
             FROM warm_memories WHERE key = ?"
        ).map_err(|e| NuClawError::Database { message: e.to_string() })?;

        let result = stmt.query_row([key], |row| {
            let tags_str: String = row.get(8)?;
            let tags: Vec<String> = serde_json::from_str(&tags_str).unwrap_or_default();
            
            Ok(TieredMemoryEntry {
                id: row.get(0)?,
                key: row.get(1)?,
                content: row.get(2)?,
                tier: MemoryTier::Warm,
                priority: Priority::from_str(&row.get::<_, String>(3)?),
                timestamp: row.get(4)?,
                accessed_at: row.get(5)?,
                access_count: row.get(6)?,
                session_id: row.get(7)?,
                tags,
            })
        });

        match result {
            Ok(entry) => Ok(Some(entry)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(NuClawError::Database { message: e.to_string() }.into()),
        }
    }

    pub fn store(&self, entry: &TieredMemoryEntry) -> Result<()> {
        let conn = self.conn.read().unwrap();
        let tags_json = serde_json::to_string(&entry.tags).unwrap_or_default();

        conn.execute(
            "INSERT OR REPLACE INTO warm_memories 
             (id, key, content, priority, timestamp, accessed_at, access_count, session_id, tags) 
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            rusqlite::params![
                entry.id,
                entry.key,
                entry.content,
                entry.priority.to_string(),
                entry.timestamp,
                entry.accessed_at,
                entry.access_count,
                entry.session_id,
                tags_json,
            ],
        ).map_err(|e| NuClawError::Database { message: e.to_string() })?;

        Ok(())
    }

    /// Delete entry
    pub fn delete(&self, key: &str) -> Result<bool> {
        let conn = self.conn.read().unwrap();
        let affected = conn.execute("DELETE FROM warm_memories WHERE key = ?", [key])
            .map_err(|e| NuClawError::Database { message: e.to_string() })?;
        Ok(affected > 0)
    }

    /// Get all entries
    pub fn get_all(&self) -> Result<Vec<TieredMemoryEntry>> {
        let conn = self.conn.read().unwrap();
        
        let mut stmt = conn.prepare(
            "SELECT id, key, content, priority, timestamp, accessed_at, access_count, session_id, tags 
             FROM warm_memories"
        ).map_err(|e| NuClawError::Database { message: e.to_string() })?;

        let rows = stmt.query_map([], |row| {
            let tags_str: String = row.get(8)?;
            let tags: Vec<String> = serde_json::from_str(&tags_str).unwrap_or_default();
            
            Ok(TieredMemoryEntry {
                id: row.get(0)?,
                key: row.get(1)?,
                content: row.get(2)?,
                tier: MemoryTier::Warm,
                priority: Priority::from_str(&row.get::<_, String>(3)?),
                timestamp: row.get(4)?,
                accessed_at: row.get(5)?,
                access_count: row.get(6)?,
                session_id: row.get(7)?,
                tags,
            })
        }).map_err(|e| NuClawError::Database { message: e.to_string() })?;

        let mut results = Vec::new();
        for entry in rows {
            if let Ok(e) = entry {
                results.push(e);
            }
        }
        Ok(results)
    }

    /// Get entries for archiving
    pub fn get_entries_for_archival(&self) -> Result<Vec<TieredMemoryEntry>> {
        let conn = self.conn.read().unwrap();
        
        let mut stmt = conn.prepare(
            "SELECT id, key, content, priority, timestamp, accessed_at, access_count, session_id, tags 
             FROM warm_memories WHERE timestamp < datetime('now', '-30 days')"
        ).map_err(|e| NuClawError::Database { message: e.to_string() })?;

        let rows = stmt.query_map([], |row| {
            let tags_str: String = row.get(8)?;
            let tags: Vec<String> = serde_json::from_str(&tags_str).unwrap_or_default();
            
            Ok(TieredMemoryEntry {
                id: row.get(0)?,
                key: row.get(1)?,
                content: row.get(2)?,
                tier: MemoryTier::Warm,
                priority: Priority::from_str(&row.get::<_, String>(3)?),
                timestamp: row.get(4)?,
                accessed_at: row.get(5)?,
                access_count: row.get(6)?,
                session_id: row.get(7)?,
                tags,
            })
        }).map_err(|e| NuClawError::Database { message: e.to_string() })?;

        let mut results = Vec::new();
        for entry in rows {
            if let Ok(e) = entry {
                results.push(e);
            }
        }
        Ok(results)
    }

    /// Search
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<TieredMemoryEntry>> {
        let conn = self.conn.read().unwrap();
        let pattern = format!("%{}%", query);

        let mut stmt = conn.prepare(
            "SELECT id, key, content, priority, timestamp, accessed_at, access_count, session_id, tags 
             FROM warm_memories WHERE content LIKE ? LIMIT ?"
        ).map_err(|e| NuClawError::Database { message: e.to_string() })?;

        let rows = stmt.query_map(rusqlite::params![pattern, limit as i64], |row| {
            let tags_str: String = row.get(8)?;
            let tags: Vec<String> = serde_json::from_str(&tags_str).unwrap_or_default();
            
            Ok(TieredMemoryEntry {
                id: row.get(0)?,
                key: row.get(1)?,
                content: row.get(2)?,
                tier: MemoryTier::Warm,
                priority: Priority::from_str(&row.get::<_, String>(3)?),
                timestamp: row.get(4)?,
                accessed_at: row.get(5)?,
                access_count: row.get(6)?,
                session_id: row.get(7)?,
                tags,
            })
        }).map_err(|e| NuClawError::Database { message: e.to_string() })?;

        let mut results = Vec::new();
        for entry in rows {
            if let Ok(e) = entry {
                results.push(e);
            }
        }
        Ok(results)
    }

    /// Count
    pub fn count(&self) -> Result<usize> {
        let conn = self.conn.read().unwrap();
        let count: i64 = conn.query_row("SELECT COUNT(*) FROM warm_memories", [], |row| row.get(0))
            .map_err(|e| NuClawError::Database { message: e.to_string() })?;
        Ok(count as usize)
    }

    pub fn health_check(&self) -> bool {
        if let Ok(conn) = self.conn.read() {
            conn.query_row("SELECT 1", [], |_| Ok(())).is_ok()
        } else {
            false
        }
    }
}

// ============================================================================
// P2: Cold Memory - Archive storage (30+ days)
// ============================================================================

/// Cold memory - P2 tier, archive storage
pub struct ColdMemory {
    conn: RwLock<Connection>,
}

impl ColdMemory {
    /// Create new cold memory
    pub fn new(path: impl AsRef<Path>) -> Result<Self> {
        let conn = Connection::open(path)
            .map_err(|e| NuClawError::Database { message: e.to_string() })?;
        
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS cold_memories (
                id TEXT PRIMARY KEY,
                key TEXT NOT NULL,
                content TEXT NOT NULL,
                priority TEXT NOT NULL,
                timestamp TEXT NOT NULL,
                archived_at TEXT NOT NULL,
                session_id TEXT,
                tags TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_cold_key ON cold_memories(key);
            CREATE INDEX IF NOT EXISTS idx_cold_timestamp ON cold_memories(timestamp);"
        ).map_err(|e| NuClawError::Database { message: e.to_string() })?;

        Ok(Self { conn: RwLock::new(conn) })
    }

    /// Get entry
    pub fn get(&self, key: &str) -> Result<Option<TieredMemoryEntry>> {
        let conn = self.conn.read().unwrap();
        
        let mut stmt = conn.prepare(
            "SELECT id, key, content, priority, timestamp, archived_at, session_id, tags 
             FROM cold_memories WHERE key = ?"
        ).map_err(|e| NuClawError::Database { message: e.to_string() })?;

        let result = stmt.query_row([key], |row| {
            let tags_str: String = row.get(7)?;
            let tags: Vec<String> = serde_json::from_str(&tags_str).unwrap_or_default();
            
            Ok(TieredMemoryEntry {
                id: row.get(0)?,
                key: row.get(1)?,
                content: row.get(2)?,
                tier: MemoryTier::Cold,
                priority: Priority::from_str(&row.get::<_, String>(3)?),
                timestamp: row.get(4)?,
                accessed_at: row.get(5)?,
                access_count: 0,
                session_id: row.get(6)?,
                tags,
            })
        });

        match result {
            Ok(entry) => Ok(Some(entry)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(NuClawError::Database { message: e.to_string() }.into()),
        }
    }

    /// Archive entry
    pub fn archive(&self, entry: &TieredMemoryEntry) -> Result<()> {
        let conn = self.conn.read().unwrap();
        let tags_json = serde_json::to_string(&entry.tags).unwrap_or_default();
        let archived_at = Utc::now().to_rfc3339();

        conn.execute(
            "INSERT OR REPLACE INTO cold_memories 
             (id, key, content, priority, timestamp, archived_at, session_id, tags) 
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            rusqlite::params![
                entry.id,
                entry.key,
                entry.content,
                entry.priority.to_string(),
                entry.timestamp,
                archived_at,
                entry.session_id,
                tags_json,
            ],
        ).map_err(|e| NuClawError::Database { message: e.to_string() })?;

        Ok(())
    }

    /// Delete entry
    pub fn delete(&self, key: &str) -> Result<bool> {
        let conn = self.conn.read().unwrap();
        let affected = conn.execute("DELETE FROM cold_memories WHERE key = ?", [key])
            .map_err(|e| NuClawError::Database { message: e.to_string() })?;
        Ok(affected > 0)
    }

    /// Search
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<TieredMemoryEntry>> {
        let conn = self.conn.read().unwrap();
        let pattern = format!("%{}%", query);

        let mut stmt = conn.prepare(
            "SELECT id, key, content, priority, timestamp, archived_at, session_id, tags 
             FROM cold_memories WHERE content LIKE ? LIMIT ?"
        ).map_err(|e| NuClawError::Database { message: e.to_string() })?;

        let rows = stmt.query_map(rusqlite::params![pattern, limit as i64], |row| {
            let tags_str: String = row.get(7)?;
            let tags: Vec<String> = serde_json::from_str(&tags_str).unwrap_or_default();
            
            Ok(TieredMemoryEntry {
                id: row.get(0)?,
                key: row.get(1)?,
                content: row.get(2)?,
                tier: MemoryTier::Cold,
                priority: Priority::from_str(&row.get::<_, String>(3)?),
                timestamp: row.get(4)?,
                accessed_at: row.get(5)?,
                access_count: 0,
                session_id: row.get(6)?,
                tags,
            })
        }).map_err(|e| NuClawError::Database { message: e.to_string() })?;

        let mut results = Vec::new();
        for entry in rows {
            if let Ok(e) = entry {
                results.push(e);
            }
        }
        Ok(results)
    }

    /// Count
    pub fn count(&self) -> Result<usize> {
        let conn = self.conn.read().unwrap();
        let count: i64 = conn.query_row("SELECT COUNT(*) FROM cold_memories", [], |row| row.get(0))
            .map_err(|e| NuClawError::Database { message: e.to_string() })?;
        Ok(count as usize)
    }

    pub fn health_check(&self) -> bool {
        if let Ok(conn) = self.conn.read() {
            conn.query_row("SELECT 1", [], |_| Ok(())).is_ok()
        } else {
            false
        }
    }
}

// ============================================================================
// Tiered Memory - Unified Facade
// ============================================================================

/// Unified tiered memory facade
pub struct TieredMemory {
    hot: Arc<HotMemory>,
    warm: Arc<WarmMemory>,
    cold: Arc<ColdMemory>,
    policy: MigrationPolicy,
}

impl TieredMemory {
    /// Create new tiered memory
    pub fn new(db_path: impl AsRef<Path>, policy: MigrationPolicy) -> Result<Self> {
        let hot = Arc::new(HotMemory::new(policy.max_hot_entries));
        let warm = Arc::new(WarmMemory::new(db_path.as_ref().join("warm_memories.db"))?);
        let cold = Arc::new(ColdMemory::new(db_path.as_ref().join("cold_memories.db"))?);

        Ok(Self { hot, warm, cold, policy })
    }

    /// Remember - store a memory
    pub async fn remember(&self, key: &str, content: &str, priority: Priority) -> Result<()> {
        // Check if exists in any tier
        if self.hot.get(key).is_some() {
            // Update in hot
            let mut entry = self.hot.get(key).unwrap();
            entry.content = content.to_string();
            entry.accessed_at = Utc::now().to_rfc3339();
            entry.access_count += 1;
            self.hot.store(entry);
            return Ok(());
        }

        // Create new entry
        let entry = TieredMemoryEntry::new(key.to_string(), content.to_string(), priority);
        self.hot.store(entry);
        Ok(())
    }

    /// Recall - retrieve a memory
    pub async fn recall(&self, key: &str) -> Result<Option<TieredMemoryEntry>> {
        // Try hot first
        if let Some(entry) = self.hot.get(key) {
            return Ok(Some(entry));
        }

        // Try warm
        if let Some(entry) = self.warm.get(key)? {
            // Promote to hot
            let mut promoted = entry.clone();
            promoted.tier = MemoryTier::Hot;
            self.hot.store(promoted.clone());
            return Ok(Some(promoted));
        }

        // Try cold
        if let Some(entry) = self.cold.get(key)? {
            // Promote to hot
            let mut promoted = entry;
            promoted.tier = MemoryTier::Hot;
            self.hot.store(promoted.clone());
            return Ok(Some(promoted));
        }

        Ok(None)
    }

    /// Search across all tiers
    pub async fn search(&self, query: &str, limit: usize) -> Result<Vec<TieredMemoryEntry>> {
        let mut results = Vec::new();
        
        // Search hot
        results.extend(self.hot.search(query, limit));
        
        // Search warm
        if results.len() < limit {
            results.extend(self.warm.search(query, limit - results.len())?);
        }
        
        // Search cold
        if results.len() < limit {
            results.extend(self.cold.search(query, limit - results.len())?);
        }

        Ok(results)
    }

    /// Forget - delete from all tiers
    pub async fn forget(&self, key: &str) -> Result<bool> {
        let mut deleted = false;
        
        if self.hot.remove(key) {
            deleted = true;
        }
        if self.warm.delete(key)? {
            deleted = true;
        }
        if self.cold.delete(key)? {
            deleted = true;
        }

        Ok(deleted)
    }

    /// Count total memories
    pub async fn count(&self) -> Result<usize> {
        Ok(self.hot.count() + self.warm.count()? + self.cold.count()?)
    }

    /// Maintenance - run migration
    pub async fn maintain(&self) -> Result<MaintenanceReport> {
        let mut report = MaintenanceReport {
            hot_to_warm_migrated: 0,
            warm_to_cold_migrated: 0,
            cold_to_warm_promoted: 0,
            hot_evicted: 0,
            total_hot: self.hot.count(),
            total_warm: self.warm.count()?,
            total_cold: self.cold.count()?,
        };

        // Migrate hot to warm
        let to_promote = self.hot.get_entries_for_promotion();
        for entry in &to_promote {
            let mut promoted = entry.clone();
            promoted.tier = MemoryTier::Warm;
            self.warm.store(&promoted)?;
            self.hot.remove(&entry.key);
            report.hot_to_warm_migrated += 1;
        }

        // Archive warm to cold
        let to_archive = self.warm.get_entries_for_archival()?;
        for entry in &to_archive {
            self.cold.archive(entry)?;
            self.warm.delete(&entry.key)?;
            report.warm_to_cold_migrated += 1;
        }

        // Update counts
        report.total_hot = self.hot.count();
        report.total_warm = self.warm.count()?;
        report.total_cold = self.cold.count()?;

        Ok(report)
    }

    /// Health check
    pub async fn health_check(&self) -> bool {
        self.hot.health_check() && self.warm.health_check() && self.cold.health_check()
    }

    /// Get hot memory (for testing)
    #[cfg(test)]
    pub fn hot(&self) -> &HotMemory {
        &self.hot
    }

    /// Get warm memory (for testing)
    #[cfg(test)]
    pub fn warm(&self) -> &WarmMemory {
        &self.warm
    }

    /// Get cold memory (for testing)
    #[cfg(test)]
    pub fn cold(&self) -> &ColdMemory {
        &self.cold
    }
}

// ============================================================================
// Legacy Memory Trait - Backward Compatibility
// ============================================================================

/// Legacy memory trait
#[async_trait]
pub trait Memory: Send + Sync {
    fn name(&self) -> &str;
    async fn store(&self, key: &str, content: &str, category: MemoryCategory) -> Result<()>;
    async fn recall(&self, query: &str, limit: usize) -> Result<Vec<MemoryEntry>>;
    async fn get(&self, key: &str) -> Result<Option<MemoryEntry>>;
    async fn list(&self, category: Option<&MemoryCategory>) -> Result<Vec<MemoryEntry>>;
    async fn forget(&self, key: &str) -> Result<bool>;
    async fn count(&self) -> Result<usize>;
    async fn health_check(&self) -> bool;
}

/// No-op memory implementation
pub struct NoopMemory;

#[async_trait]
impl Memory for NoopMemory {
    fn name(&self) -> &str {
        "noop"
    }

    async fn store(&self, _key: &str, _content: &str, _category: MemoryCategory) -> Result<()> {
        Ok(())
    }

    async fn recall(&self, _query: &str, _limit: usize) -> Result<Vec<MemoryEntry>> {
        Ok(Vec::new())
    }

    async fn get(&self, _key: &str) -> Result<Option<MemoryEntry>> {
        Ok(None)
    }

    async fn list(&self, _category: Option<&MemoryCategory>) -> Result<Vec<MemoryEntry>> {
        Ok(Vec::new())
    }

    async fn forget(&self, _key: &str) -> Result<bool> {
        Ok(false)
    }

    async fn count(&self) -> Result<usize> {
        Ok(0)
    }

    async fn health_check(&self) -> bool {
        true
    }
}

/// Legacy SQLite memory (kept for backward compatibility)
pub struct SqliteMemory {
    conn: Mutex<Connection>,
}

impl SqliteMemory {
    pub fn new(path: impl AsRef<Path>) -> Result<Self> {
        let conn = Connection::open(path)
            .map_err(|e| NuClawError::Database { message: e.to_string() })?;
        
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS memories (
                id TEXT PRIMARY KEY,
                key TEXT UNIQUE NOT NULL,
                content TEXT NOT NULL,
                category TEXT NOT NULL,
                timestamp TEXT NOT NULL,
                session_id TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_memories_category ON memories(category);
            CREATE INDEX IF NOT EXISTS idx_memories_key ON memories(key);"
        ).map_err(|e| NuClawError::Database { message: e.to_string() })?;

        Ok(Self { conn: Mutex::new(conn) })
    }

    fn generate_id(&self) -> String {
        format!("mem_{}", uuid::Uuid::new_v4())
    }
}

#[async_trait]
impl Memory for SqliteMemory {
    fn name(&self) -> &str {
        "sqlite"
    }

    async fn store(&self, key: &str, content: &str, category: MemoryCategory) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let id = self.generate_id();
        let timestamp = Utc::now().to_rfc3339();

        conn.execute(
            "INSERT OR REPLACE INTO memories (id, key, content, category, timestamp) VALUES (?, ?, ?, ?, ?)",
            [&id, key, content, &category.to_string(), &timestamp],
        ).map_err(|e| NuClawError::Database { message: e.to_string() })?;

        Ok(())
    }

    async fn recall(&self, query: &str, limit: usize) -> Result<Vec<MemoryEntry>> {
        let conn = self.conn.lock().unwrap();
        
        let mut stmt = conn.prepare(
            "SELECT id, key, content, category, timestamp, session_id FROM memories WHERE content LIKE ? LIMIT ?"
        ).map_err(|e| NuClawError::Database { message: e.to_string() })?;

        let search_pattern = format!("%{}%", query);
        let entries = stmt.query_map([&search_pattern, &limit.to_string()], |row| {
            Ok(MemoryEntry {
                id: row.get(0)?,
                key: row.get(1)?,
                content: row.get(2)?,
                category: MemoryCategory::from_str(&row.get::<_, String>(3)?),
                timestamp: row.get(4)?,
                session_id: row.get(5)?,
                score: None,
            })
        }).map_err(|e| NuClawError::Database { message: e.to_string() })?;

        let mut results = Vec::new();
        for entry in entries {
            if let Ok(e) = entry {
                results.push(e);
            }
        }

        Ok(results)
    }

    async fn get(&self, key: &str) -> Result<Option<MemoryEntry>> {
        let conn = self.conn.lock().unwrap();
        
        let mut stmt = conn.prepare(
            "SELECT id, key, content, category, timestamp, session_id FROM memories WHERE key = ?"
        ).map_err(|e| NuClawError::Database { message: e.to_string() })?;

        let result = stmt.query_row([key], |row| {
            Ok(MemoryEntry {
                id: row.get(0)?,
                key: row.get(1)?,
                content: row.get(2)?,
                category: MemoryCategory::from_str(&row.get::<_, String>(3)?),
                timestamp: row.get(4)?,
                session_id: row.get(5)?,
                score: None,
            })
        });

        match result {
            Ok(entry) => Ok(Some(entry)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(NuClawError::Database { message: e.to_string() }.into()),
        }
    }

    async fn list(&self, category: Option<&MemoryCategory>) -> Result<Vec<MemoryEntry>> {
        let conn = self.conn.lock().unwrap();
        
        let mut results = Vec::new();
        
        if let Some(cat) = category {
            let mut stmt = conn.prepare(
                "SELECT id, key, content, category, timestamp, session_id FROM memories WHERE category = ?"
            ).map_err(|e| NuClawError::Database { message: e.to_string() })?;

            let rows = stmt.query_map([cat.to_string()], |row| {
                Ok(MemoryEntry {
                    id: row.get(0)?,
                    key: row.get(1)?,
                    content: row.get(2)?,
                    category: MemoryCategory::from_str(&row.get::<_, String>(3)?),
                    timestamp: row.get(4)?,
                    session_id: row.get(5)?,
                    score: None,
                })
            }).map_err(|e| NuClawError::Database { message: e.to_string() })?;

            for entry in rows {
                if let Ok(e) = entry {
                    results.push(e);
                }
            }
        } else {
            let mut stmt = conn.prepare(
                "SELECT id, key, content, category, timestamp, session_id FROM memories"
            ).map_err(|e| NuClawError::Database { message: e.to_string() })?;

            let rows = stmt.query_map([], |row| {
                Ok(MemoryEntry {
                    id: row.get(0)?,
                    key: row.get(1)?,
                    content: row.get(2)?,
                    category: MemoryCategory::from_str(&row.get::<_, String>(3)?),
                    timestamp: row.get(4)?,
                    session_id: row.get(5)?,
                    score: None,
                })
            }).map_err(|e| NuClawError::Database { message: e.to_string() })?;

            for entry in rows {
                if let Ok(e) = entry {
                    results.push(e);
                }
            }
        }

        Ok(results)
    }

    async fn forget(&self, key: &str) -> Result<bool> {
        let conn = self.conn.lock().unwrap();
        
        let affected = conn.execute("DELETE FROM memories WHERE key = ?", [key])
            .map_err(|e| NuClawError::Database { message: e.to_string() })?;

        Ok(affected > 0)
    }

    async fn count(&self) -> Result<usize> {
        let conn = self.conn.lock().unwrap();
        
        let count: i64 = conn.query_row("SELECT COUNT(*) FROM memories", [], |row| row.get(0))
            .map_err(|e| NuClawError::Database { message: e.to_string() })?;

        Ok(count as usize)
    }

    async fn health_check(&self) -> bool {
        if let Ok(conn) = self.conn.lock() {
            conn.query_row("SELECT 1", [], |_| Ok(())).is_ok()
        } else {
            false
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tier_tests {
    use super::*;
    use std::fs;

    fn temp_dir() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("nuclaw_test_{}", uuid::Uuid::new_v4()));
        let _ = fs::create_dir_all(&dir);
        dir
    }

    fn cleanup(path: &std::path::Path) {
        let _ = fs::remove_file(path.join("warm_memories.db"));
        let _ = fs::remove_file(path.join("cold_memories.db"));
        let _ = fs::remove_dir(path);
    }

    // ========== Priority Tests ==========

    #[test]
    fn test_priority_from_category() {
        assert_eq!(Priority::from_category(&MemoryCategory::Core), Priority::Critical);
        assert_eq!(Priority::from_category(&MemoryCategory::Daily), Priority::High);
        assert_eq!(Priority::from_category(&MemoryCategory::Conversation), Priority::Normal);
    }

    #[test]
    fn test_priority_from_str() {
        assert_eq!(Priority::from_str("critical"), Priority::Critical);
        assert_eq!(Priority::from_str("high"), Priority::High);
        assert_eq!(Priority::from_str("normal"), Priority::Normal);
        assert_eq!(Priority::from_str("low"), Priority::Low);
        assert_eq!(Priority::from_str("unknown"), Priority::Normal);
    }

    #[test]
    fn test_priority_display() {
        assert_eq!(Priority::Critical.to_string(), "critical");
        assert_eq!(Priority::High.to_string(), "high");
        assert_eq!(Priority::Normal.to_string(), "normal");
        assert_eq!(Priority::Low.to_string(), "low");
    }

    // ========== MemoryTier Tests ==========

    #[test]
    fn test_memory_tier_display() {
        assert_eq!(MemoryTier::Hot.to_string(), "hot");
        assert_eq!(MemoryTier::Warm.to_string(), "warm");
        assert_eq!(MemoryTier::Cold.to_string(), "cold");
    }

    // ========== TieredMemoryEntry Tests ==========

    #[test]
    fn test_tiered_memory_entry_new() {
        let entry = TieredMemoryEntry::new(
            "test_key".to_string(),
            "test_content".to_string(),
            Priority::High,
        );

        assert!(entry.id.starts_with("mem_"));
        assert_eq!(entry.key, "test_key");
        assert_eq!(entry.content, "test_content");
        assert_eq!(entry.tier, MemoryTier::Hot);
        assert_eq!(entry.priority, Priority::High);
        assert_eq!(entry.access_count, 1);
        assert!(entry.session_id.is_none());
        assert!(entry.tags.is_empty());
    }

    #[test]
    fn test_tiered_memory_entry_to_legacy() {
        let entry = TieredMemoryEntry::new(
            "key".to_string(),
            "content".to_string(),
            Priority::Critical,
        );

        let legacy = entry.to_legacy();
        assert_eq!(legacy.key, "key");
        assert_eq!(legacy.content, "content");
        assert_eq!(legacy.category, MemoryCategory::Core);
    }

    // ========== MigrationPolicy Tests ==========

    #[test]
    fn test_migration_policy_default() {
        let policy = MigrationPolicy::default();
        assert_eq!(policy.hot_to_warm_days, 7);
        assert_eq!(policy.warm_to_cold_days, 30);
        assert_eq!(policy.max_hot_entries, 1000);
    }

    // ========== HotMemory Tests ==========

    #[test]
    fn test_hot_memory_store_and_get() {
        let hot = HotMemory::new(100);
        let entry = TieredMemoryEntry::new("key1".to_string(), "content1".to_string(), Priority::Normal);
        
        hot.store(entry);
        let retrieved = hot.get("key1");
        
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().content, "content1");
    }

    #[test]
    fn test_hot_memory_remove() {
        let hot = HotMemory::new(100);
        let entry = TieredMemoryEntry::new("key1".to_string(), "content1".to_string(), Priority::Normal);
        
        hot.store(entry);
        assert!(hot.remove("key1"));
        assert!(hot.get("key1").is_none());
    }

    #[test]
    fn test_hot_memory_count() {
        let hot = HotMemory::new(100);
        assert_eq!(hot.count(), 0);
        
        hot.store(TieredMemoryEntry::new("k1".to_string(), "c1".to_string(), Priority::Normal));
        hot.store(TieredMemoryEntry::new("k2".to_string(), "c2".to_string(), Priority::Normal));
        
        assert_eq!(hot.count(), 2);
    }

    #[test]
    fn test_hot_memory_search() {
        let hot = HotMemory::new(100);
        hot.store(TieredMemoryEntry::new("k1".to_string(), "hello world".to_string(), Priority::Normal));
        hot.store(TieredMemoryEntry::new("k2".to_string(), "goodbye world".to_string(), Priority::Normal));
        
        let results = hot.search("hello", 10);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].key, "k1");
    }

    #[test]
    fn test_hot_memory_health_check() {
        let hot = HotMemory::new(100);
        assert!(hot.health_check());
    }

    #[test]
    fn test_hot_memory_lru_eviction() {
        let hot = HotMemory::new(2);
        
        hot.store(TieredMemoryEntry::new("k1".to_string(), "c1".to_string(), Priority::Normal));
        hot.store(TieredMemoryEntry::new("k2".to_string(), "c2".to_string(), Priority::Normal));
        hot.store(TieredMemoryEntry::new("k3".to_string(), "c3".to_string(), Priority::Normal));
        
        // k1 should be evicted
        assert!(hot.get("k1").is_none());
        assert!(hot.get("k2").is_some());
        assert!(hot.get("k3").is_some());
    }

    // ========== WarmMemory Tests ==========

    #[test]
    fn test_warm_memory_operations() {
        let dir = temp_dir();
        
        let warm = WarmMemory::new(dir.join("warm.db")).unwrap();
        
        // Store
        let entry = TieredMemoryEntry::new("warm_key".to_string(), "warm_content".to_string(), Priority::High);
        warm.store(&entry).unwrap();
        
        // Get
        let retrieved = warm.get("warm_key").unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().content, "warm_content");
        
        // Count
        assert_eq!(warm.count().unwrap(), 1);
        
        // Delete
        assert!(warm.delete("warm_key").unwrap());
        assert_eq!(warm.count().unwrap(), 0);
        
        cleanup(&dir);
    }

    #[test]
    fn test_warm_memory_search() {
        let dir = temp_dir();
        
        let warm = WarmMemory::new(dir.join("warm.db")).unwrap();
        
        warm.store(&TieredMemoryEntry::new("k1".to_string(), "hello world".to_string(), Priority::Normal)).unwrap();
        warm.store(&TieredMemoryEntry::new("k2".to_string(), "goodbye world".to_string(), Priority::Normal)).unwrap();
        
        let results = warm.search("hello", 10).unwrap();
        assert_eq!(results.len(), 1);
        
        cleanup(&dir);
    }

    #[test]
    fn test_warm_memory_health_check() {
        let dir = temp_dir();
        
        let warm = WarmMemory::new(dir.join("warm.db")).unwrap();
        assert!(warm.health_check());
        
        cleanup(&dir);
    }

    // ========== ColdMemory Tests ==========

    #[test]
    fn test_cold_memory_operations() {
        let dir = temp_dir();
        
        let cold = ColdMemory::new(dir.join("cold.db")).unwrap();
        
        // Archive
        let entry = TieredMemoryEntry::new("cold_key".to_string(), "cold_content".to_string(), Priority::Low);
        cold.archive(&entry).unwrap();
        
        // Get
        let retrieved = cold.get("cold_key").unwrap();
        assert!(retrieved.is_some());
        
        // Count
        assert_eq!(cold.count().unwrap(), 1);
        
        // Delete
        assert!(cold.delete("cold_key").unwrap());
        
        cleanup(&dir);
    }

    #[test]
    fn test_cold_memory_search() {
        let dir = temp_dir();
        
        let cold = ColdMemory::new(dir.join("cold.db")).unwrap();
        
        cold.archive(&TieredMemoryEntry::new("k1".to_string(), "archived content".to_string(), Priority::Low)).unwrap();
        
        let results = cold.search("archived", 10).unwrap();
        assert_eq!(results.len(), 1);
        
        cleanup(&dir);
    }

    // ========== TieredMemory Tests ==========

    #[test]
    fn test_tiered_memory_remember_and_recall() {
        let dir = temp_dir();
        
        let tiered = TieredMemory::new(&dir, MigrationPolicy::default()).unwrap();
        
        // Remember
        tiered.blocking_remember("test_key", "test_content", Priority::High).unwrap();
        
        // Recall
        let result = tiered.blocking_recall("test_key").unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap().content, "test_content");
        
        // Cleanup
        let _ = fs::remove_file(dir.join("warm_memories.db"));
        let _ = fs::remove_file(dir.join("cold_memories.db"));
        let _ = fs::remove_dir(dir);
    }

    #[test]
    fn test_tiered_memory_search() {
        let dir = temp_dir();
        
        let tiered = TieredMemory::new(&dir, MigrationPolicy::default()).unwrap();
        
        tiered.blocking_remember("k1", "hello world", Priority::Normal).unwrap();
        
        let results = tiered.blocking_search("hello", 10).unwrap();
        assert_eq!(results.len(), 1);
        
        // Cleanup
        let _ = fs::remove_file(dir.join("warm_memories.db"));
        let _ = fs::remove_file(dir.join("cold_memories.db"));
        let _ = fs::remove_dir(dir);
    }

    #[test]
    fn test_tiered_memory_forget() {
        let dir = temp_dir();
        
        let tiered = TieredMemory::new(&dir, MigrationPolicy::default()).unwrap();
        
        tiered.blocking_remember("to_delete", "content", Priority::Normal).unwrap();
        assert!(tiered.blocking_forget("to_delete").unwrap());
        
        // Cleanup
        let _ = fs::remove_file(dir.join("warm_memories.db"));
        let _ = fs::remove_file(dir.join("cold_memories.db"));
        let _ = fs::remove_dir(dir);
    }

    #[test]
    fn test_tiered_memory_health_check() {
        let dir = temp_dir();
        
        let tiered = TieredMemory::new(&dir, MigrationPolicy::default()).unwrap();
        
        assert!(tiered.hot().health_check());
        assert!(tiered.warm().health_check());
        assert!(tiered.cold().health_check());
        
        // Cleanup
        let _ = fs::remove_file(dir.join("warm_memories.db"));
        let _ = fs::remove_file(dir.join("cold_memories.db"));
        let _ = fs::remove_dir(dir);
    }
}

// Add blocking wrappers for tests
impl TieredMemory {
    /// Blocking remember
    pub fn blocking_remember(&self, key: &str, content: &str, priority: Priority) -> Result<()> {
        // Check if exists in hot
        if self.hot.get(key).is_some() {
            let mut entry = self.hot.get(key).unwrap();
            entry.content = content.to_string();
            entry.accessed_at = Utc::now().to_rfc3339();
            entry.access_count += 1;
            self.hot.store(entry);
            return Ok(());
        }

        let entry = TieredMemoryEntry::new(key.to_string(), content.to_string(), priority);
        self.hot.store(entry);
        Ok(())
    }

    /// Blocking recall
    pub fn blocking_recall(&self, key: &str) -> Result<Option<TieredMemoryEntry>> {
        if let Some(entry) = self.hot.get(key) {
            return Ok(Some(entry));
        }

        if let Some(entry) = self.warm.get(key)? {
            let mut promoted = entry;
            promoted.tier = MemoryTier::Hot;
            self.hot.store(promoted.clone());
            return Ok(Some(promoted));
        }

        if let Some(entry) = self.cold.get(key)? {
            let mut promoted = entry;
            promoted.tier = MemoryTier::Hot;
            self.hot.store(promoted.clone());
            return Ok(Some(promoted));
        }

        Ok(None)
    }

    /// Blocking search
    pub fn blocking_search(&self, query: &str, limit: usize) -> Result<Vec<TieredMemoryEntry>> {
        let mut results = Vec::new();
        
        results.extend(self.hot.search(query, limit));
        
        if results.len() < limit {
            results.extend(self.warm.search(query, limit - results.len())?);
        }
        
        if results.len() < limit {
            results.extend(self.cold.search(query, limit - results.len())?);
        }

        Ok(results)
    }

    /// Blocking forget
    pub fn blocking_forget(&self, key: &str) -> Result<bool> {
        let mut deleted = false;
        
        if self.hot.remove(key) {
            deleted = true;
        }
        if self.warm.delete(key)? {
            deleted = true;
        }
        if self.cold.delete(key)? {
            deleted = true;
        }

        Ok(deleted)
    }
}

#[cfg(test)]
mod legacy_tests {
    use super::*;
    use std::fs;

    fn temp_path() -> String {
        format!("/tmp/test_memory_{}.db", uuid::Uuid::new_v4())
    }

    fn cleanup(path: &str) {
        let _ = fs::remove_file(path);
    }

    #[test]
    fn test_memory_category_from_str() {
        assert_eq!(MemoryCategory::from_str("core"), MemoryCategory::Core);
        assert_eq!(MemoryCategory::from_str("daily"), MemoryCategory::Daily);
        assert_eq!(MemoryCategory::from_str("conversation"), MemoryCategory::Conversation);
        assert_eq!(MemoryCategory::from_str("custom"), MemoryCategory::Custom("custom".to_string()));
    }

    #[test]
    fn test_memory_entry() {
        let entry = MemoryEntry {
            id: "test_id".to_string(),
            key: "test_key".to_string(),
            content: "test content".to_string(),
            category: MemoryCategory::Core,
            timestamp: "2024-01-01T00:00:00Z".to_string(),
            session_id: Some("session_1".to_string()),
            score: Some(0.9),
        };

        assert_eq!(entry.id, "test_id");
        assert_eq!(entry.key, "test_key");
        assert_eq!(entry.category, MemoryCategory::Core);
        assert!(entry.session_id.is_some());
        assert!(entry.score.is_some());
    }

    #[test]
    fn test_memory_category_display() {
        assert_eq!(MemoryCategory::Core.to_string(), "core");
        assert_eq!(MemoryCategory::Daily.to_string(), "daily");
        assert_eq!(MemoryCategory::Conversation.to_string(), "conversation");
        assert_eq!(MemoryCategory::Custom("custom".to_string()).to_string(), "custom");
    }

    #[test]
    fn test_memory_category_serialization() {
        let core = serde_json::to_string(&MemoryCategory::Core).unwrap();
        assert_eq!(core, "\"core\"");

        let custom = serde_json::to_string(&MemoryCategory::Custom("test".to_string())).unwrap();
        assert!(custom.contains("test"));
    }

    #[test]
    fn test_noop_memory_name() {
        let memory = NoopMemory;
        assert_eq!(memory.name(), "noop");
    }

    #[tokio::test]
    async fn test_noop_memory_operations() {
        let memory = NoopMemory;
        
        assert!(memory.store("key", "content", MemoryCategory::Core).await.is_ok());
        assert!(memory.get("key").await.unwrap().is_none());
        assert!(memory.recall("query", 10).await.unwrap().is_empty());
        assert!(memory.list(None).await.unwrap().is_empty());
        assert!(!memory.forget("key").await.unwrap());
        assert_eq!(memory.count().await.unwrap(), 0);
        assert!(memory.health_check().await);
    }

    #[test]
    fn test_memory_entry_serialization() {
        let entry = MemoryEntry {
            id: "id1".to_string(),
            key: "key1".to_string(),
            content: "content1".to_string(),
            category: MemoryCategory::Conversation,
            timestamp: "2024-01-01T00:00:00Z".to_string(),
            session_id: None,
            score: None,
        };

        let json = serde_json::to_string(&entry).unwrap();
        let parsed: MemoryEntry = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.id, entry.id);
        assert_eq!(parsed.key, entry.key);
        assert_eq!(parsed.category, entry.category);
    }

    #[tokio::test]
    async fn test_sqlite_memory_operations() {
        let path = temp_path();
        
        {
            let memory = SqliteMemory::new(&path).unwrap();
            
            memory.store("test_key", "test content", MemoryCategory::Core).await.unwrap();
            
            let result = memory.get("test_key").await.unwrap();
            assert!(result.is_some());
            assert_eq!(result.unwrap().key, "test_key");
            
            memory.store("key2", "content2", MemoryCategory::Daily).await.unwrap();
            
            let all = memory.list(None).await.unwrap();
            assert_eq!(all.len(), 2);
            
            let core = memory.list(Some(&MemoryCategory::Core)).await.unwrap();
            assert_eq!(core.len(), 1);
            
            memory.forget("key2").await.unwrap();
            assert_eq!(memory.count().await.unwrap(), 1);
            
            assert!(memory.health_check().await);
        }
        
        cleanup(&path);
    }
}
