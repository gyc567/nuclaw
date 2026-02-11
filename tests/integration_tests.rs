//! Integration tests for NuClaw
//!
//! These tests verify the interaction between components.

use nuclaw::config;
use nuclaw::error::Result;
use std::fs;

/// Test that required directories are created
#[test]
fn test_directory_creation() {
    // Ensure directories exist
    config::ensure_directories().expect("Failed to create directories");

    // Verify directories were created
    assert!(config::store_dir().exists(), "Store directory should exist");
    assert!(config::data_dir().exists(), "Data directory should exist");
    assert!(
        config::groups_dir().exists(),
        "Groups directory should exist"
    );
}

/// Test database initialization
#[test]
fn test_database_initialization() {
    use nuclaw::db::Database;

    // Ensure directories exist first
    config::ensure_directories().expect("Failed to create directories");

    // Initialize database
    let db = Database::new().expect("Failed to create database");

    // Verify we can get a connection
    let conn = db.get_connection().expect("Failed to get connection");

    // Verify tables exist by running a simple query
    let tables: rusqlite::Result<Vec<String>> = conn
        .prepare("SELECT name FROM sqlite_master WHERE type='table'")
        .expect("Failed to prepare statement")
        .query_map([], |row| row.get::<_, String>(0))
        .expect("Failed to query tables")
        .collect();

    let tables = tables.expect("Failed to collect results");

    assert!(
        tables.contains(&"chats".to_string()),
        "chats table should exist"
    );
    assert!(
        tables.contains(&"messages".to_string()),
        "messages table should exist"
    );
    assert!(
        tables.contains(&"scheduled_tasks".to_string()),
        "scheduled_tasks table should exist"
    );
    assert!(
        tables.contains(&"task_run_logs".to_string()),
        "task_run_logs table should exist"
    );
}

/// Test container timeout configuration
#[test]
fn test_container_timeout_configuration() {
    use std::time::Duration;

    // Save original
    let original = std::env::var("CONTAINER_TIMEOUT").ok();

    // Test default
    std::env::remove_var("CONTAINER_TIMEOUT");
    let timeout = nuclaw::container_runner::container_timeout();
    assert_eq!(timeout, Duration::from_millis(300_000));

    // Test custom value
    std::env::set_var("CONTAINER_TIMEOUT", "60000");
    let timeout = nuclaw::container_runner::container_timeout();
    assert_eq!(timeout, Duration::from_millis(60_000));

    // Restore
    match original {
        Some(val) => std::env::set_var("CONTAINER_TIMEOUT", val),
        None => std::env::remove_var("CONTAINER_TIMEOUT"),
    }
}

/// Test scheduler configuration
#[test]
fn test_scheduler_configuration() {
    use nuclaw::task_scheduler::{poll_interval, task_timeout};
    use std::time::Duration;

    // Save originals
    let original_poll = std::env::var("SCHEDULER_POLL_INTERVAL").ok();
    let original_timeout = std::env::var("TASK_TIMEOUT").ok();

    // Test defaults
    std::env::remove_var("SCHEDULER_POLL_INTERVAL");
    std::env::remove_var("TASK_TIMEOUT");
    assert_eq!(poll_interval(), Duration::from_secs(60));
    assert_eq!(task_timeout(), Duration::from_secs(600));

    // Test custom values
    std::env::set_var("SCHEDULER_POLL_INTERVAL", "30");
    std::env::set_var("TASK_TIMEOUT", "300");
    assert_eq!(poll_interval(), Duration::from_secs(30));
    assert_eq!(task_timeout(), Duration::from_secs(300));

    // Restore
    match original_poll {
        Some(val) => std::env::set_var("SCHEDULER_POLL_INTERVAL", val),
        None => std::env::remove_var("SCHEDULER_POLL_INTERVAL"),
    }
    match original_timeout {
        Some(val) => std::env::set_var("TASK_TIMEOUT", val),
        None => std::env::remove_var("TASK_TIMEOUT"),
    }
}

