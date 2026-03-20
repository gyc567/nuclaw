//! User preferences module for NuClaw
//!
//! Provides CRUD operations for storing and managing user preferences.
//! Uses r2d2 for connection pooling and rusqlite for SQLite access.
//! All operations return Result<T> with proper error handling.

use crate::db::Database;
use crate::error::NuClawError;
use chrono::{DateTime, Utc};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// User preference entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserPreference {
    /// Unique identifier for the user
    pub user_id: String,
    /// Preference key
    pub key: String,
    /// Preference value (JSON serialized)
    pub value: String,
    /// When the preference was created
    pub created_at: DateTime<Utc>,
    /// When the preference was last updated
    pub updated_at: DateTime<Utc>,
}

/// User preferences manager
#[derive(Debug, Clone)]
pub struct UserPreferences {
    db: Database,
}

impl UserPreferences {
    /// Create a new UserPreferences manager with the given database
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    /// Initialize the user_preferences table in the database
    pub fn initialize_table(&self) -> Result<(), NuClawError> {
        let conn = self.db.get_connection()?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS user_preferences (
                user_id TEXT NOT NULL,
                key TEXT NOT NULL,
                value TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                PRIMARY KEY (user_id, key)
            )",
            [],
        )
        .map_err(|e| NuClawError::Database {
            message: format!("Failed to create user_preferences table: {}", e),
        })?;

