use std::sync::Arc;
use crate::runtime::Runtime;
use crate::types::{ContainerInput, ContainerOutput, AppEvent};
use crate::error::Result;

pub struct EventRouter {
    runtime: Arc<dyn Runtime>,
}

impl EventRouter {
    pub fn new(runtime: Arc<dyn Runtime>) -> Self {
        Self { runtime }
    }

    /// Legacy method for direct container execution (Phase 1)
    pub async fn handle_event(&self, input: ContainerInput) -> Result<ContainerOutput> {
        self.runtime.run(input).await
    }

    /// New standardized entry point for all application events (Phase 2)
    pub async fn dispatch(&self, event: AppEvent) -> Result<ContainerOutput> {
        let input = self.map_event_to_input(event).await?;
        self.runtime.run(input).await
    }

    /// Internal logic to transform high-level events into execution requests.
    /// This is where business logic like "which group folder to use" resides.
    async fn map_event_to_input(&self, event: AppEvent) -> Result<ContainerInput> {
        match event {
            AppEvent::ChatMessage { platform, chat_id, user_id: _, message_id, message_text, group_folder, is_group } => {
                Ok(ContainerInput {
                    prompt: message_text,
                    session_id: Some(format!("{}_{}", platform, message_id)),
                    group_folder,
                    chat_jid: chat_id,
                    is_main: !is_group,
                    is_scheduled_task: false,
                    session_workspace_id: None,
                })
            }
            AppEvent::ScheduledTask { task_id } => {
                Ok(ContainerInput {
                    prompt: format!("Running scheduled task: {}", task_id),
                    session_id: None,
                    group_folder: "system".to_string(),
                    chat_jid: "system".to_string(),
                    is_main: true,
                    is_scheduled_task: true,
                    session_workspace_id: None,
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::mock::MockRuntime;
    use crate::types::{ContainerInput, ContainerOutput, AppEvent};

    #[tokio::test]
    async fn test_event_router_delegates_to_runtime() {
        let mock_output = ContainerOutput {
            status: "success".to_string(),
            result: Some("hello from mock".to_string()),
            new_session_id: None,
            error: None,
        };

        let mock_runtime = Arc::new(MockRuntime::new(mock_output.clone()));
        let router = EventRouter::new(mock_runtime.clone());

        let input = ContainerInput {
            prompt: "test prompt".to_string(),
            session_id: None,
            group_folder: "test_group".to_string(),
            chat_jid: "test_jid".to_string(),
            is_main: true,
            is_scheduled_task: false,
            session_workspace_id: None,
        };

        let result = router.handle_event(input.clone()).await.unwrap();

        assert_eq!(result.status, "success");
        let invocations = mock_runtime.invocations.lock().unwrap();
        assert_eq!(invocations.len(), 1);
    }

    #[tokio::test]
    async fn test_dispatch_chat_message() {
        let mock_output = ContainerOutput {
            status: "success".to_string(),
            result: Some("processed".to_string()),
            new_session_id: None,
            error: None,
        };

        let mock_runtime = Arc::new(MockRuntime::new(mock_output));
        let router = EventRouter::new(mock_runtime.clone());

        let event = AppEvent::ChatMessage {
            platform: "telegram".to_string(),
            chat_id: "12345".to_string(),
            user_id: "678".to_string(),
            message_id: "msg_99".to_string(),
            message_text: "hi bot".to_string(),
            group_folder: "telegram_data".to_string(),
            is_group: false,
        };

        let _ = router.dispatch(event).await.unwrap();

        let invocations = mock_runtime.invocations.lock().unwrap();
        assert_eq!(invocations.len(), 1);
        assert_eq!(invocations[0].prompt, "hi bot");
        assert_eq!(invocations[0].session_id, Some("telegram_msg_99".to_string()));
        assert_eq!(invocations[0].group_folder, "telegram_data");
        assert_eq!(invocations[0].chat_jid, "12345");
        assert_eq!(invocations[0].is_scheduled_task, false);
    }

    #[tokio::test]
    async fn test_dispatch_scheduled_task() {
        let mock_runtime = Arc::new(MockRuntime::new(ContainerOutput {
            status: "success".to_string(),
            result: None,
            new_session_id: None,
            error: None,
        }));
        let router = EventRouter::new(mock_runtime.clone());

        let event = AppEvent::ScheduledTask {
            task_id: "daily_report".to_string(),
        };

        let _ = router.dispatch(event).await.unwrap();

        let invocations = mock_runtime.invocations.lock().unwrap();
        assert_eq!(invocations.len(), 1);
        assert!(invocations[0].prompt.contains("daily_report"));
        assert_eq!(invocations[0].is_scheduled_task, true);
    }
}
