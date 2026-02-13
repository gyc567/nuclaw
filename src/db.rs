//! Database module for NuClaw
//!
//! Provides SQLite database operations with connection pooling.
//! Uses r2d2 for connection management and rusqlite for SQLite access.

use crate::config::store_dir;
use crate::error::NuClawError;
use r2d2::{Pool, PooledConnection};
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::Connection;
use std::path::PathBuf;

/// Database configuration
#[derive(Debug, Clone)]
pub struct DatabaseConfig {
    /// Maximum pool size
    pub pool_size: u32,
    /// Connection timeout in milliseconds
    pub connection_timeout_ms: u64,
    /// Database file path
    pub db_path: PathBuf,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            pool_size: std::env::var("DB_POOL_SIZE")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(10),
            connection_timeout_ms: std::env::var("DB_CONNECTION_TIMEOUT_MS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(30000),
            db_path: store_dir().join("nuclaw.db"),
        }
    }
}

/// Database wrapper with connection pool
#[derive(Clone, Debug)]
pub struct Database {
    pool: Pool<SqliteConnectionManager>,
    config: DatabaseConfig,
}

impl Database {
    /// Create a new Database with default config
    pub fn new() -> Result<Self, NuClawError> {
        Self::with_config(DatabaseConfig::default())
    }

    /// Create a new Database with custom config
    pub fn with_config(config: DatabaseConfig) -> Result<Self, NuClawError> {
        let manager = SqliteConnectionManager::file(&config.db_path).with_init(|conn| {
            conn.pragma_update(None, "foreign_keys", "ON")?;
            conn.pragma_update(None, "journal_mode", "WAL")?;
            conn.pragma_update(None, "synchronous", "NORMAL")?;
            Ok(())
        });

        let pool = Pool::builder()
            .max_size(config.pool_size)
            .connection_timeout(std::time::Duration::from_millis(
                config.connection_timeout_ms,
            ))
            .build(manager)
            .map_err(|e| NuClawError::Database {
                message: format!("Failed to create connection pool: {}", e),
            })?;

        let conn = pool.get().map_err(|e| NuClawError::Database {
            message: format!("Failed to get connection: {}", e),
        })?;
        initialize_schema(&conn)?;

        Ok(Database { pool, config })
    }

    /// Get a connection from the pool
    pub fn get_connection(&self) -> Result<PooledConnection<SqliteConnectionManager>, NuClawError> {
        self.pool.get().map_err(|e| NuClawError::Database {
            message: format!("Failed to get connection from pool: {}", e),
        })
    }

    /// Get the database configuration
    pub fn config(&self) -> &DatabaseConfig {
        &self.config
    }

    /// Get pool status
    pub fn pool_status(&self) -> PoolStatus {
        let state = self.pool.state();
        PoolStatus {
            connections_idle: state.idle_connections,
            connections_active: state.connections - state.idle_connections,
            max_size: self.config.pool_size,
        }
    }
}

/// Pool status information
#[derive(Debug, Clone)]
pub struct PoolStatus {
    pub connections_idle: u32,
    pub connections_active: u32,
    pub max_size: u32,
}

