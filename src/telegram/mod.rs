//! Telegram Integration for NuClaw
//!
//! Provides Telegram Bot connectivity via Bot API with webhook support.
//! Follows OpenClaw Telegram specification for message handling.

pub mod client;
pub mod pairing;
pub mod policy;
pub mod types;
pub mod utils;

pub use client::TelegramClient;
pub use client::{load_registered_groups, load_router_state};
pub use pairing::PairingManager;
pub use policy::{ChunkMode, DMPolicy, GroupPolicy, ReplyMode, StreamMode};
pub use types::{TelegramChat, TelegramMessage, TelegramUpdate, TelegramUser};
pub use utils::{
    chunk_text_advanced, chunk_text_pure, extract_chat_id_pure, is_allowed_group_pure,
    is_duplicate_message_pure, truncate, DEFAULT_TEXT_CHUNK_LIMIT,
};

/// Determines whether Telegram should auto-start based on environment configuration.
///
/// Returns `true` if the `TELEGRAM_BOT_TOKEN` environment variable is set,
/// indicating that Telegram Bot should be automatically started.
///
/// # Examples
///
/// ```
/// use nuclaw::telegram::should_auto_start_telegram;
///
/// // Without token - returns false
/// std::env::remove_var("TELEGRAM_BOT_TOKEN");
/// assert_eq!(should_auto_start_telegram(), false);
///
/// // With token - returns true
/// std::env::set_var("TELEGRAM_BOT_TOKEN", "test_token");
/// assert_eq!(should_auto_start_telegram(), true);
/// std::env::remove_var("TELEGRAM_BOT_TOKEN");
/// ```
pub fn should_auto_start_telegram() -> bool {
    std::env::var("TELEGRAM_BOT_TOKEN").is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[test]
    fn test_module_exports() {
        let _ = TelegramClient::new;
        let _ = DMPolicy::parse;
        let _ = GroupPolicy::parse;
        let _ = chunk_text_pure;
        let _ = extract_chat_id_pure;
    }

    #[test]
    #[serial]
    fn test_should_auto_start_telegram_when_not_set() {
        // Save original value
        let original = std::env::var("TELEGRAM_BOT_TOKEN").ok();
        
        // Remove the variable
        std::env::remove_var("TELEGRAM_BOT_TOKEN");
        
        // Should return false
        assert_eq!(should_auto_start_telegram(), false);
        
        // Restore original
        match original {
            Some(val) => std::env::set_var("TELEGRAM_BOT_TOKEN", val),
            None => std::env::remove_var("TELEGRAM_BOT_TOKEN"),
        }
    }

    #[test]
    #[serial]
    fn test_should_auto_start_telegram_when_set() {
        // Save original value
        let original = std::env::var("TELEGRAM_BOT_TOKEN").ok();
        
        // Set a test token
        std::env::set_var("TELEGRAM_BOT_TOKEN", "123456:ABC-DEF1234ghIkl-zyx57W2v1u123ew11");
        
        // Should return true
        assert_eq!(should_auto_start_telegram(), true);
        
        // Restore original
        match original {
            Some(val) => std::env::set_var("TELEGRAM_BOT_TOKEN", val),
            None => std::env::remove_var("TELEGRAM_BOT_TOKEN"),
        }
    }

    #[test]
    #[serial]
    fn test_should_auto_start_telegram_with_empty_string() {
        // Save original value
        let original = std::env::var("TELEGRAM_BOT_TOKEN").ok();
        
        // Set empty string - this still counts as "set" in Rust
        // std::env::var("") returns Ok("") not Err
        std::env::set_var("TELEGRAM_BOT_TOKEN", "");
        
        // Should return true because the variable IS set (even if empty)
        // This is expected Rust behavior - var("") returns Ok
        assert_eq!(should_auto_start_telegram(), true);
        
        // Restore original
        match original {
            Some(val) => std::env::set_var("TELEGRAM_BOT_TOKEN", val),
            None => std::env::remove_var("TELEGRAM_BOT_TOKEN"),
        }
    }
}
