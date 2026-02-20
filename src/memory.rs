use async_trait::async_trait;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Mutex;

use crate::error::{NuClawError, Result};

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
        let timestamp = chrono::Utc::now().to_rfc3339();

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
        self.conn.lock().unwrap().execute("SELECT 1", []).is_ok()
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

#[cfg(test)]
mod sqlite_tests {
    use super::*;
    use std::fs;

    fn create_temp_memory() -> SqliteMemory {
        let path = "/tmp/test_memory.db";
        let _ = fs::remove_file(path);
        SqliteMemory::new(path).unwrap()
    }

    fn cleanup() {
        let _ = fs::remove_file("/tmp/test_memory.db");
    }

    #[tokio::test]
    #[ignore]
    async fn test_sqlite_memory_store_and_get() {
        let memory = create_temp_memory();
        memory.store("test_key", "test content", MemoryCategory::Core).await.unwrap();
        let result = memory.get("test_key").await.unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap().key, "test_key");
        cleanup();
    }

    #[tokio::test]
    #[ignore]
    async fn test_sqlite_memory_recall() {
        let memory = create_temp_memory();
        memory.store("key1", "hello world", MemoryCategory::Core).await.unwrap();
        memory.store("key2", "goodbye world", MemoryCategory::Conversation).await.unwrap();
        let results = memory.recall("hello", 10).await.unwrap();
        assert!(!results.is_empty());
        cleanup();
    }

    #[tokio::test]
    #[ignore]
    async fn test_sqlite_memory_list() {
        let memory = create_temp_memory();
        memory.store("key1", "content1", MemoryCategory::Core).await.unwrap();
        memory.store("key2", "content2", MemoryCategory::Daily).await.unwrap();
        let all = memory.list(None).await.unwrap();
        assert_eq!(all.len(), 2);
        let core = memory.list(Some(&MemoryCategory::Core)).await.unwrap();
        assert_eq!(core.len(), 1);
        cleanup();
    }

    #[tokio::test]
    #[ignore]
    async fn test_sqlite_memory_forget() {
        let memory = create_temp_memory();
        memory.store("key1", "content", MemoryCategory::Core).await.unwrap();
        assert!(memory.get("key1").await.unwrap().is_some());
        memory.forget("key1").await.unwrap();
        assert!(memory.get("key1").await.unwrap().is_none());
        cleanup();
    }

    #[tokio::test]
    #[ignore]
    async fn test_sqlite_memory_count() {
        let memory = create_temp_memory();
        assert_eq!(memory.count().await.unwrap(), 0);
        memory.store("key1", "content", MemoryCategory::Core).await.unwrap();
        assert_eq!(memory.count().await.unwrap(), 1);
        cleanup();
    }

    #[tokio::test]
    #[ignore]
    async fn test_sqlite_memory_health_check() {
        let memory = create_temp_memory();
        assert!(memory.health_check().await);
        cleanup();
    }

    #[tokio::test]
    #[ignore]
    async fn test_sqlite_memory_replace() {
        let memory = create_temp_memory();
        memory.store("key1", "original", MemoryCategory::Core).await.unwrap();
        memory.store("key1", "updated", MemoryCategory::Core).await.unwrap();
        let result = memory.get("key1").await.unwrap().unwrap();
        assert_eq!(result.content, "updated");
        cleanup();
    }

    #[test]
    fn test_memory_category_from_str() {
        assert_eq!(MemoryCategory::from_str("core"), MemoryCategory::Core);
        assert_eq!(MemoryCategory::from_str("daily"), MemoryCategory::Daily);
        assert_eq!(MemoryCategory::from_str("conversation"), MemoryCategory::Conversation);
        assert_eq!(MemoryCategory::from_str("custom"), MemoryCategory::Custom("custom".to_string()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
