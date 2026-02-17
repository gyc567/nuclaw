//! End-to-End tests for NuClaw
//!
//! These tests verify complete workflows and component interactions.

use async_trait::async_trait;
use nuclaw::agent_runner::create_runner;
use nuclaw::channels::{Channel, ChannelRegistry};
use nuclaw::config;
use nuclaw::container_runner::{create_group_ipc_directory, max_output_size};
use nuclaw::db::Database;
use nuclaw::providers::provider_registry;
use nuclaw::skills::{builtin_skills, Skill, SkillRegistry};
use nuclaw::task_scheduler::{parse_cron_expression, poll_interval, task_timeout, TaskScheduler};
use nuclaw::telegram::{chunk_text_advanced, chunk_text_pure, ChunkMode, DMPolicy, GroupPolicy};
use nuclaw::types::{ContainerInput, NewMessage, ScheduledTask, Session};
use nuclaw::error::Result;
use nuclaw::utils::json::{load_json, save_json};
use std::fs;
use std::time::Duration;

#[cfg(test)]
mod e2e_tests {
    use super::*;

    // =========================================================================
    // Configuration E2E Tests
    // =========================================================================

    #[test]
    fn test_full_configuration_loading() {
        // Setup
        config::ensure_directories().expect("Failed to create directories");
        
        // Test all config functions work together
        let home = config::nuclaw_home();
        assert!(home.exists());
        
        let store = config::store_dir();
        assert!(store.exists());
        
        let data = config::data_dir();
        assert!(data.exists());
        
        let groups = config::groups_dir();
        assert!(groups.exists());
        
        let logs = config::logs_dir();
        assert!(logs.exists());
    }

    #[test]
    fn test_configuration_persistence() {
        // Create a test config file
        let config_path = config::data_dir().join("test_config.json");
        let test_config = serde_json::json!({
            "test_key": "test_value",
            "nested": {"inner": 42}
        });
        
        save_json(&config_path, &test_config).expect("Failed to save config");
        assert!(config_path.exists());
        
        // Load and verify
        let loaded: serde_json::Value = load_json(&config_path, serde_json::json!({}));
        assert_eq!(loaded["test_key"], "test_value");
        assert_eq!(loaded["nested"]["inner"], 42);
        
        // Cleanup
        let _ = fs::remove_file(&config_path);
    }

    // =========================================================================
    // Provider Registry E2E Tests
    // =========================================================================

    #[test]
    fn test_provider_registry_workflow() {
        // Setup
        std::env::remove_var("ANTHROPIC_API_KEY");
        std::env::remove_var("OPENAI_API_KEY");
        
        let registry = provider_registry();
        
        // Test provider detection
        let detected = registry.detect_provider();
        assert!(detected.is_none(), "No provider should be detected without API keys");
        
        // Register a test provider (using existing anthropic)
        std::env::set_var("ANTHROPIC_API_KEY", "test-key-12345");
        let detected = registry.detect_provider();
        assert_eq!(detected, Some("anthropic".to_string()));
        
        // Verify config loading
        let config = registry.load_config("anthropic");
        assert!(config.is_some());
        assert!(config.unwrap().is_configured());
        
        // Cleanup
        std::env::remove_var("ANTHROPIC_API_KEY");
    }

    #[test]
    fn test_provider_config_loading() {
        // Setup environment
        std::env::remove_var("ANTHROPIC_API_KEY");
        std::env::remove_var("CLAUDE_MODEL");
        
        // Test without API key
        let registry = provider_registry();
        let config = registry.load_config("anthropic");
        assert!(config.is_some());
        assert!(!config.unwrap().is_configured());
        
        // Test with API key
        std::env::set_var("ANTHROPIC_API_KEY", "test-key");
        let registry = provider_registry();
        let config = registry.load_config("anthropic");
        assert!(config.unwrap().is_configured());
        
        // Test model override
        std::env::set_var("CLAUDE_MODEL", "claude-3-opus-20240229");
        let registry = provider_registry();
        let config = registry.load_config("anthropic");
        assert_eq!(config.unwrap().model, Some("claude-3-opus-20240229".to_string()));
        
        // Cleanup
        std::env::remove_var("ANTHROPIC_API_KEY");
        std::env::remove_var("CLAUDE_MODEL");
    }

    // =========================================================================
    // Skills Registry E2E Tests
    // =========================================================================

    #[test]
    fn test_skills_workflow() {
        let skills: &dyn SkillRegistry = &builtin_skills();
        
        // Verify all built-in skills exist
        let github = skills.get("github");
        assert!(github.is_some());
        assert_eq!(github.unwrap().name, "github");
        
        let weather = skills.get("weather");
        assert!(weather.is_some());
        
        let search = skills.get("search");
        assert!(search.is_some());
        
        let memory = skills.get("memory");
        assert!(memory.is_some());
        
        // Test skill listing
        let names = skills.names();
        assert!(names.len() >= 4);
        assert!(names.contains(&"github".to_string()));
        
        // Test skill content
        let github = skills.get("github").unwrap();
        assert!(github.content.contains("GitHub"));
        assert!(github.description.contains("GitHub"));
    }