/// Initialize database schema
fn initialize_schema(conn: &Connection) -> Result<(), NuClawError> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS chats (
            jid TEXT PRIMARY KEY,
            name TEXT,
            last_message_time TEXT
        )",
        [],
    )
    .map_err(|e| NuClawError::Database {
        message: format!("Failed to create chats table: {}", e),
    })?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS messages (
            id TEXT,
            chat_jid TEXT,
            sender TEXT,
            sender_name TEXT,
            content TEXT,
            timestamp TEXT,
            is_from_me INTEGER DEFAULT 0,
            PRIMARY KEY (id, chat_jid)
        )",
        [],
    )
    .map_err(|e| NuClawError::Database {
        message: format!("Failed to create messages table: {}", e),
    })?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS scheduled_tasks (
            id TEXT PRIMARY KEY,
            group_folder TEXT NOT NULL,
            chat_jid TEXT NOT NULL,
            prompt TEXT NOT NULL,
            schedule_type TEXT NOT NULL,
            schedule_value TEXT NOT NULL,
            next_run TEXT,
            last_run TEXT,
            last_result TEXT,
            status TEXT DEFAULT 'active',
            created_at TEXT NOT NULL,
            context_mode TEXT DEFAULT 'isolated'
        )",
        [],
    )
    .map_err(|e| NuClawError::Database {
        message: format!("Failed to create scheduled_tasks table: {}", e),
    })?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS task_run_logs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            task_id TEXT NOT NULL,
            run_at TEXT NOT NULL,
            duration_ms INTEGER NOT NULL,
            status TEXT NOT NULL,
            result TEXT,
            error TEXT
        )",
        [],
    )
    .map_err(|e| NuClawError::Database {
        message: format!("Failed to create task_run_logs table: {}", e),
    })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn test_db_path() -> PathBuf {
        store_dir().join("test_nuclaw.db")
    }

    fn cleanup_test_db(path: &PathBuf) {
        let _ = fs::remove_file(path);
        let _ = fs::remove_file(path.with_extension("db-wal"));
        let _ = fs::remove_file(path.with_extension("db-shm"));
    }

    #[test]
    fn test_database_new() {
        let db_path = test_db_path();
        cleanup_test_db(&db_path);

        let config = DatabaseConfig {
            db_path: db_path.clone(),
            pool_size: 5,
            connection_timeout_ms: 5000,
        };

        let result = Database::with_config(config);
        assert!(
            result.is_ok(),
            "Database should be created successfully: {:?}",
            result.err()
        );

        // Explicit cleanup
        drop(result);
        cleanup_test_db(&db_path);
    }

    #[test]
    fn test_get_connection() {
        let db_path = test_db_path();
        cleanup_test_db(&db_path);

        let config = DatabaseConfig {
            db_path: db_path.clone(),
            pool_size: 3,
            connection_timeout_ms: 5000,
        };

        let db = Database::with_config(config).unwrap();
        let conn = db.get_connection();
        assert!(conn.is_ok(), "Should get connection from pool");
        cleanup_test_db(&db_path);
    }

    #[test]
    fn test_concurrent_connections() {
        let db_path = test_db_path();
        cleanup_test_db(&db_path);

        let config = DatabaseConfig {
            db_path: db_path.clone(),
            pool_size: 5,
            connection_timeout_ms: 10000,
        };

        let db = Database::with_config(config).unwrap();

        let mut handles = Vec::new();
        for _ in 0..5 {
            let db_clone = db.clone();
            handles.push(std::thread::spawn(move || db_clone.get_connection()));
        }

        let results: Vec<Result<PooledConnection<SqliteConnectionManager>, NuClawError>> =
            handles.into_iter().map(|h| h.join().unwrap()).collect();

        assert_eq!(results.len(), 5);
        assert!(
            results.into_iter().all(|r| r.is_ok()),
            "All connections should succeed"
        );
        cleanup_test_db(&db_path);
    }

    #[test]
    fn test_pool_status() {
        let db_path = test_db_path();
        cleanup_test_db(&db_path);

        let config = DatabaseConfig {
            db_path: db_path.clone(),
            pool_size: 10,
            connection_timeout_ms: 5000,
        };

        let db = Database::with_config(config).unwrap();

        let status = db.pool_status();
        assert!(
            status.connections_idle >= 1,
            "Should have at least one idle connection"
        );
        assert_eq!(status.max_size, 10);

        cleanup_test_db(&db_path);
    }

    #[test]
    fn test_database_config_defaults() {
        let _lock = ENV_LOCK.lock().unwrap();

        std::env::remove_var("DB_POOL_SIZE");
        std::env::remove_var("DB_CONNECTION_TIMEOUT_MS");

        let config = DatabaseConfig::default();
        assert_eq!(config.pool_size, 10);
        assert_eq!(config.connection_timeout_ms, 30000);
    }

    #[test]
    fn test_database_config_from_env() {
        let _lock = ENV_LOCK.lock().unwrap();

        std::env::remove_var("DB_POOL_SIZE");
        std::env::remove_var("DB_CONNECTION_TIMEOUT_MS");

        std::env::set_var("DB_POOL_SIZE", "20");
        std::env::set_var("DB_CONNECTION_TIMEOUT_MS", "60000");

        let config = DatabaseConfig::default();
        assert_eq!(config.pool_size, 20);
        assert_eq!(config.connection_timeout_ms, 60000);

        std::env::remove_var("DB_POOL_SIZE");
        std::env::remove_var("DB_CONNECTION_TIMEOUT_MS");
    }

    #[test]
    fn test_schema_initialization() {
        let db_path = test_db_path();
        cleanup_test_db(&db_path);

        let config = DatabaseConfig {
            db_path: db_path.clone(),
            pool_size: 3,
            connection_timeout_ms: 5000,
        };

        let db = Database::with_config(config).unwrap();
        let conn = db.get_connection().unwrap();

        let tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table'")
            .unwrap()
            .query_map([], |row| row.get::<_, String>(0))
            .unwrap()
            .collect::<Result<Vec<String>, _>>()
            .unwrap();

        assert!(tables.contains(&"chats".to_string()));
        assert!(tables.contains(&"messages".to_string()));
        assert!(tables.contains(&"scheduled_tasks".to_string()));
        assert!(tables.contains(&"task_run_logs".to_string()));

        cleanup_test_db(&db_path);
    }

    #[test]
    fn test_clone_database() {
        let db_path = test_db_path();
        cleanup_test_db(&db_path);

        let config = DatabaseConfig {
            db_path: db_path.clone(),
            pool_size: 3,
            connection_timeout_ms: 5000,
        };

        let db1 = Database::with_config(config.clone()).unwrap();
        let db2 = db1.clone();

        assert!(db1.get_connection().is_ok());
        assert!(db2.get_connection().is_ok());

        cleanup_test_db(&db_path);
    }
}
