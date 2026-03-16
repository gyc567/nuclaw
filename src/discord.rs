//! Discord Integration Skeleton for NuClaw
//!
//! Provides Discord connectivity (future).
//! Implements the universal EventRouter pattern established in Phase 2/3.

use crate::error::Result;
use crate::router::EventRouter;
use std::sync::Arc;

/// Discord client state
pub struct DiscordClient {
    /// Event router for message dispatching
    router: Arc<EventRouter>,
}

impl DiscordClient {
    /// Create a new Discord client skeleton
    pub fn new(router: Arc<EventRouter>) -> Self {
        Self { router }
    }

    /// Simulate receiving a message from Discord
    pub async fn simulate_handle_message(
        &self,
        chat_id: String,
        user_id: String,
        message_id: String,
        content: String,
        is_group: bool,
    ) -> Result<()> {
        let event = crate::types::AppEvent::ChatMessage {
            platform: "discord".to_string(),
            chat_id,
            user_id,
            message_id,
            message_text: content,
            group_folder: "discord_data".to_string(),
            is_group,
        };

        // For testing/simulation, we dispatch it immediately
        let _ = self.router.dispatch(event).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::mock::MockRuntime;
    use crate::types::ContainerOutput;
    use tokio;

    #[tokio::test]
    async fn test_discord_client_dispatches_app_event() {
        let mock_output = ContainerOutput {
            status: "success".to_string(),
            result: Some("discord mock response".to_string()),
            new_session_id: None,
            error: None,
        };

        let mock_runtime = Arc::new(MockRuntime::new(mock_output));
        let router = Arc::new(EventRouter::new(mock_runtime.clone()));

        let client = DiscordClient::new(router);

        client
            .simulate_handle_message(
                "channel_123".to_string(),
                "user_456".to_string(),
                "msg_789".to_string(),
                "ping".to_string(),
                true,
            )
            .await
            .unwrap();

        let invocations = mock_runtime.invocations.lock().unwrap();
        assert_eq!(invocations.len(), 1);
        
        let req = &invocations[0];
        assert_eq!(req.prompt, "ping");
        assert_eq!(req.chat_jid, "channel_123");
        assert_eq!(req.group_folder, "discord_data");
        assert_eq!(req.session_id, Some("discord_msg_789".to_string()));
        // In EventRouter, is_main = !is_group. We passed is_group = true, so is_main = false
        assert_eq!(req.is_main, false);
    }
}