use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FeishuMetric {
    TokenRefreshRate,
    MessageDeliveryRate,
    WebhookProcessingRate,
    PolicyAccuracy,
    PassRate,
}

impl FeishuMetric {
    pub fn name(&self) -> &str {
        match self {
            FeishuMetric::TokenRefreshRate => "token_refresh_rate",
            FeishuMetric::MessageDeliveryRate => "message_delivery_rate",
            FeishuMetric::WebhookProcessingRate => "webhook_processing_rate",
            FeishuMetric::PolicyAccuracy => "policy_accuracy",
            FeishuMetric::PassRate => "pass_rate",
        }
    }

    pub fn lower_is_better(&self) -> bool {
        false
    }
}

impl Default for FeishuMetric {
    fn default() -> Self {
        FeishuMetric::PassRate
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FeishuTestType {
    TokenRefresh,
    MessageSend,
    WebhookEvent,
    DMPolicy,
    GroupPolicy,
    DuplicateDetection,
    ErrorHandling,
    EdgeCase,
}

impl FeishuTestType {
    pub fn name(&self) -> &str {
        match self {
            FeishuTestType::TokenRefresh => "token_refresh",
            FeishuTestType::MessageSend => "message_send",
            FeishuTestType::WebhookEvent => "webhook_event",
            FeishuTestType::DMPolicy => "dm_policy",
            FeishuTestType::GroupPolicy => "group_policy",
            FeishuTestType::DuplicateDetection => "duplicate_detection",
            FeishuTestType::ErrorHandling => "error_handling",
            FeishuTestType::EdgeCase => "edge_case",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeishuTestCase {
    pub name: String,
    pub test_type: FeishuTestType,
    pub description: String,
    pub input: FeishuTestInput,
    pub expected: FeishuTestExpected,
}

impl FeishuTestCase {
    pub fn new(
        name: &str,
        test_type: FeishuTestType,
        description: &str,
        input: FeishuTestInput,
        expected: FeishuTestExpected,
    ) -> Self {
        Self {
            name: name.to_string(),
            test_type,
            description: description.to_string(),
            input,
            expected,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeishuTestInput {
    pub app_id: Option<String>,
    pub app_secret: Option<String>,
    pub mock_token_response: Option<FeishuMockTokenResponse>,
    pub message_content: Option<String>,
    pub chat_id: Option<String>,
    pub chat_type: Option<String>,
    pub webhook_event: Option<FeishuMockWebhookEvent>,
    pub dm_policy: Option<String>,
    pub group_policy: Option<String>,
    pub user_id: Option<String>,
    pub chat_jid: Option<String>,
    pub allowlist: Option<Vec<String>>,
    pub last_message_ids: Option<HashMap<String, String>>,
    pub message_id: Option<String>,
}

impl Default for FeishuTestInput {
    fn default() -> Self {
        Self {
            app_id: None,
            app_secret: None,
            mock_token_response: None,
            message_content: None,
            chat_id: None,
            chat_type: None,
            webhook_event: None,
            dm_policy: None,
            group_policy: None,
            user_id: None,
            chat_jid: None,
            allowlist: None,
            last_message_ids: None,
            message_id: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeishuTestExpected {
    pub success: bool,
    pub error_code: Option<i32>,
    pub error_message: Option<String>,
    pub response_contains: Option<String>,
}

impl FeishuTestExpected {
    pub fn success() -> Self {
        Self {
            success: true,
            error_code: None,
            error_message: None,
            response_contains: None,
        }
    }

    pub fn failure(error_code: i32, error_message: &str) -> Self {
        Self {
            success: false,
            error_code: Some(error_code),
            error_message: Some(error_message.to_string()),
            response_contains: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeishuMockTokenResponse {
    pub code: i32,
    pub msg: String,
    pub tenant_access_token: Option<String>,
    pub expire: Option<i32>,
}

impl FeishuMockTokenResponse {
    pub fn success() -> Self {
        Self {
            code: 0,
            msg: "success".to_string(),
            tenant_access_token: Some("mock_token_123".to_string()),
            expire: Some(7200),
        }
    }

    pub fn failure(code: i32, msg: &str) -> Self {
        Self {
            code,
            msg: msg.to_string(),
            tenant_access_token: None,
            expire: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeishuMockWebhookEvent {
    pub schema: String,
    pub event_type: String,
    pub message_type: String,
    pub chat_id: String,
    pub chat_type: String,
    pub sender_id: String,
    pub content: String,
    pub message_id: String,
}

impl FeishuMockWebhookEvent {
    pub fn text_message(chat_id: &str, chat_type: &str, content: &str) -> Self {
        Self {
            schema: "2.0".to_string(),
            event_type: "im.message.receive_v1".to_string(),
            message_type: "text".to_string(),
            chat_id: chat_id.to_string(),
            chat_type: chat_type.to_string(),
            sender_id: "user_123".to_string(),
            content: content.to_string(),
            message_id: format!("msg_{}", chrono::Utc::now().timestamp_millis()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeishuEvalConfig {
    pub metric: FeishuMetric,
    pub test_cases: Vec<FeishuTestCase>,
    pub output_dir: String,
}

impl Default for FeishuEvalConfig {
    fn default() -> Self {
        Self {
            metric: FeishuMetric::default(),
            test_cases: Vec::new(),
            output_dir: "feishu_eval_results".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeishuTestResult {
    pub test_name: String,
    pub test_type: FeishuTestType,
    pub passed: bool,
    pub actual: String,
    pub expected: String,
    pub duration_ms: u64,
    pub error: Option<String>,
}

impl FeishuTestResult {
    pub fn new(
        test_name: &str,
        test_type: FeishuTestType,
        passed: bool,
        actual: &str,
        expected: &str,
        duration_ms: u64,
    ) -> Self {
        Self {
            test_name: test_name.to_string(),
            test_type,
            passed,
            actual: actual.to_string(),
            expected: expected.to_string(),
            duration_ms,
            error: None,
        }
    }

    pub fn error(test_name: &str, test_type: FeishuTestType, error_msg: &str) -> Self {
        Self {
            test_name: test_name.to_string(),
            test_type,
            passed: false,
            actual: String::new(),
            expected: String::new(),
            duration_ms: 0,
            error: Some(error_msg.to_string()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeishuEvalResult {
    pub metric: FeishuMetric,
    pub metric_value: f64,
    pub total_tests: usize,
    pub passed_tests: usize,
    pub failed_tests: usize,
    pub results: Vec<FeishuTestResult>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl FeishuEvalResult {
    pub fn new(metric: FeishuMetric, results: Vec<FeishuTestResult>) -> Self {
        let passed_tests = results.iter().filter(|r| r.passed).count();
        let total_tests = results.len();
        let metric_value = if total_tests > 0 {
            (passed_tests as f64) / (total_tests as f64)
        } else {
            0.0
        };

        Self {
            metric,
            metric_value,
            total_tests,
            passed_tests,
            failed_tests: total_tests - passed_tests,
            results,
            timestamp: chrono::Utc::now(),
        }
    }

    pub fn pass_rate(&self) -> f64 {
        self.metric_value
    }

    pub fn is_success(&self) -> bool {
        self.pass_rate() >= 1.0
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FeishuEvalHistory {
    pub results: Vec<FeishuEvalResult>,
}

impl FeishuEvalHistory {
    pub fn push(&mut self, result: FeishuEvalResult) {
        self.results.push(result);
    }

    pub fn latest(&self) -> Option<&FeishuEvalResult> {
        self.results.last()
    }

    pub fn best(&self) -> Option<&FeishuEvalResult> {
        self.results
            .iter()
            .max_by(|a, b| a.metric_value.partial_cmp(&b.metric_value).unwrap())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metric_default() {
        let metric = FeishuMetric::default();
        assert_eq!(metric.name(), "pass_rate");
    }

    #[test]
    fn test_test_type_names() {
        assert_eq!(FeishuTestType::TokenRefresh.name(), "token_refresh");
        assert_eq!(FeishuTestType::MessageSend.name(), "message_send");
        assert_eq!(FeishuTestType::WebhookEvent.name(), "webhook_event");
    }

    #[test]
    fn test_mock_token_response() {
        let success = FeishuMockTokenResponse::success();
        assert_eq!(success.code, 0);
        assert!(success.tenant_access_token.is_some());

        let failure = FeishuMockTokenResponse::failure(999, "test error");
        assert_eq!(failure.code, 999);
        assert!(failure.tenant_access_token.is_none());
    }

    #[test]
    fn test_mock_webhook_event() {
        let event = FeishuMockWebhookEvent::text_message("chat_123", "p2p", "Hello");
        assert_eq!(event.message_type, "text");
        assert_eq!(event.chat_id, "chat_123");
    }

    #[test]
    fn test_eval_result() {
        let results = vec![
            FeishuTestResult::new("test1", FeishuTestType::TokenRefresh, true, "ok", "ok", 10),
            FeishuTestResult::new("test2", FeishuTestType::MessageSend, true, "ok", "ok", 5),
        ];
        let eval = FeishuEvalResult::new(FeishuMetric::PassRate, results);
        assert_eq!(eval.pass_rate(), 1.0);
        assert_eq!(eval.passed_tests, 2);
    }
}