        // Create index for faster lookups by user_id
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_user_preferences_user_id ON user_preferences(user_id)",
            [],
        )
        .map_err(|e| NuClawError::Database {
            message: format!("Failed to create user_preferences user_id index: {}", e),
        })?;

        Ok(())
    }

    /// Create or update a user preference
    pub fn set_preference(
        &self,
        user_id: &str,
        key: &str,
        value: &str,
    ) -> Result<UserPreference, NuClawError> {
        let conn = self.db.get_connection()?;
        let now = Utc::now();

        conn.execute(
            "INSERT INTO user_preferences (user_id, key, value, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(user_id, key) DO UPDATE SET
                value = excluded.value,
                updated_at = excluded.updated_at",
            params![user_id, key, value, now.to_rfc3339(), now.to_rfc3339()],
        )
        .map_err(|e| NuClawError::Database {
            message: format!("Failed to set preference: {}", e),
        })?;

        Ok(UserPreference {
            user_id: user_id.to_string(),
            key: key.to_string(),
            value: value.to_string(),
            created_at: now,
            updated_at: now,
        })
    }

    /// Get a single user preference by user_id and key
    pub fn get_preference(
        &self,
        user_id: &str,
        key: &str,
    ) -> Result<Option<UserPreference>, NuClawError> {
        let conn = self.db.get_connection()?;

        let mut stmt = conn
            .prepare(
                "SELECT user_id, key, value, created_at, updated_at
                 FROM user_preferences
                 WHERE user_id = ?1 AND key = ?2",
            )
            .map_err(|e| NuClawError::Database {
                message: format!("Failed to prepare get preference query: {}", e),
            })?;

        let result = stmt
            .query_row(params![user_id, key], |row| {
                Ok(UserPreference {
                    user_id: row.get(0)?,
                    key: row.get(1)?,
                    value: row.get(2)?,
                    created_at: row
                        .get::<_, String>(3)?
                        .parse::<DateTime<Utc>>()
                        .map_err(|e| {
                            rusqlite::Error::FromSqlConversionFailure(
                                3,
                                rusqlite::types::Type::Text,
                                Box::new(e),
                            )
                        })?,
                    updated_at: row
                        .get::<_, String>(4)?
                        .parse::<DateTime<Utc>>()
                        .map_err(|e| {
                            rusqlite::Error::FromSqlConversionFailure(
                                4,
                                rusqlite::types::Type::Text,
                                Box::new(e),
                            )
                        })?,
                })
            })
            .optional()
            .map_err(|e| NuClawError::Database {
                message: format!("Failed to get preference: {}", e),
            })?;

        Ok(result)
    }

    /// Get all preferences for a user
    pub fn get_user_preferences(&self, user_id: &str) -> Result<Vec<UserPreference>, NuClawError> {
        let conn = self.db.get_connection()?;

        let mut stmt = conn
            .prepare(
                "SELECT user_id, key, value, created_at, updated_at
                 FROM user_preferences
                 WHERE user_id = ?1
                 ORDER BY key",
            )
            .map_err(|e| NuClawError::Database {
                message: format!("Failed to prepare get user preferences query: {}", e),
            })?;

        let preferences: Result<Vec<UserPreference>, rusqlite::Error> = stmt
            .query_map(params![user_id], |row| {
                let created_at_str: String = row.get(3)?;
                let updated_at_str: String = row.get(4)?;

                Ok(UserPreference {
                    user_id: row.get(0)?,
                    key: row.get(1)?,
                    value: row.get(2)?,
                    created_at: created_at_str.parse::<DateTime<Utc>>().map_err(|e| {
                        rusqlite::Error::FromSqlConversionFailure(
                            3,
                            rusqlite::types::Type::Text,
                            Box::new(e),
                        )
                    })?,
                    updated_at: updated_at_str.parse::<DateTime<Utc>>().map_err(|e| {
                        rusqlite::Error::FromSqlConversionFailure(
                            4,
                            rusqlite::types::Type::Text,
                            Box::new(e),
                        )
                    })?,
                })
            })?
            .collect();

        preferences.map_err(|e| NuClawError::Database {
            message: format!("Failed to get user preferences: {}", e),
        })
    }

    /// Get all preferences for a user as a HashMap
    pub fn get_user_preferences_map(
        &self,
        user_id: &str,
    ) -> Result<HashMap<String, String>, NuClawError> {
        let preferences = self.get_user_preferences(user_id)?;
        let mut map = HashMap::new();

        for pref in preferences {
            map.insert(pref.key, pref.value);
        }

        Ok(map)
    }

    /// Delete a single user preference
    pub fn delete_preference(&self, user_id: &str, key: &str) -> Result<bool, NuClawError> {
        let conn = self.db.get_connection()?;

        let rows_affected = conn
            .execute(
                "DELETE FROM user_preferences WHERE user_id = ?1 AND key = ?2",
                params![user_id, key],
            )
            .map_err(|e| NuClawError::Database {
                message: format!("Failed to delete preference: {}", e),
            })?;

        Ok(rows_affected > 0)
    }

    /// Delete all preferences for a user
    pub fn delete_user_preferences(&self, user_id: &str) -> Result<usize, NuClawError> {
        let conn = self.db.get_connection()?;

        let rows_affected = conn
            .execute(
                "DELETE FROM user_preferences WHERE user_id = ?1",
                params![user_id],
            )
            .map_err(|e| NuClawError::Database {
                message: format!("Failed to delete user preferences: {}", e),
            })?;

        Ok(rows_affected)
    }

    /// Check if a preference exists
    pub fn preference_exists(&self, user_id: &str, key: &str) -> Result<bool, NuClawError> {
        let conn = self.db.get_connection()?;

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM user_preferences WHERE user_id = ?1 AND key = ?2",
                params![user_id, key],
                |row| row.get(0),
            )
            .map_err(|e| NuClawError::Database {
                message: format!("Failed to check preference existence: {}", e),
            })?;

        Ok(count > 0)
    }

    /// Get all users with preferences
    pub fn get_all_users(&self) -> Result<Vec<String>, NuClawError> {
        let conn = self.db.get_connection()?;

        let mut stmt = conn
            .prepare("SELECT DISTINCT user_id FROM user_preferences ORDER BY user_id")
            .map_err(|e| NuClawError::Database {
                message: format!("Failed to prepare get all users query: {}", e),
            })?;

        let users: Result<Vec<String>, rusqlite::Error> =
            stmt.query_map([], |row| row.get(0))?.collect();

        users.map_err(|e| NuClawError::Database {
            message: format!("Failed to get all users: {}", e),
        })
    }

    /// Count total preferences
    pub fn count_preferences(&self) -> Result<usize, NuClawError> {
        let conn = self.db.get_connection()?;

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM user_preferences", [], |row| {
                row.get(0)
            })
            .map_err(|e| NuClawError::Database {
                message: format!("Failed to count preferences: {}", e),
            })?;

        Ok(count as usize)
    }

    /// Count preferences for a specific user
    pub fn count_user_preferences(&self, user_id: &str) -> Result<usize, NuClawError> {
        let conn = self.db.get_connection()?;

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM user_preferences WHERE user_id = ?1",
                params![user_id],
                |row| row.get(0),
            )
            .map_err(|e| NuClawError::Database {
                message: format!("Failed to count user preferences: {}", e),
            })?;

        Ok(count as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{Database, DatabaseConfig};
    use std::fs;
    use std::path::PathBuf;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn store_dir() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".nuclaw")
    }

    fn setup_test_dirs() {
        let _guard = ENV_LOCK.lock().unwrap();
        let store = store_dir();
        if !store.exists() {
            let _ = fs::create_dir_all(&store);
        }
    }

    fn test_db_path() -> PathBuf {
        store_dir().join("test_user_preferences.db")
    }

    fn cleanup_test_db(path: &PathBuf) {
        let _guard = ENV_LOCK.lock().unwrap();
        let _ = fs::remove_file(path);
        let _ = fs::remove_file(path.with_extension("db-wal"));
        let _ = fs::remove_file(path.with_extension("db-shm"));
    }

    fn create_test_db() -> Database {
        setup_test_dirs();
        let db_path = test_db_path();
        cleanup_test_db(&db_path);

        let config = DatabaseConfig {
            db_path: db_path.clone(),
            pool_size: 3,
            connection_timeout_ms: 5000,
        };

        Database::with_config(config).unwrap()
    }

    #[test]
    fn test_initialize_table() {
        let db = create_test_db();
        let prefs = UserPreferences::new(db);

        let result = prefs.initialize_table();
        assert!(result.is_ok(), "Table should be initialized successfully");

        cleanup_test_db(&test_db_path());
    }

    #[test]
    fn test_set_and_get_preference() {
        let db = create_test_db();
        let prefs = UserPreferences::new(db);
        prefs.initialize_table().unwrap();

        let user_id = "user123";
        let key = "theme";
        let value = "dark";

        let result = prefs.set_preference(user_id, key, value);
        assert!(result.is_ok(), "Should set preference successfully");

        let pref = prefs.get_preference(user_id, key).unwrap();
        assert!(pref.is_some(), "Preference should exist");

        let pref = pref.unwrap();
        assert_eq!(pref.user_id, user_id);
        assert_eq!(pref.key, key);
        assert_eq!(pref.value, value);

        cleanup_test_db(&test_db_path());
    }

    #[test]
    fn test_get_nonexistent_preference() {
        let db = create_test_db();
        let prefs = UserPreferences::new(db);
        prefs.initialize_table().unwrap();

        let result = prefs.get_preference("nonexistent", "key").unwrap();
        assert!(
            result.is_none(),
            "Should return None for nonexistent preference"
        );

        cleanup_test_db(&test_db_path());
    }

    #[test]
    fn test_update_preference() {
        let db = create_test_db();
        let prefs = UserPreferences::new(db);
        prefs.initialize_table().unwrap();

        let user_id = "user123";
        let key = "theme";

        prefs.set_preference(user_id, key, "dark").unwrap();
        prefs.set_preference(user_id, key, "light").unwrap();

        let pref = prefs.get_preference(user_id, key).unwrap().unwrap();
        assert_eq!(pref.value, "light", "Value should be updated");
        assert!(
            pref.updated_at > pref.created_at,
            "Updated_at should be after created_at"
        );

        cleanup_test_db(&test_db_path());
    }

    #[test]
    fn test_get_user_preferences() {
        let db = create_test_db();
        let prefs = UserPreferences::new(db);
        prefs.initialize_table().unwrap();

        let user_id = "user123";
        prefs.set_preference(user_id, "theme", "dark").unwrap();
        prefs.set_preference(user_id, "language", "en").unwrap();
        prefs
            .set_preference(user_id, "notifications", "true")
            .unwrap();

        let user_prefs = prefs.get_user_preferences(user_id).unwrap();
        assert_eq!(user_prefs.len(), 3, "Should have 3 preferences");

        cleanup_test_db(&test_db_path());
    }

    #[test]
    fn test_get_user_preferences_map() {
        let db = create_test_db();
        let prefs = UserPreferences::new(db);
        prefs.initialize_table().unwrap();

        let user_id = "user123";
        prefs.set_preference(user_id, "theme", "dark").unwrap();
        prefs.set_preference(user_id, "language", "en").unwrap();

        let map = prefs.get_user_preferences_map(user_id).unwrap();
        assert_eq!(map.len(), 2);
        assert_eq!(map.get("theme"), Some(&"dark".to_string()));
        assert_eq!(map.get("language"), Some(&"en".to_string()));

        cleanup_test_db(&test_db_path());
    }

    #[test]
    fn test_delete_preference() {
        let db = create_test_db();
        let prefs = UserPreferences::new(db);
        prefs.initialize_table().unwrap();

        let user_id = "user123";
        prefs.set_preference(user_id, "theme", "dark").unwrap();

        let deleted = prefs.delete_preference(user_id, "theme").unwrap();
        assert!(deleted, "Should return true when preference is deleted");

        let pref = prefs.get_preference(user_id, "theme").unwrap();
        assert!(pref.is_none(), "Preference should be deleted");

        let not_deleted = prefs.delete_preference(user_id, "nonexistent").unwrap();
        assert!(
            !not_deleted,
            "Should return false when preference doesn't exist"
        );

        cleanup_test_db(&test_db_path());
    }

    #[test]
    fn test_delete_user_preferences() {
        let db = create_test_db();
        let prefs = UserPreferences::new(db);
        prefs.initialize_table().unwrap();

        let user_id = "user123";
        prefs.set_preference(user_id, "key1", "value1").unwrap();
        prefs.set_preference(user_id, "key2", "value2").unwrap();
        prefs.set_preference(user_id, "key3", "value3").unwrap();

        let deleted_count = prefs.delete_user_preferences(user_id).unwrap();
        assert_eq!(deleted_count, 3, "Should delete all 3 preferences");

        let user_prefs = prefs.get_user_preferences(user_id).unwrap();
        assert!(user_prefs.is_empty(), "Should have no preferences left");

        cleanup_test_db(&test_db_path());
    }

    #[test]
    fn test_preference_exists() {
        let db = create_test_db();
        let prefs = UserPreferences::new(db);
        prefs.initialize_table().unwrap();

        let user_id = "user123";
        prefs.set_preference(user_id, "theme", "dark").unwrap();

        assert!(
            prefs.preference_exists(user_id, "theme").unwrap(),
            "Should exist"
        );
        assert!(
            !prefs.preference_exists(user_id, "nonexistent").unwrap(),
            "Should not exist"
        );
        assert!(
            !prefs.preference_exists("other_user", "theme").unwrap(),
            "Should not exist for other user"
        );

        cleanup_test_db(&test_db_path());
    }

    #[test]
    fn test_get_all_users() {
        let db = create_test_db();
        let prefs = UserPreferences::new(db);
        prefs.initialize_table().unwrap();

        prefs.set_preference("user1", "key", "value").unwrap();
        prefs.set_preference("user2", "key", "value").unwrap();
        prefs.set_preference("user3", "key", "value").unwrap();

        let users = prefs.get_all_users().unwrap();
        assert_eq!(users.len(), 3, "Should have 3 unique users");
        assert!(users.contains(&"user1".to_string()));
        assert!(users.contains(&"user2".to_string()));
        assert!(users.contains(&"user3".to_string()));

        cleanup_test_db(&test_db_path());
    }

    #[test]
    fn test_count_preferences() {
        let db = create_test_db();
        let prefs = UserPreferences::new(db);
        prefs.initialize_table().unwrap();

        assert_eq!(prefs.count_preferences().unwrap(), 0, "Should start with 0");

        prefs.set_preference("user1", "key1", "value1").unwrap();
        prefs.set_preference("user1", "key2", "value2").unwrap();
        prefs.set_preference("user2", "key1", "value1").unwrap();

        assert_eq!(prefs.count_preferences().unwrap(), 3, "Should have 3 total");

        cleanup_test_db(&test_db_path());
    }

    #[test]
    fn test_count_user_preferences() {
        let db = create_test_db();
        let prefs = UserPreferences::new(db);
        prefs.initialize_table().unwrap();

        let user_id = "user123";
        assert_eq!(
            prefs.count_user_preferences(user_id).unwrap(),
            0,
            "Should start with 0"
        );

        prefs.set_preference(user_id, "key1", "value1").unwrap();
        prefs.set_preference(user_id, "key2", "value2").unwrap();
        prefs
            .set_preference("other_user", "key1", "value1")
            .unwrap();

        assert_eq!(
            prefs.count_user_preferences(user_id).unwrap(),
            2,
            "Should have 2 for user"
        );

        cleanup_test_db(&test_db_path());
    }

    #[test]
    fn test_user_preference_struct() {
        let pref = UserPreference {
            user_id: "user123".to_string(),
            key: "theme".to_string(),
            value: "dark".to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        assert_eq!(pref.user_id, "user123");
        assert_eq!(pref.key, "theme");
        assert_eq!(pref.value, "dark");
    }
}
