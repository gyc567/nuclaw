//! Database for NuClaw

use crate::config::store_dir;
use rusqlite::{Connection, Result as SqlResult};
use std::sync::Mutex;

pub struct Database {
    pub connection: Mutex<Connection>,
}

impl Clone for Database {
    fn clone(&self) -> Self {
        let db_path = store_dir().join("nuclaw.db");
        let connection = Connection::open(&db_path)
            .unwrap_or_else(|_| panic!("Failed to open database at {:?}", db_path));
        Database {
            connection: Mutex::new(connection),
        }
    }
}

impl Database {
    pub fn new() -> SqlResult<Self> {
        let db_path = store_dir().join("nuclaw.db");
        let connection = Connection::open(&db_path)?;

        // Create tables
        connection.execute(
            "CREATE TABLE IF NOT EXISTS chats (
                jid TEXT PRIMARY KEY,
                name TEXT,
                last_message_time TEXT
            )",
            [],
        )?;

        connection.execute(
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
        )?;

        connection.execute(
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
        )?;

        connection.execute(
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
        )?;

        Ok(Database {
            connection: Mutex::new(connection),
        })
    }

    /// Get a connection from the pool
    pub fn get_connection(&self) -> SqlResult<Connection> {
        let _guard = self.connection.lock().map_err(|e| {
            rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error {
                    code: rusqlite::ErrorCode::DatabaseBusy,
                    extended_code: 5, // SQLITE_BUSY
                },
                Some(e.to_string()),
            )
        })?;
        // Return a new connection by opening the same database
        // This is a workaround since MutexGuard cannot be cloned
        let db_path = store_dir().join("nuclaw.db");
        Connection::open(&db_path)
    }
}
