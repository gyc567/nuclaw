//! Telegram API types

use serde::{Deserialize, Serialize};

/// Telegram Update object (Telegram Bot API)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelegramUpdate {
    pub update_id: i64,
    pub message: Option<TelegramMessage>,
    pub edited_message: Option<TelegramMessage>,
}

/// Telegram User object
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelegramUser {
    pub id: i64,
    pub is_bot: bool,
    pub first_name: String,
    pub last_name: Option<String>,
    pub username: Option<String>,
}

/// Telegram Chat object
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelegramChat {
    pub id: i64,
    #[serde(rename = "type")]
    pub chat_type: String,
    pub title: Option<String>,
}

/// Telegram Message object
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelegramMessage {
    pub message_id: i64,
    pub from: Option<TelegramUser>,
    pub chat: TelegramChat,
    pub date: i64,
    pub text: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_telegram_update() {
        let json = r#"{
            "update_id": 123,
            "message": {
                "message_id": 456,
                "from": {"id": 789, "is_bot": false, "first_name": "Test"},
                "chat": {"id": -100123, "type": "supergroup", "title": "Test Group"},
                "date": 1234567890,
                "text": "@Andy hello"
            }
        }"#;

        let update: TelegramUpdate = serde_json::from_str(json).unwrap();
        assert_eq!(update.update_id, 123);
        assert!(update.message.is_some());
    }

    #[test]
    fn test_telegram_user_serialization() {
        let user = TelegramUser {
            id: 123,
            is_bot: false,
            first_name: "John".to_string(),
            last_name: Some("Doe".to_string()),
            username: Some("johndoe".to_string()),
        };

        let json = serde_json::to_string(&user).unwrap();
        assert!(json.contains("first_name"));
        assert!(json.contains("John"));
    }

    #[test]
    fn test_telegram_message_serialization() {
        let msg = TelegramMessage {
            message_id: 1,
            from: Some(TelegramUser {
                id: 123,
                is_bot: false,
                first_name: "Test".to_string(),
                last_name: None,
                username: None,
            }),
            chat: TelegramChat {
                id: -100123,
                chat_type: "supergroup".to_string(),
                title: Some("Test Group".to_string()),
            },
            date: 1234567890,
            text: Some("Hello".to_string()),
        };

        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("Hello"));
    }
}
