//! Telegram utility functions

use crate::telegram::policy::{ChunkMode, GroupPolicy};
use crate::types::NewMessage;

/// Default text chunk limit: 4000 characters
pub const DEFAULT_TEXT_CHUNK_LIMIT: usize = 4000;

/// Chunk text into smaller pieces (pure function)
pub fn chunk_text_pure(text: &str, chunk_limit: usize) -> Vec<String> {
    if text.len() <= chunk_limit {
        return vec![text.to_string()];
    }

    let mut chunks = Vec::new();
    let mut current = String::new();

    for paragraph in text.split("\n\n") {
        let para = paragraph;

        if para.len() > chunk_limit {
            if !current.is_empty() {
                chunks.push(current);
                current = String::new();
            }

            let mut remaining = para;
            while !remaining.is_empty() {
                let split_point = remaining.len().min(chunk_limit);
                chunks.push(remaining[..split_point].to_string());
                remaining = &remaining[split_point..];
            }
        } else if current.len() + para.len() + 2 > chunk_limit {
            if !current.is_empty() {
                chunks.push(current);
            }
            current = para.to_string();
        } else {
            if !current.is_empty() {
                current.push_str("\n\n");
            }
            current.push_str(para);
        }
    }

    if !current.is_empty() {
        chunks.push(current);
    }

    chunks
}

/// Advanced chunk text with configurable mode
pub fn chunk_text_advanced(text: &str, chunk_limit: usize, mode: ChunkMode) -> Vec<String> {
    match mode {
        ChunkMode::Length | ChunkMode::Newline => chunk_text_pure(text, chunk_limit),
    }
}

/// Extract chat ID from jid (pure function)
pub fn extract_chat_id_pure(jid: &str) -> Option<String> {
    jid.strip_prefix("telegram:group:").map(|s| s.to_string())
}

/// Check if message is duplicate (pure function)
pub fn is_duplicate_message_pure(
    msg: &NewMessage,
    last_timestamp: &str,
    last_agent_timestamps: &std::collections::HashMap<String, String>,
) -> bool {
    if last_timestamp == msg.timestamp {
        return true;
    }

    if let Some(agent_ts) = last_agent_timestamps.get(&msg.chat_jid) {
        if agent_ts == &msg.timestamp {
            return true;
        }
    }

    false
}

/// Check if group is allowed (pure function)
pub fn is_allowed_group_pure(
    chat_jid: &str,
    policy: GroupPolicy,
    allowed_groups: &[String],
) -> bool {
    match policy {
        GroupPolicy::Disabled => false,
        GroupPolicy::Open => true,
        GroupPolicy::Allowlist => {
            if let Some(chat_id) = chat_jid.strip_prefix("telegram:group:") {
                allowed_groups
                    .iter()
                    .any(|g| g == chat_id || g == &format!("-{}", chat_id))
            } else {
                false
            }
        }
    }
}

/// Helper to truncate strings
pub fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunk_text_short() {
        let chunks = chunk_text_pure("short text", 4000);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], "short text");
    }

    #[test]
    fn test_chunk_text_long_paragraphs() {
        let text = "This is paragraph one.\n\nThis is paragraph two.\n\nThis is paragraph three.";
        let chunks = chunk_text_pure(text, 20);
        assert!(chunks.len() > 1);
    }

    #[test]
    fn test_chunk_text_empty() {
        let chunks = chunk_text_pure("", 4000);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], "");
    }

    #[test]
    fn test_chunk_text_exact_limit() {
        let text = "12345678901234567890"; // 20 chars
        let chunks = chunk_text_pure(text, 20);
        assert_eq!(chunks.len(), 1);
    }

    #[test]
    fn test_extract_chat_id_pure() {
        assert_eq!(
            extract_chat_id_pure("telegram:group:-100123"),
            Some("-100123".to_string())
        );
        assert_eq!(
            extract_chat_id_pure("telegram:group:123"),
            Some("123".to_string())
        );
        assert_eq!(extract_chat_id_pure("invalid"), None);
    }

    #[test]
    fn test_is_allowed_group_pure() {
        let allowed = vec!["-100123".to_string(), "456".to_string()];

        assert!(is_allowed_group_pure(
            "telegram:group:-100123",
            GroupPolicy::Open,
            &allowed
        ));
        assert!(!is_allowed_group_pure(
            "telegram:group:-100123",
            GroupPolicy::Disabled,
            &allowed
        ));
        assert!(is_allowed_group_pure(
            "telegram:group:-100123",
            GroupPolicy::Allowlist,
            &allowed
        ));
        assert!(!is_allowed_group_pure(
            "telegram:group:999",
            GroupPolicy::Allowlist,
            &allowed
        ));
    }

    #[test]
    fn test_truncate() {
        assert_eq!(truncate("hello", 10), "hello");
        assert_eq!(truncate("hello world", 5), "he...");
        assert_eq!(truncate("hi", 10), "hi");
    }

    #[test]
    fn test_chunk_text_advanced() {
        let text = "Test\n\nTest";
        let chunks = chunk_text_advanced(text, 10, ChunkMode::Length);
        assert!(!chunks.is_empty());

        let chunks_newline = chunk_text_advanced(text, 10, ChunkMode::Newline);
        assert!(!chunks_newline.is_empty());
    }

    #[test]
    fn test_is_duplicate_message_pure() {
        let msg = NewMessage {
            id: "1".to_string(),
            chat_jid: "chat1".to_string(),
            sender: "user1".to_string(),
            sender_name: "User".to_string(),
            content: "test".to_string(),
            timestamp: "123".to_string(),
        };

        let mut last_agent = std::collections::HashMap::new();

        // Not duplicate
        assert!(!is_duplicate_message_pure(&msg, "456", &last_agent));

        // Duplicate by timestamp
        assert!(is_duplicate_message_pure(&msg, "123", &last_agent));

        // Duplicate by agent timestamp
        last_agent.insert("chat1".to_string(), "123".to_string());
        assert!(is_duplicate_message_pure(&msg, "456", &last_agent));
    }
}