/// Test database operations
#[test]
fn test_database_operations() {
    use nuclaw::db::Database;
    use rusqlite::params;

    config::ensure_directories().expect("Failed to create directories");
    let db = Database::new().expect("Failed to create database");
    let conn = db.get_connection().expect("Failed to get connection");

    // Insert a test message
    conn.execute(
        "INSERT OR REPLACE INTO messages (id, chat_jid, sender, sender_name, content, timestamp, is_from_me)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
        params![
            "test_msg_1",
            "test@chat",
            "sender1",
            "Test Sender",
            "Test message content",
            "2025-01-01T00:00:00Z",
            0,
        ],
    ).expect("Failed to insert message");

    // Query the message back
    let content: String = conn
        .query_row(
            "SELECT content FROM messages WHERE id = ?",
            ["test_msg_1"],
            |row| row.get(0),
        )
        .expect("Failed to query message");

    assert_eq!(content, "Test message content");

    // Clean up
    conn.execute("DELETE FROM messages WHERE id = ?", ["test_msg_1"])
        .expect("Failed to delete test message");
}

/// Test group context isolation
#[test]
fn test_group_context_isolation() {
    use nuclaw::container_runner::create_group_ipc_directory;

    config::ensure_directories().expect("Failed to create directories");

    // Create IPC directories for different groups
    let group1 = create_group_ipc_directory("test_group_1").expect("Failed to create group 1");
    let group2 = create_group_ipc_directory("test_group_2").expect("Failed to create group 2");

    // Verify they are different paths
    assert_ne!(group1, group2);

    // Create files in each directory
    fs::write(group1.join("test.txt"), "group1").expect("Failed to write to group1");
    fs::write(group2.join("test.txt"), "group2").expect("Failed to write to group2");

    // Verify isolation
    let content1 = fs::read_to_string(group1.join("test.txt")).expect("Failed to read group1");
    let content2 = fs::read_to_string(group2.join("test.txt")).expect("Failed to read group2");

    assert_eq!(content1, "group1");
    assert_eq!(content2, "group2");

    // Cleanup
    let _ = fs::remove_dir_all(&group1);
    let _ = fs::remove_dir_all(&group2);
}

/// Test cron expression parsing with various formats
#[test]
fn test_cron_expression_variations() {
    use nuclaw::task_scheduler::parse_cron_expression;

    // Standard 6-field cron (with seconds)
    assert!(parse_cron_expression("0 0 9 * * *").is_ok());
    assert!(parse_cron_expression("0 0 0 1 * *").is_ok()); // First day of month
    assert!(parse_cron_expression("0 0 12 * * 1-5").is_ok()); // Weekdays at noon

    // Invalid expressions
    assert!(parse_cron_expression("").is_err());
    assert!(parse_cron_expression("invalid").is_err());
    assert!(parse_cron_expression("* * *").is_err()); // Too few fields
}

/// Test error handling for missing database
#[test]
#[ignore = "May interfere with other tests"]
fn test_database_error_handling() {
    // This test is ignored by default as it manipulates environment
    // It would test error handling when database cannot be opened
}

/// Test configuration loading from environment
#[test]
fn test_environment_configuration() {
    use nuclaw::config::assistant_name;

    // Save originals
    let original_name = std::env::var("ASSISTANT_NAME").ok();

    // Test assistant name default
    std::env::remove_var("ASSISTANT_NAME");
    assert_eq!(assistant_name(), "Andy");

    // Test custom assistant name
    std::env::set_var("ASSISTANT_NAME", "Bob");
    assert_eq!(assistant_name(), "Bob");

    // Restore
    match original_name {
        Some(val) => std::env::set_var("ASSISTANT_NAME", val),
        None => std::env::remove_var("ASSISTANT_NAME"),
    }
}

/// Test max output size configuration
#[test]
fn test_max_output_size_configuration() {
    use nuclaw::container_runner::max_output_size;

    // Save original
    let original = std::env::var("CONTAINER_MAX_OUTPUT_SIZE").ok();

    // Test default
    std::env::remove_var("CONTAINER_MAX_OUTPUT_SIZE");
    let size = max_output_size();
    assert_eq!(size, 10 * 1024 * 1024); // 10MB default

    // Test custom value
    std::env::set_var("CONTAINER_MAX_OUTPUT_SIZE", "5242880");
    let size = max_output_size();
    assert_eq!(size, 5 * 1024 * 1024); // 5MB

    // Restore
    match original {
        Some(val) => std::env::set_var("CONTAINER_MAX_OUTPUT_SIZE", val),
        None => std::env::remove_var("CONTAINER_MAX_OUTPUT_SIZE"),
    }
}
