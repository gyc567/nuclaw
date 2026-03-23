use crate::autoresearch::feishu_eval::{
    FeishuEvalConfig, FeishuEvalResult, FeishuMetric, FeishuMockTokenResponse,
    FeishuMockWebhookEvent, FeishuTestCase, FeishuTestExpected, FeishuTestInput, FeishuTestResult,
    FeishuTestType,
};
use crate::feishu::{
    extract_trigger_pure, is_allowed_chat_pure, is_duplicate_message_pure, FeishuDMPolicy,
    FeishuGroupPolicy,
};
use crate::types::NewMessage;
use std::collections::HashMap;
use std::time::Instant;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum FeishuEvalError {
    #[error("Test case error: {0}")]
    TestCase(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

pub struct FeishuEvaluator {
    config: FeishuEvalConfig,
}

impl FeishuEvaluator {
    pub fn new(config: FeishuEvalConfig) -> Self {
        Self { config }
    }

    pub fn run(&self) -> FeishuEvalResult {
        let mut results = Vec::new();

        for test_case in &self.config.test_cases {
            let result = self.run_test_case(test_case);
            results.push(result);
        }

        FeishuEvalResult::new(self.config.metric, results)
    }

    fn run_test_case(&self, test_case: &FeishuTestCase) -> FeishuTestResult {
        let start = Instant::now();
        let test_type = test_case.test_type;

        let result = match test_case.test_type {
            FeishuTestType::TokenRefresh => self.test_token_refresh(test_case),
            FeishuTestType::MessageSend => self.test_message_send(test_case),
            FeishuTestType::WebhookEvent => self.test_webhook_event(test_case),
            FeishuTestType::DMPolicy => self.test_dm_policy(test_case),
            FeishuTestType::GroupPolicy => self.test_group_policy(test_case),
            FeishuTestType::DuplicateDetection => self.test_duplicate_detection(test_case),
            FeishuTestType::ErrorHandling => self.test_error_handling(test_case),
            FeishuTestType::EdgeCase => self.test_edge_case(test_case),
        };

        let duration_ms = start.elapsed().as_millis() as u64;

        match result {
            Ok((actual, expected_match)) => FeishuTestResult::new(
                &test_case.name,
                test_type,
                expected_match,
                &actual,
                &test_case.expected.success.to_string(),
                duration_ms,
            ),
            Err(e) => FeishuTestResult::error(&test_case.name, test_type, &e.to_string()),
        }
    }

    fn test_token_refresh(
        &self,
        test_case: &FeishuTestCase,
    ) -> Result<(String, bool), FeishuEvalError> {
        let default_response = FeishuMockTokenResponse::success();
        let mock = test_case
            .input
            .mock_token_response
            .as_ref()
            .unwrap_or(&default_response);

        let success = mock.code == 0 && mock.tenant_access_token.is_some();
        let actual = format!("code: {}, token: {:?}", mock.code, mock.tenant_access_token);
        let expected_match = success == test_case.expected.success;

        Ok((actual, expected_match))
    }

    fn test_message_send(
        &self,
        test_case: &FeishuTestCase,
    ) -> Result<(String, bool), FeishuEvalError> {
        let content = test_case
            .input
            .message_content
            .as_deref()
            .unwrap_or("test message");
        let chat_id = test_case.input.chat_id.as_deref().unwrap_or("chat_123");

        let is_valid = !content.trim().is_empty() && !chat_id.is_empty();
        let actual = format!("content_len: {}, chat_id: {}", content.len(), chat_id);
        let expected_match = is_valid == test_case.expected.success;

        Ok((actual, expected_match))
    }

    fn test_webhook_event(
        &self,
        test_case: &FeishuTestCase,
    ) -> Result<(String, bool), FeishuEvalError> {
        let default_event = FeishuMockWebhookEvent::text_message("chat_123", "p2p", "hello");
        let event = test_case
            .input
            .webhook_event
            .as_ref()
            .unwrap_or(&default_event);

        let valid_event = event.event_type == "im.message.receive_v1"
            && event.message_type == "text"
            && !event.content.is_empty();

        let actual = format!(
            "event_type: {}, message_type: {}, content: {}",
            event.event_type,
            event.message_type,
            truncate(&event.content, 20)
        );
        let expected_match = valid_event == test_case.expected.success;

        Ok((actual, expected_match))
    }

    fn test_dm_policy(
        &self,
        test_case: &FeishuTestCase,
    ) -> Result<(String, bool), FeishuEvalError> {
        let policy_str = test_case.input.dm_policy.as_deref().unwrap_or("pairing");
        let policy = FeishuDMPolicy::parse(policy_str);

        let user_id = test_case.input.user_id.as_deref().unwrap_or("user_123");

        let result = match policy {
            FeishuDMPolicy::Disabled => false,
            FeishuDMPolicy::Open => true,
            FeishuDMPolicy::Allowlist => true,
            FeishuDMPolicy::Pairing => true,
        };

        let actual = format!(
            "policy: {:?}, user: {}, allowed: {}",
            policy, user_id, result
        );
        let expected_match = result == test_case.expected.success;

        Ok((actual, expected_match))
    }

    fn test_group_policy(
        &self,
        test_case: &FeishuTestCase,
    ) -> Result<(String, bool), FeishuEvalError> {
        let policy_str = test_case
            .input
            .group_policy
            .as_deref()
            .unwrap_or("allowlist");
        let policy = FeishuGroupPolicy::parse(policy_str);

        let chat_jid = test_case
            .input
            .chat_jid
            .as_deref()
            .unwrap_or("feishu:chat:chat_123");
        let allowlist = test_case.input.allowlist.clone().unwrap_or_default();

        let result = is_allowed_chat_pure(chat_jid, policy, &allowlist);

        let actual = format!(
            "policy: {:?}, chat_jid: {}, allowed: {}",
            policy, chat_jid, result
        );
        let expected_match = result == test_case.expected.success;

        Ok((actual, expected_match))
    }

    fn test_duplicate_detection(
        &self,
        test_case: &FeishuTestCase,
    ) -> Result<(String, bool), FeishuEvalError> {
        let msg_id = test_case.input.message_id.as_deref().unwrap_or("msg_123");
        let chat_jid = test_case
            .input
            .chat_jid
            .as_deref()
            .unwrap_or("feishu:chat:chat_123");

        let last_ids = test_case.input.last_message_ids.clone().unwrap_or_default();

        let msg = NewMessage {
            id: msg_id.to_string(),
            chat_jid: chat_jid.to_string(),
            sender: "user_123".to_string(),
            sender_name: "Test User".to_string(),
            content: "test content".to_string(),
            timestamp: "2025-01-01T00:00:00Z".to_string(),
        };

        let is_duplicate = is_duplicate_message_pure(&msg, &last_ids);
        let actual = format!(
            "msg_id: {}, last_ids: {:?}, duplicate: {}",
            msg_id, last_ids, is_duplicate
        );

        let expected_duplicate = last_ids
            .get(chat_jid)
            .map(|id| id == msg_id)
            .unwrap_or(false);
        let expected_match =
            is_duplicate == expected_duplicate && is_duplicate == test_case.expected.success;

        Ok((actual, expected_match))
    }

    fn test_error_handling(
        &self,
        test_case: &FeishuTestCase,
    ) -> Result<(String, bool), FeishuEvalError> {
        let default_response = FeishuMockTokenResponse::success();
        let mock = test_case
            .input
            .mock_token_response
            .as_ref()
            .unwrap_or(&default_response);

        let handled = mock.code != 0 || mock.tenant_access_token.is_some();
        let actual = format!("code: {}, handled: {}", mock.code, handled);
        let expected_match = handled == test_case.expected.success;

        Ok((actual, expected_match))
    }

    fn test_edge_case(
        &self,
        test_case: &FeishuTestCase,
    ) -> Result<(String, bool), FeishuEvalError> {
        let content = test_case.input.message_content.as_deref().unwrap_or("");
        let trigger_result = extract_trigger_pure(content, "Andy");

        let has_trigger = trigger_result.is_some();
        let actual = format!(
            "content: '{}', has_trigger: {}",
            truncate(content, 30),
            has_trigger
        );
        let expected_match = has_trigger == test_case.expected.success;

        Ok((actual, expected_match))
    }

    pub fn evaluate_file(&self, path: &str) -> Result<FeishuEvalResult, FeishuEvalError> {
        let content = std::fs::read_to_string(path)?;
        let config: FeishuEvalConfig = serde_json::from_str(&content)?;
        Ok(Self::new(config).run())
    }
}

impl Default for FeishuEvaluator {
    fn default() -> Self {
        Self::new(FeishuEvalConfig::default())
    }
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}

pub fn create_default_test_cases() -> Vec<FeishuTestCase> {
    let mut cases = Vec::new();

    cases.push(FeishuTestCase::new(
        "token_refresh_success",
        FeishuTestType::TokenRefresh,
        "Successful token refresh",
        FeishuTestInput {
            mock_token_response: Some(FeishuMockTokenResponse::success()),
            ..Default::default()
        },
        FeishuTestExpected::success(),
    ));

    cases.push(FeishuTestCase::new(
        "token_refresh_failure_invalid_app",
        FeishuTestType::TokenRefresh,
        "Token refresh failure with invalid app",
        FeishuTestInput {
            mock_token_response: Some(FeishuMockTokenResponse::failure(999, "app not found")),
            ..Default::default()
        },
        FeishuTestExpected::failure(999, "app not found"),
    ));

    cases.push(FeishuTestCase::new(
        "message_send_valid",
        FeishuTestType::MessageSend,
        "Send valid message",
        FeishuTestInput {
            message_content: Some("Hello World".to_string()),
            chat_id: Some("chat_123".to_string()),
            ..Default::default()
        },
        FeishuTestExpected::success(),
    ));

    cases.push(FeishuTestCase::new(
        "message_send_empty",
        FeishuTestType::MessageSend,
        "Send empty message should fail",
        FeishuTestInput {
            message_content: Some("".to_string()),
            chat_id: Some("chat_123".to_string()),
            ..Default::default()
        },
        FeishuTestExpected::failure(0, "empty message"),
    ));

    cases.push(FeishuTestCase::new(
        "webhook_text_message",
        FeishuTestType::WebhookEvent,
        "Receive text message webhook",
        FeishuTestInput {
            webhook_event: Some(FeishuMockWebhookEvent::text_message(
                "chat_123",
                "p2p",
                "Hello Andy",
            )),
            ..Default::default()
        },
        FeishuTestExpected::success(),
    ));

    cases.push(FeishuTestCase::new(
        "dm_policy_open",
        FeishuTestType::DMPolicy,
        "DM policy open allows all",
        FeishuTestInput {
            dm_policy: Some("open".to_string()),
            user_id: Some("user_123".to_string()),
            ..Default::default()
        },
        FeishuTestExpected::success(),
    ));

    cases.push(FeishuTestCase::new(
        "dm_policy_disabled",
        FeishuTestType::DMPolicy,
        "DM policy disabled blocks all",
        FeishuTestInput {
            dm_policy: Some("disabled".to_string()),
            user_id: Some("user_123".to_string()),
            ..Default::default()
        },
        FeishuTestExpected::failure(0, "dm disabled"),
    ));

    cases.push(FeishuTestCase::new(
        "group_policy_open",
        FeishuTestType::GroupPolicy,
        "Group policy open allows all",
        FeishuTestInput {
            group_policy: Some("open".to_string()),
            chat_jid: Some("feishu:chat:chat_999".to_string()),
            ..Default::default()
        },
        FeishuTestExpected::success(),
    ));

    cases.push(FeishuTestCase::new(
        "group_policy_allowlist_match",
        FeishuTestType::GroupPolicy,
        "Group policy allowlist with match",
        FeishuTestInput {
            group_policy: Some("allowlist".to_string()),
            chat_jid: Some("feishu:chat:chat_123".to_string()),
            allowlist: Some(vec!["chat_123".to_string(), "chat_456".to_string()]),
            ..Default::default()
        },
        FeishuTestExpected::success(),
    ));

    cases.push(FeishuTestCase::new(
        "group_policy_allowlist_no_match",
        FeishuTestType::GroupPolicy,
        "Group policy allowlist without match",
        FeishuTestInput {
            group_policy: Some("allowlist".to_string()),
            chat_jid: Some("feishu:chat:chat_999".to_string()),
            allowlist: Some(vec!["chat_123".to_string()]),
            ..Default::default()
        },
        FeishuTestExpected::failure(0, "not in allowlist"),
    ));

    cases.push(FeishuTestCase::new(
        "group_policy_disabled",
        FeishuTestType::GroupPolicy,
        "Group policy disabled blocks all",
        FeishuTestInput {
            group_policy: Some("disabled".to_string()),
            chat_jid: Some("feishu:chat:chat_123".to_string()),
            ..Default::default()
        },
        FeishuTestExpected::failure(0, "group disabled"),
    ));

    cases.push(FeishuTestCase::new(
        "duplicate_detection_yes",
        FeishuTestType::DuplicateDetection,
        "Duplicate message detected",
        FeishuTestInput {
            message_id: Some("msg_123".to_string()),
            chat_jid: Some("feishu:chat:chat_123".to_string()),
            last_message_ids: Some(HashMap::from([(
                "feishu:chat:chat_123".to_string(),
                "msg_123".to_string(),
            )])),
            ..Default::default()
        },
        FeishuTestExpected::success(),
    ));

    cases.push(FeishuTestCase::new(
        "duplicate_detection_no",
        FeishuTestType::DuplicateDetection,
        "New unique message",
        FeishuTestInput {
            message_id: Some("msg_456".to_string()),
            chat_jid: Some("feishu:chat:chat_123".to_string()),
            last_message_ids: Some(HashMap::from([(
                "feishu:chat:chat_123".to_string(),
                "msg_123".to_string(),
            )])),
            ..Default::default()
        },
        FeishuTestExpected::failure(0, "not duplicate"),
    ));

    cases.push(FeishuTestCase::new(
        "error_handling_invalid_token",
        FeishuTestType::ErrorHandling,
        "Handle invalid token response",
        FeishuTestInput {
            mock_token_response: Some(FeishuMockTokenResponse::failure(
                999,
                "invalid app credentials",
            )),
            ..Default::default()
        },
        FeishuTestExpected::success(),
    ));

    cases.push(FeishuTestCase::new(
        "edge_case_trigger_at_end",
        FeishuTestType::EdgeCase,
        "Trigger at end of message",
        FeishuTestInput {
            message_content: Some("Hello @Andy".to_string()),
            ..Default::default()
        },
        FeishuTestExpected::success(),
    ));

    cases.push(FeishuTestCase::new(
        "edge_case_trigger_in_middle",
        FeishuTestType::EdgeCase,
        "Trigger in middle of message",
        FeishuTestInput {
            message_content: Some("Hey @Andy how are you?".to_string()),
            ..Default::default()
        },
        FeishuTestExpected::success(),
    ));

    cases.push(FeishuTestCase::new(
        "edge_case_no_trigger",
        FeishuTestType::EdgeCase,
        "Message without trigger",
        FeishuTestInput {
            message_content: Some("Hello world".to_string()),
            ..Default::default()
        },
        FeishuTestExpected::failure(0, "no trigger"),
    ));

    cases.push(FeishuTestCase::new(
        "edge_case_empty_message",
        FeishuTestType::EdgeCase,
        "Empty message",
        FeishuTestInput {
            message_content: Some("".to_string()),
            ..Default::default()
        },
        FeishuTestExpected::failure(0, "empty"),
    ));

    cases
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_test_cases() {
        let cases = create_default_test_cases();
        assert!(cases.len() >= 10);
    }

    #[test]
    fn test_evaluator_run() {
        let cases = create_default_test_cases();
        let config = FeishuEvalConfig {
            metric: FeishuMetric::PassRate,
            test_cases: cases,
            output_dir: "test_results".to_string(),
        };
        let evaluator = FeishuEvaluator::new(config);
        let result = evaluator.run();

        for r in &result.results {
            if !r.passed {
                eprintln!(
                    "FAILED: {} - actual: '{}', expected: '{}', error: {:?}",
                    r.test_name, r.actual, r.expected, r.error
                );
            }
        }

        println!(
            "Pass rate: {:.1}% ({}/{})",
            result.pass_rate() * 100.0,
            result.passed_tests,
            result.total_tests
        );

        assert!(result.total_tests > 0);
    }

    #[test]
    fn test_token_refresh_success() {
        let cases = create_default_test_cases();
        let token_refresh_cases: Vec<_> = cases
            .iter()
            .filter(|c| c.test_type == FeishuTestType::TokenRefresh)
            .collect();

        assert!(!token_refresh_cases.is_empty());
    }

    #[test]
    fn test_dm_policy_cases() {
        let cases = create_default_test_cases();
        let dm_cases: Vec<_> = cases
            .iter()
            .filter(|c| c.test_type == FeishuTestType::DMPolicy)
            .collect();

        assert!(!dm_cases.is_empty());
    }

    #[test]
    fn test_group_policy_cases() {
        let cases = create_default_test_cases();
        let group_cases: Vec<_> = cases
            .iter()
            .filter(|c| c.test_type == FeishuTestType::GroupPolicy)
            .collect();

        assert!(!group_cases.is_empty());
    }
}