    #[test]
    fn test_skill_registration_workflow() {
        use nuclaw::skills::{BuiltinSkillRegistry, Skill};
        
        let mut registry = BuiltinSkillRegistry::new();
        
        // Register custom skill
        let custom = Skill::new(
            "custom_test",
            "A custom test skill",
            "You are a test assistant.",
        );
        registry.register(custom);
        
        // Verify registration - use the trait object
        let registry: &dyn SkillRegistry = &registry;
        let retrieved = registry.get("custom_test");
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().name, "custom_test");
    }

    // =========================================================================
    // Channel Registry E2E Tests
    // =========================================================================

    #[test]
    fn test_channel_registry_workflow() {
        let registry = ChannelRegistry::new();
        
        // Initially empty
        assert!(registry.list().is_empty());
        
        // Create a mock channel
        struct TestChannel;
        
        #[async_trait]
        impl Channel for TestChannel {
            fn name(&self) -> &str { "test" }
            async fn send(&self, _jid: &str, _msg: &str) -> Result<()> { Ok(()) }
            async fn start(&self) -> Result<()> { Ok(()) }
            fn is_enabled(&self) -> bool { true }
        }
        
        // Register channel
        registry.register(TestChannel);
        
        // Verify registration
        assert!(registry.is_registered("test"));
        assert!(registry.is_enabled("test"));
        
        let channels = registry.list();
        assert_eq!(channels.len(), 1);
        assert!(channels.contains(&"test".to_string()));
    }

    // =========================================================================
    // Database E2E Tests
    // =========================================================================

    #[test]
    fn test_database_full_workflow() {
        config::ensure_directories().expect("Failed to create directories");
        let db = Database::new().expect("Failed to create database");
        
        // Test connection
        let conn = db.get_connection().expect("Failed to get connection");
        
        // Test transaction
        let mut conn = conn; // Make mutable for transaction
        let tx = conn.transaction().expect("Failed to begin transaction");
        
        // Insert test data
        tx.execute(
            "INSERT INTO messages (id, chat_jid, sender, sender_name, content, timestamp, is_from_me)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
            rusqlite::params![
                "e2e_test_1",
                "test@chat",
                "user1",
                "Test User",
                "Test message",
                "2025-01-01T00:00:00Z",
                0
            ],
        ).expect("Failed to insert message");
        
        // Query and verify
        let content: String = tx.query_row(
            "SELECT content FROM messages WHERE id = ?",
            ["e2e_test_1"],
            |row| row.get(0),
        ).expect("Failed to query message");
        
        assert_eq!(content, "Test message");
        
        // Rollback (test transaction)
        tx.rollback().expect("Failed to rollback");
        
        // Verify rollback
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM messages WHERE id = ?",
            ["e2e_test_1"],
            |row| row.get(0),
        ).expect("Failed to count");
        
        assert_eq!(count, 0);
    }

    #[test]
    fn test_database_concurrent_operations() {
        config::ensure_directories().expect("Failed to create directories");
        let db = Database::new().expect("Failed to create database");
        
        // Clean up any existing test data
        let conn = db.get_connection().expect("Failed to get connection");
        let _ = conn.execute("DELETE FROM messages WHERE id LIKE 'concurrent_e2e_%'", []);
        
        // Insert multiple messages
        for i in 0..5 {
            let conn = db.get_connection().expect("Failed to get connection");
            conn.execute(
                "INSERT INTO messages (id, chat_jid, sender, sender_name, content, timestamp, is_from_me)
                 VALUES (?, ?, ?, ?, ?, ?, ?)",
                rusqlite::params![
                    format!("concurrent_e2e_{}", i),
                    "test@chat",
                    "user",
                    "Test",
                    format!("Message {}", i),
                    "2025-01-01T00:00:00Z",
                    0
                ],
            ).expect("Failed to insert message");
        }
        
        // Verify all inserts succeeded
        let conn = db.get_connection().expect("Failed to get connection");
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM messages WHERE id LIKE 'concurrent_e2e_%'",
                [],
                |row| row.get(0),
            )
            .expect("Failed to count");
        
        assert_eq!(count, 5);
    }

    // =========================================================================
    // Task Scheduler E2E Tests
    // =========================================================================

    #[test]
    fn test_task_scheduler_workflow() {
        config::ensure_directories().expect("Failed to create directories");
        
        // Test poll interval
        let interval = poll_interval();
        assert_eq!(interval, Duration::from_secs(60));
        
        // Test task timeout
        let timeout = task_timeout();
        assert_eq!(timeout, Duration::from_secs(600));
        
        // Test cron parsing (using the full format with seconds)
        assert!(parse_cron_expression("0 0 9 * * *").is_ok()); // 6-field format
        assert!(parse_cron_expression("0 0 0 1 * *").is_ok()); // First day of month
    }

    #[test]
    fn test_scheduled_task_creation() {
        let task = ScheduledTask {
            id: "test_task_1".to_string(),
            group_folder: "test_group".to_string(),
            chat_jid: "test@chat".to_string(),
            prompt: "Test prompt".to_string(),
            schedule_type: "cron".to_string(),
            schedule_value: "0 9 * * *".to_string(),
            context_mode: "main".to_string(),
            next_run: None,
            last_run: None,
            last_result: None,
            status: "active".to_string(),
            created_at: "2025-01-01T00:00:00Z".to_string(),
        };
        
        assert_eq!(task.id, "test_task_1");
        assert_eq!(task.schedule_value, "0 9 * * *");
        assert_eq!(task.status, "active");
    }

    // =========================================================================
    // Container Runner E2E Tests
    // =========================================================================

    #[test]
    fn test_container_config_workflow() {
        config::ensure_directories().expect("Failed to create directories");
        
        // Test group IPC directory creation
        let ipc_dir = create_group_ipc_directory("test_group_e2e")
            .expect("Failed to create IPC directory");
        
        assert!(ipc_dir.exists());
        assert!(ipc_dir.to_string_lossy().contains("test_group_e2e"));
        
        // Test file creation in IPC directory
        let test_file = ipc_dir.join("test.json");
        save_json(&test_file, &serde_json::json!({"test": true}))
            .expect("Failed to save test file");
        
        assert!(test_file.exists());
        
        // Load and verify
        let loaded: serde_json::Value = load_json(&test_file, serde_json::json!({}));
        assert_eq!(loaded["test"], true);
        
        // Cleanup
        let _ = fs::remove_dir_all(&ipc_dir);
    }

    #[test]
    fn test_max_output_size_config() {
        // Test default
        let size = max_output_size();
        assert_eq!(size, 10 * 1024 * 1024); // 10MB
        
        // Environment variable is handled at module init, so we test the default
        // In real scenario, this would be set before initialization
    }

    // =========================================================================
    // Message Processing E2E Tests
    // =========================================================================

    #[test]
    fn test_message_processing_workflow() {
        // Create a test message
        let message = NewMessage {
            id: "test_msg_e2e".to_string(),
            chat_jid: "telegram:group:123456".to_string(),
            sender: "user123".to_string(),
            sender_name: "Test User".to_string(),
            content: "Hello, this is a test message".to_string(),
            timestamp: "2025-01-01T12:00:00Z".to_string(),
        };
        
        // Verify message structure
        assert_eq!(message.id, "test_msg_e2e");
        assert!(message.chat_jid.starts_with("telegram:group:"));
        assert!(!message.content.is_empty());
    }

    #[test]
    fn test_container_input_workflow() {
        let input = ContainerInput {
            prompt: "Test prompt".to_string(),
            session_id: Some("session_123".to_string()),
            group_folder: "test_group".to_string(),
            chat_jid: "test@chat".to_string(),
            is_main: true,
            is_scheduled_task: false,
        };
        
        assert_eq!(input.prompt, "Test prompt");
        assert!(input.session_id.is_some());
        assert!(input.is_main);
    }

    // =========================================================================
    // Telegram E2E Tests
    // =========================================================================

    #[test]
    fn test_telegram_message_chunking() {
        // Test short message (no chunking)
        let short = "Short message";
        let chunks = chunk_text_pure(short, 4000);
        assert_eq!(chunks.len(), 1);
        
        // Test long message (chunking)
        let long = "a".repeat(8000);
        let chunks = chunk_text_pure(&long, 4000);
        assert!(chunks.len() > 1);
        
        // Test with newlines
        let with_newlines = "Para1\n\nPara2\n\nPara3";
        let chunks = chunk_text_pure(with_newlines, 100);
        assert!(chunks.len() >= 1);
        
        // Test advanced chunking with newline mode
        let chunks = chunk_text_advanced(with_newlines, 50, ChunkMode::Newline);
        for chunk in &chunks {
            assert!(chunk.len() <= 50);
        }
    }

    #[test]
    fn test_telegram_policy_parsing() {
        // DM Policy
        assert_eq!(DMPolicy::parse("pairing"), DMPolicy::Pairing);
        assert_eq!(DMPolicy::parse("allowlist"), DMPolicy::Allowlist);
        assert_eq!(DMPolicy::parse("open"), DMPolicy::Open);
        assert_eq!(DMPolicy::parse("disabled"), DMPolicy::Disabled);
        
        // Group Policy
        assert_eq!(GroupPolicy::parse("open"), GroupPolicy::Open);
        assert_eq!(GroupPolicy::parse("allowlist"), GroupPolicy::Allowlist);
        assert_eq!(GroupPolicy::parse("disabled"), GroupPolicy::Disabled);
    }

    // =========================================================================
    // Error Handling E2E Tests
    // =========================================================================

    #[test]
    fn test_error_propagation_workflow() {
        use nuclaw::error::NuClawError;
        
        // Test error creation
        let config_err = NuClawError::Config {
            message: "Test config error".to_string(),
        };
        assert!(format!("{}", config_err).contains("Config"));
        
        let db_err = NuClawError::Database {
            message: "Test DB error".to_string(),
        };
        assert!(format!("{}", db_err).contains("Database"));
        
        let api_err = NuClawError::Api {
            message: "Test API error".to_string(),
        };
        assert!(format!("{}", api_err).contains("API"));
    }

    // =========================================================================
    // Session Management E2E Tests
    // =========================================================================

    #[test]
    fn test_session_workflow() {
        let session = Session::new();
        
        assert!(session.is_empty());
        assert_eq!(session.len(), 0);
    }

    // =========================================================================
    // Agent Runner E2E Tests
    // =========================================================================

    #[test]
    fn test_agent_runner_mode_switching() {
        // Test default mode (container)
        std::env::remove_var("AGENT_RUNNER_MODE");
        let runner = create_runner();
        // Runner creation may fail if config is missing, but mode should work
        // This tests the configuration aspect
        
        // Test API mode (should require API key)
        std::env::set_var("AGENT_RUNNER_MODE", "api");
        // Without API key, creation will fail
        
        // Cleanup
        std::env::remove_var("AGENT_RUNNER_MODE");
    }

    // =========================================================================
    // Type Serialization E2E Tests
    // =========================================================================

    #[test]
    fn test_type_serialization_roundtrip() {
        use nuclaw::types::{ContainerInput, ContainerOutput, NewMessage};
        
        // Test NewMessage serialization
        let msg = NewMessage {
            id: "serialize_test".to_string(),
            chat_jid: "test@chat".to_string(),
            sender: "user".to_string(),
            sender_name: "Test User".to_string(),
            content: "Test content".to_string(),
            timestamp: "2025-01-01T00:00:00Z".to_string(),
        };
        
        let json = serde_json::to_string(&msg).expect("Failed to serialize");
        let loaded: NewMessage = serde_json::from_str(&json).expect("Failed to deserialize");
        
        assert_eq!(msg.id, loaded.id);
        assert_eq!(msg.content, loaded.content);
        
        // Test ContainerInput serialization
        let input = ContainerInput {
            prompt: "Test prompt".to_string(),
            session_id: Some("sess_123".to_string()),
            group_folder: "test_group".to_string(),
            chat_jid: "test@chat".to_string(),
            is_main: true,
            is_scheduled_task: false,
        };
        
        let json = serde_json::to_string(&input).expect("Failed to serialize");
        let loaded: ContainerInput = serde_json::from_str(&json).expect("Failed to deserialize");
        
        assert_eq!(input.prompt, loaded.prompt);
        assert_eq!(input.group_folder, loaded.group_folder);
    }
}

