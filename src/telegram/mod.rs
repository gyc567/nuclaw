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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_exports() {
        let _ = TelegramClient::new;
        let _ = DMPolicy::parse;
        let _ = GroupPolicy::parse;
        let _ = chunk_text_pure;
        let _ = extract_chat_id_pure;
    }
}
