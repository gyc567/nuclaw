//! Core types for NuClaw

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisteredGroup {
    pub name: String,
    pub folder: String,
    pub trigger: String,
    pub added_at: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Session(HashMap<String, String>);

impl Session {
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduledTask {
    pub id: String,
    pub group_folder: String,
    pub chat_jid: String,
    pub prompt: String,
    pub schedule_type: String,
    pub schedule_value: String,
    pub context_mode: String,
    pub next_run: Option<String>,
    pub last_run: Option<String>,
    pub last_result: Option<String>,
    pub status: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskRunLog {
    pub task_id: String,
    pub run_at: String,
    pub duration_ms: i64,
    pub status: String,
    pub result: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewMessage {
    pub id: String,
    pub chat_jid: String,
    pub sender: String,
    pub sender_name: String,
    pub content: String,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatInfo {
    pub jid: String,
    pub name: String,
    pub last_message_time: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerInput {
    pub prompt: String,
    pub session_id: Option<String>,
    pub group_folder: String,
    pub chat_jid: String,
    pub is_main: bool,
    pub is_scheduled_task: bool,
    /// Session workspace ID (set when using per-session workspace)
    pub session_workspace_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerOutput {
    pub status: String,
    pub result: Option<String>,
    pub new_session_id: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AppEvent {
    ChatMessage {
        platform: String,
        chat_id: String,
        user_id: String,
        message_id: String,
        message_text: String,
        group_folder: String,
        is_group: bool,
    },
    ScheduledTask {
        task_id: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RouterState {
    pub last_timestamp: String,
    pub last_agent_timestamp: HashMap<String, String>,
    /// Tracks the last message ID per chat to detect duplicates
    pub last_message_ids: HashMap<String, String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registered_group() {
        let group = RegisteredGroup {
            name: "Test Group".to_string(),
            folder: "test_group".to_string(),
            trigger: "@Andy".to_string(),
            added_at: "2025-01-01T00:00:00Z".to_string(),
        };
        assert_eq!(group.name, "Test Group");
        assert_eq!(group.folder, "test_group");
    }

    #[test]
    fn test_session() {
        let mut session = Session::new();
        session.0.insert("key1".to_string(), "value1".to_string());
        assert_eq!(session.len(), 1);
    }

    #[test]
    fn test_scheduled_task() {
        let task = ScheduledTask {
            id: "task_1".to_string(),
            group_folder: "group_1".to_string(),
            chat_jid: "chat_1".to_string(),
            prompt: "test prompt".to_string(),
            schedule_type: "cron".to_string(),
            schedule_value: "0 0 9 * * *".to_string(),
            context_mode: "isolated".to_string(),
            next_run: Some("2025-01-01T09:00:00Z".to_string()),
            last_run: None,
            last_result: None,
            status: "active".to_string(),
            created_at: "2025-01-01T00:00:00Z".to_string(),
        };
        assert_eq!(task.schedule_type, "cron");
        assert_eq!(task.status, "active");
    }

    #[test]
    fn test_container_input() {
        let input = ContainerInput {
            prompt: "test".to_string(),
            session_id: Some("sess_123".to_string()),
            group_folder: "group_1".to_string(),
            chat_jid: "chat_1".to_string(),
            is_main: true,
            is_scheduled_task: false,
            session_workspace_id: Some("ws_456".to_string()),
        };
        assert!(input.session_id.is_some());
        assert!(input.is_main);
        assert!(input.session_workspace_id.is_some());
    }

    #[test]
    fn test_container_output() {
        let output = ContainerOutput {
            status: "success".to_string(),
            result: Some("result".to_string()),
            new_session_id: Some("new_sess".to_string()),
            error: None,
        };
        assert_eq!(output.status, "success");
        assert!(output.result.is_some());
        assert!(output.error.is_none());
    }

    #[test]
    fn test_router_state() {
        let mut state = RouterState::default();
        state.last_timestamp = "2025-01-01T00:00:00Z".to_string();
        state
            .last_agent_timestamp
            .insert("chat_1".to_string(), "2025-01-01T00:00:00Z".to_string());
        assert_eq!(state.last_agent_timestamp.len(), 1);
    }

    #[test]
    fn test_new_message() {
        let msg = NewMessage {
            id: "msg_1".to_string(),
            chat_jid: "chat_1".to_string(),
            sender: "user_1".to_string(),
            sender_name: "Test User".to_string(),
            content: "Hello".to_string(),
            timestamp: "2025-01-01T00:00:00Z".to_string(),
        };
        assert_eq!(msg.content, "Hello");
    }

    #[test]
    fn test_task_run_log() {
        let log = TaskRunLog {
            task_id: "task_1".to_string(),
            run_at: "2025-01-01T00:00:00Z".to_string(),
            duration_ms: 1000,
            status: "success".to_string(),
            result: Some("ok".to_string()),
            error: None,
        };
        assert_eq!(log.duration_ms, 1000);
    }

    #[test]
    fn test_chat_info() {
        let info = ChatInfo {
            jid: "chat_1".to_string(),
            name: "Test Chat".to_string(),
            last_message_time: "2025-01-01T00:00:00Z".to_string(),
        };
        assert_eq!(info.name, "Test Chat");
    }
}