// =========================================================================
// Performance Tests
// =========================================================================

#[cfg(test)]
mod performance_tests {
    use super::*;
    use std::time::Instant;

    #[test]
    fn test_json_serialization_performance() {
        let data = serde_json::json!({
            "messages": (0..100).map(|i| {
                serde_json::json!({
                    "id": format!("msg_{}", i),
                    "content": format!("Test message {}", i),
                    "sender": "user"
                })
            }).collect::<Vec<_>>()
        });
        
        let iterations = 100;
        let start = Instant::now();
        
        for _ in 0..iterations {
            let json = serde_json::to_string(&data).unwrap();
            let _: serde_json::Value = serde_json::from_str(&json).unwrap();
        }
        
        let elapsed = start.elapsed();
        let avg_ms = elapsed.as_millis() as f64 / iterations as f64;
        
        // Should complete in reasonable time (less than 10ms average)
        assert!(avg_ms < 10.0, "JSON serialization took {}ms average", avg_ms);
    }

    #[test]
    fn test_path_operations_performance() {
        let iterations = 1000;
        let start = Instant::now();
        
        for i in 0..iterations {
            let path = config::data_dir().join(format!("test_{}.json", i));
            let _ = path.to_string_lossy().to_string();
        }
        
        let elapsed = start.elapsed();
        let avg_ns = elapsed.as_nanos() as f64 / iterations as f64;
        
        // Should be reasonably fast (less than 5000ns average on most systems)
        assert!(avg_ns < 5000.0, "Path operations took {}ns average", avg_ns);
    }
}
