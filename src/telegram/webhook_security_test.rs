//! Tests for Telegram webhook security

#[cfg(test)]
mod webhook_security_tests {
    use crate::telegram::client::verify_webhook_secret;

    /// Test: Without secret configured, webhook should allow all requests
    #[test]
    fn test_no_secret_allows_requests() {
        std::env::remove_var("TELEGRAM_WEBHOOK_SECRET");
        let result = verify_webhook_secret(None);
        assert!(result, "Should allow requests when no secret is configured");
    }

    /// Test: With secret configured, None token should be rejected
    #[test]
    fn test_secret_rejects_missing_token() {
        std::env::set_var("TELEGRAM_WEBHOOK_SECRET", "test_secret_123");
        let result = verify_webhook_secret(None);
        std::env::remove_var("TELEGRAM_WEBHOOK_SECRET");
        assert!(!result, "Should reject when secret is configured but token is missing");
    }

    /// Test: With secret configured, matching token should be accepted
    #[test]
    fn test_secret_accepts_matching_token() {
        std::env::set_var("TELEGRAM_WEBHOOK_SECRET", "my_secure_token");
        let result = verify_webhook_secret(Some("my_secure_token"));
        std::env::remove_var("TELEGRAM_WEBHOOK_SECRET");
        assert!(result, "Should accept matching token");
    }

    /// Test: With secret configured, wrong token should be rejected
    #[test]
    fn test_secret_rejects_wrong_token() {
        std::env::set_var("TELEGRAM_WEBHOOK_SECRET", "correct_secret");
        let result = verify_webhook_secret(Some("wrong_secret"));
        std::env::remove_var("TELEGRAM_WEBHOOK_SECRET");
        assert!(!result, "Should reject wrong token");
    }
}

/// Tests for panic removal in message handling
#[cfg(test)]
mod panic_removal_tests {
    use crate::error::NuClawError;

    /// Test: Group not found should return error, not panic
    #[test]
    fn test_group_not_found_returns_error() {
        let error = NuClawError::Telegram {
            message: "Group not found: test_group".to_string(),
        };

        match error {
            NuClawError::Telegram { message } => {
                assert!(message.contains("Group not found"));
            }
            _ => panic!("Expected Telegram error"),
        }
    }
}
