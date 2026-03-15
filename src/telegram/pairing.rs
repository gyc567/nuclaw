use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::config::nuclaw_home;
use crate::error::{NuClawError, Result};

const PAIRING_CODE_LENGTH: usize = 6;
const PAIRING_CODE_EXPIRE_MINUTES: u64 = 10;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingCode {
    pub user_id: String,
    pub chat_id: i64,
    pub created_at: u64,
    pub expires_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthorizedUser {
    pub user_id: String,
    pub chat_id: i64,
    pub authorized_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PairingStorage {
    pub pending_codes: HashMap<String, PendingCode>,
    pub authorized_users: HashMap<String, AuthorizedUser>,
}

pub struct PairingManager {
    storage_path: PathBuf,
    storage: PairingStorage,
}

impl PairingManager {
    pub fn new() -> Result<Self> {
        let storage_path = nuclaw_home().join("pairing.json");
        let storage = if storage_path.exists() {
            let content =
                fs::read_to_string(&storage_path).map_err(|e| NuClawError::FileSystem {
                    message: format!("Failed to read pairing storage: {}", e),
                })?;
            serde_json::from_str(&content).unwrap_or_default()
        } else {
            PairingStorage::default()
        };
        Ok(Self {
            storage_path,
            storage,
        })
    }

    fn save(&self) -> Result<()> {
        if let Some(parent) = self.storage_path.parent() {
            fs::create_dir_all(parent).map_err(|e| NuClawError::FileSystem {
                message: format!("Failed to create directory: {}", e),
            })?;
        }
        let content =
            serde_json::to_string_pretty(&self.storage).map_err(|e| NuClawError::FileSystem {
                message: format!("Failed to serialize pairing storage: {}", e),
            })?;
        fs::write(&self.storage_path, content).map_err(|e| NuClawError::FileSystem {
            message: format!("Failed to write pairing storage: {}", e),
        })?;
        Ok(())
    }

    fn generate_random_code() -> String {
        use std::iter;
        const CHARSET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
        iter::repeat_with(|| {
            let idx = rand_u32() as usize % CHARSET.len();
            CHARSET[idx] as char
        })
        .take(PAIRING_CODE_LENGTH)
        .collect()
    }

    pub fn generate_code(&mut self, user_id: &str, chat_id: i64) -> Result<String> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let expires_at = now + (PAIRING_CODE_EXPIRE_MINUTES * 60);

        let code = Self::generate_random_code();

        let pending = PendingCode {
            user_id: user_id.to_string(),
            chat_id,
            created_at: now,
            expires_at,
        };

        self.storage.pending_codes.insert(code.clone(), pending);
        self.save()?;

        Ok(code)
    }

    pub fn verify_code(&self, code: &str) -> Result<Option<PendingCode>> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        if let Some(pending) = self.storage.pending_codes.get(code) {
            if now <= pending.expires_at {
                return Ok(Some(pending.clone()));
            }
        }
        Ok(None)
    }

    pub fn authorize_user(&mut self, pending: PendingCode) -> Result<()> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let authorized = AuthorizedUser {
            user_id: pending.user_id.clone(),
            chat_id: pending.chat_id,
            authorized_at: now,
        };

        self.storage
            .authorized_users
            .insert(pending.user_id.clone(), authorized);
        self.storage
            .pending_codes
            .retain(|k, v| k != &pending.user_id);
        self.save()?;

        Ok(())
    }

    pub fn is_authorized(&self, user_id: &str) -> bool {
        self.storage.authorized_users.contains_key(user_id)
    }

    pub fn list_authorized(&self) -> Vec<AuthorizedUser> {
        self.storage.authorized_users.values().cloned().collect()
    }

    pub fn deauthorize_user(&mut self, user_id: &str) -> Result<bool> {
        if self.storage.authorized_users.remove(user_id).is_some() {
            self.save()?;
            return Ok(true);
        }
        Ok(false)
    }

    pub fn cleanup_expired_codes(&mut self) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        self.storage
            .pending_codes
            .retain(|_, v| now <= v.expires_at);
    }
}

fn rand_u32() -> u32 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .subsec_nanos();
    nanos.wrapping_mul(1103515245).wrapping_add(12345)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn setup_test() -> String {
        let counter = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let test_dir = format!("/tmp/nuclaw_pairing_test_{}", counter);
        env::set_var("NUCLAW_HOME", &test_dir);
        let _ = fs::create_dir_all(&test_dir);
        test_dir
    }

    fn cleanup_test(test_dir: &str) {
        let _ = fs::remove_dir_all(test_dir);
    }

    #[test]
    fn test_generate_code() {
        let test_dir = setup_test();
        let mut manager = PairingManager::new().unwrap();
        let code = manager.generate_code("user123", 123456789).unwrap();
        assert_eq!(code.len(), 6);
        assert!(code.chars().all(|c| c.is_ascii_alphanumeric()));
        cleanup_test(&test_dir);
    }

    #[test]
    fn test_verify_code() {
        let test_dir = setup_test();
        let mut manager = PairingManager::new().unwrap();
        let code = manager.generate_code("user123", 123456789).unwrap();
        let pending = manager.verify_code(&code).unwrap();
        assert!(pending.is_some());
        assert_eq!(pending.unwrap().user_id, "user123");
        cleanup_test(&test_dir);
    }

    #[test]
    fn test_verify_invalid_code() {
        let test_dir = setup_test();
        let manager = PairingManager::new().unwrap();
        let pending = manager.verify_code("INVALID").unwrap();
        assert!(pending.is_none());
        cleanup_test(&test_dir);
    }

    #[test]
    fn test_authorize_user() {
        let test_dir = setup_test();
        let mut manager = PairingManager::new().unwrap();
        let code = manager.generate_code("user123", 123456789).unwrap();
        let pending = manager.verify_code(&code).unwrap().unwrap();
        manager.authorize_user(pending).unwrap();
        assert!(manager.is_authorized("user123"));
        cleanup_test(&test_dir);
    }

    #[test]
    fn test_is_authorized() {
        let test_dir = setup_test();
        let mut manager = PairingManager::new().unwrap();
        assert!(!manager.is_authorized("user123"));
        let code = manager.generate_code("user123", 123456789).unwrap();
        let pending = manager.verify_code(&code).unwrap().unwrap();
        manager.authorize_user(pending).unwrap();
        assert!(manager.is_authorized("user123"));
        cleanup_test(&test_dir);
    }

    #[test]
    fn test_deauthorize_user() {
        let test_dir = setup_test();
        let mut manager = PairingManager::new().unwrap();
        let code = manager.generate_code("user123", 123456789).unwrap();
        let pending = manager.verify_code(&code).unwrap().unwrap();
        manager.authorize_user(pending).unwrap();
        assert!(manager.is_authorized("user123"));
        manager.deauthorize_user("user123").unwrap();
        assert!(!manager.is_authorized("user123"));
        cleanup_test(&test_dir);
    }

    #[test]
    fn test_list_authorized() {
        let test_dir = setup_test();
        let mut manager = PairingManager::new().unwrap();
        let code1 = manager.generate_code("user1", 111).unwrap();
        let code2 = manager.generate_code("user2", 222).unwrap();
        let pending1 = manager.verify_code(&code1).unwrap().unwrap();
        let pending2 = manager.verify_code(&code2).unwrap().unwrap();
        manager.authorize_user(pending1).unwrap();
        manager.authorize_user(pending2).unwrap();
        let list = manager.list_authorized();
        assert_eq!(list.len(), 2);
        cleanup_test(&test_dir);
    }
}
