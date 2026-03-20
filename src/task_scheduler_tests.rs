//! Comprehensive tests for TaskScheduler service
//! Following NuClaw test patterns from error.rs and db.rs

use crate::error::{NuClawError, Result};
use crate::types::{ScheduledTask, TaskStatus};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

/// Task scheduler configuration
#[derive(Debug, Clone)]
pub struct TaskSchedulerConfig {
    pub poll_interval_ms: u64,
    pub max_concurrent_tasks: usize,
    pub task_timeout_ms: u64,
    pub enable_retries: bool,
    pub max_retries: u32,
}

impl Default for TaskSchedulerConfig {
    fn default() -> Self {
        Self {
            poll_interval_ms: std::env::var("SCHEDULER_POLL_INTERVAL_MS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(60000),
            max_concurrent_tasks: std::env::var("SCHEDULER_MAX_CONCURRENT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(5),
            task_timeout_ms: std::env::var("SCHEDULER_TASK_TIMEOUT_MS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(300000),
            enable_retries: std::env::var("SCHEDULER_ENABLE_RETRIES")
                .ok()
                .map(|v| v == "true")
                .unwrap_or(true),
            max_retries: std::env::var("SCHEDULER_MAX_RETRIES")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(3),
        }
    }
}

/// Task scheduler service
#[derive(Debug, Clone)]
pub struct TaskScheduler {
    config: TaskSchedulerConfig,
    tasks: HashMap<String, ScheduledTask>,
    running_tasks: HashMap<String, DateTime<Utc>>,
}

impl TaskScheduler {
    /// Create a new TaskScheduler with default config
    pub fn new() -> Result<Self> {
        Self::with_config(TaskSchedulerConfig::default())
    }

    /// Create a new TaskScheduler with custom config
    pub fn with_config(config: TaskSchedulerConfig) -> Result<Self> {
        if config.max_concurrent_tasks == 0 {
            return Err(NuClawError::Validation {
                message: "max_concurrent_tasks must be greater than 0".to_string(),
            });
        }
        if config.poll_interval_ms == 0 {
            return Err(NuClawError::Validation {
                message: "poll_interval_ms must be greater than 0".to_string(),
            });
        }
        Ok(TaskScheduler {
            config,
            tasks: HashMap::new(),
            running_tasks: HashMap::new(),
        })
    }

    /// Get the scheduler configuration
    pub fn config(&self) -> &TaskSchedulerConfig {
        &self.config
    }

    /// Add a task to the scheduler
    pub fn add_task(&mut self, task: ScheduledTask) -> Result<()> {
        if task.id.is_empty() {
            return Err(NuClawError::Validation {
                message: "Task ID cannot be empty".to_string(),
            });
        }
        if task.prompt.is_empty() {
            return Err(NuClawError::Validation {
                message: "Task prompt cannot be empty".to_string(),
            });
        }
        self.tasks.insert(task.id.clone(), task);
        Ok(())
    }

    /// Remove a task from the scheduler
    pub fn remove_task(&mut self, task_id: &str) -> Result<ScheduledTask> {
        self.tasks
            .remove(task_id)
            .ok_or_else(|| NuClawError::NotFound {
                message: format!("Task not found: {}", task_id),
            })
    }

    /// Get a task by ID
    pub fn get_task(&self, task_id: &str) -> Option<&ScheduledTask> {
        self.tasks.get(task_id)
    }

    /// Get all tasks
    pub fn get_all_tasks(&self) -> Vec<&ScheduledTask> {
        self.tasks.values().collect()
    }

    /// Get tasks by status
    pub fn get_tasks_by_status(&self, status: TaskStatus) -> Vec<&ScheduledTask> {
        self.tasks.values().filter(|t| t.status == status).collect()
    }

    /// Get due tasks (tasks that should run now)
    pub fn get_due_tasks(&self) -> Vec<&ScheduledTask> {
        let now = Utc::now();
        self.tasks
            .values()
            .filter(|t| {
                t.status == TaskStatus::Active
                    && t.next_run.map(|nr| nr <= now).unwrap_or(false)
                    && !self.running_tasks.contains_key(&t.id)
            })
            .collect()
    }

    /// Mark a task as running
    pub fn mark_running(&mut self, task_id: &str) -> Result<()> {
        if !self.tasks.contains_key(task_id) {
            return Err(NuClawError::NotFound {
                message: format!("Task not found: {}", task_id),
            });
        }
        let current_running = self.running_tasks.len();
        if current_running >= self.config.max_concurrent_tasks {
            return Err(NuClawError::Validation {
                message: format!(
                    "Max concurrent tasks ({}) reached",
                    self.config.max_concurrent_tasks
                ),
            });
        }
        self.running_tasks.insert(task_id.to_string(), Utc::now());
        Ok(())
    }

    /// Mark a task as completed
    pub fn mark_completed(&mut self, task_id: &str, result: Option<String>) -> Result<()> {
        self.running_tasks.remove(task_id);
        let task = self
            .tasks
            .get_mut(task_id)
            .ok_or_else(|| NuClawError::NotFound {
                message: format!("Task not found: {}", task_id),
            })?;
        task.last_run = Some(Utc::now());
        task.last_result = result;
        Ok(())
    }

    /// Get the number of currently running tasks
    pub fn running_count(&self) -> usize {
        self.running_tasks.len()
    }

    /// Check if a task is running
    pub fn is_running(&self, task_id: &str) -> bool {
        self.running_tasks.contains_key(task_id)
    }

    /// Get scheduler statistics
    pub fn stats(&self) -> SchedulerStats {
        SchedulerStats {
            total_tasks: self.tasks.len(),
            running_tasks: self.running_tasks.len(),
            active_tasks: self.get_tasks_by_status(TaskStatus::Active).len(),
            paused_tasks: self.get_tasks_by_status(TaskStatus::Paused).len(),
            max_concurrent: self.config.max_concurrent_tasks,
        }
    }
}

/// Scheduler statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchedulerStats {
    pub total_tasks: usize,
    pub running_tasks: usize,
    pub active_tasks: usize,
    pub paused_tasks: usize,
    pub max_concurrent: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::Mutex;

    // Thread-safe environment lock for tests that modify env vars
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn setup() {
        let _guard = ENV_LOCK.lock().unwrap();
        // Setup any test directories or resources
        let test_dir = std::env::temp_dir().join("nuclaw_test");
        let _ = fs::create_dir_all(&test_dir);
    }

    fn cleanup() {
        let _guard = ENV_LOCK.lock().unwrap();
        let test_dir = std::env::temp_dir().join("nuclaw_test");
        let _ = fs::remove_dir_all(&test_dir);
    }

    // =========================================================================
    // Basic Functionality Tests
    // =========================================================================

    #[test]
    fn test_scheduler_new() {
        setup();
        let scheduler = TaskScheduler::new();
        assert!(
            scheduler.is_ok(),
            "Scheduler should be created successfully"
        );
        cleanup();
    }

    #[test]
    fn test_scheduler_with_config() {
        setup();
        let config = TaskSchedulerConfig {
            poll_interval_ms: 30000,
            max_concurrent_tasks: 10,
            task_timeout_ms: 60000,
            enable_retries: false,
            max_retries: 1,
        };
        let scheduler = TaskScheduler::with_config(config.clone());
        assert!(scheduler.is_ok());
        assert_eq!(scheduler.unwrap().config().poll_interval_ms, 30000);
        cleanup();
    }

    #[test]
    fn test_add_task() {
        setup();
        let mut scheduler = TaskScheduler::new().unwrap();
        let task = ScheduledTask {
            id: "task_001".to_string(),
            group_folder: "test_group".to_string(),
            chat_jid: "12345@s.whatsapp.net".to_string(),
            prompt: "Test prompt".to_string(),
            schedule_type: "cron".to_string(),
            schedule_value: "0 * * * *".to_string(),
            next_run: Some(Utc::now()),
            last_run: None,
            last_result: None,
            status: TaskStatus::Active,
            created_at: Utc::now(),
            context_mode: "isolated".to_string(),
        };
        let result = scheduler.add_task(task);
        assert!(result.is_ok());
        assert_eq!(scheduler.get_all_tasks().len(), 1);
        cleanup();
    }

    #[test]
    fn test_get_task() {
        setup();
        let mut scheduler = TaskScheduler::new().unwrap();
        let task = ScheduledTask {
            id: "task_002".to_string(),
            group_folder: "test_group".to_string(),
            chat_jid: "12345@s.whatsapp.net".to_string(),
            prompt: "Test prompt".to_string(),
            schedule_type: "interval".to_string(),
            schedule_value: "3600".to_string(),
            next_run: Some(Utc::now()),
            last_run: None,
            last_result: None,
            status: TaskStatus::Active,
            created_at: Utc::now(),
            context_mode: "isolated".to_string(),
        };
        scheduler.add_task(task).unwrap();
        let retrieved = scheduler.get_task("task_002");
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().id, "task_002");
        cleanup();
    }

    #[test]
    fn test_remove_task() {
        setup();
        let mut scheduler = TaskScheduler::new().unwrap();
        let task = ScheduledTask {
            id: "task_003".to_string(),
            group_folder: "test_group".to_string(),
            chat_jid: "12345@s.whatsapp.net".to_string(),
            prompt: "Test prompt".to_string(),
            schedule_type: "once".to_string(),
            schedule_value: "2024-12-31T23:59:59Z".to_string(),
            next_run: Some(Utc::now()),
            last_run: None,
            last_result: None,
            status: TaskStatus::Active,
            created_at: Utc::now(),
            context_mode: "isolated".to_string(),
        };
        scheduler.add_task(task).unwrap();
        assert_eq!(scheduler.get_all_tasks().len(), 1);
        let removed = scheduler.remove_task("task_003");
        assert!(removed.is_ok());
        assert_eq!(scheduler.get_all_tasks().len(), 0);
        cleanup();
    }

    #[test]
    fn test_get_tasks_by_status() {
        setup();
        let mut scheduler = TaskScheduler::new().unwrap();
        let active_task = ScheduledTask {
            id: "active_001".to_string(),
            group_folder: "test_group".to_string(),
            chat_jid: "12345@s.whatsapp.net".to_string(),
            prompt: "Active task".to_string(),
            schedule_type: "cron".to_string(),
            schedule_value: "0 * * * *".to_string(),
            next_run: Some(Utc::now()),
            last_run: None,
            last_result: None,
            status: TaskStatus::Active,
            created_at: Utc::now(),
            context_mode: "isolated".to_string(),
        };
        let paused_task = ScheduledTask {
            id: "paused_001".to_string(),
            group_folder: "test_group".to_string(),
            chat_jid: "12345@s.whatsapp.net".to_string(),
            prompt: "Paused task".to_string(),
            schedule_type: "cron".to_string(),
            schedule_value: "0 * * * *".to_string(),
            next_run: Some(Utc::now()),
            last_run: None,
            last_result: None,
            status: TaskStatus::Paused,
            created_at: Utc::now(),
            context_mode: "isolated".to_string(),
        };
        scheduler.add_task(active_task).unwrap();
        scheduler.add_task(paused_task).unwrap();
        let active_tasks = scheduler.get_tasks_by_status(TaskStatus::Active);
        assert_eq!(active_tasks.len(), 1);
        assert_eq!(active_tasks[0].id, "active_001");
        let paused_tasks = scheduler.get_tasks_by_status(TaskStatus::Paused);
        assert_eq!(paused_tasks.len(), 1);
        cleanup();
    }

    // =========================================================================
    // Error Handling Tests
    // =========================================================================

    #[test]
    fn test_add_task_empty_id() {
        setup();
        let mut scheduler = TaskScheduler::new().unwrap();
        let task = ScheduledTask {
            id: "".to_string(),
            group_folder: "test_group".to_string(),
            chat_jid: "12345@s.whatsapp.net".to_string(),
            prompt: "Test prompt".to_string(),
            schedule_type: "cron".to_string(),
            schedule_value: "0 * * * *".to_string(),
            next_run: Some(Utc::now()),
            last_run: None,
            last_result: None,
            status: TaskStatus::Active,
            created_at: Utc::now(),
            context_mode: "isolated".to_string(),
        };
        let result = scheduler.add_task(task);
        assert!(result.is_err());
        match result {
            Err(NuClawError::Validation { message }) => {
                assert!(message.contains("Task ID cannot be empty"));
            }
            _ => panic!("Expected Validation error"),
        }
        cleanup();
    }

    #[test]
    fn test_add_task_empty_prompt() {
        setup();
        let mut scheduler = TaskScheduler::new().unwrap();
        let task = ScheduledTask {
            id: "task_004".to_string(),
            group_folder: "test_group".to_string(),
            chat_jid: "12345@s.whatsapp.net".to_string(),
            prompt: "".to_string(),
            schedule_type: "cron".to_string(),
            schedule_value: "0 * * * *".to_string(),
            next_run: Some(Utc::now()),
            last_run: None,
            last_result: None,
            status: TaskStatus::Active,
            created_at: Utc::now(),
            context_mode: "isolated".to_string(),
        };
        let result = scheduler.add_task(task);
        assert!(result.is_err());
        match result {
            Err(NuClawError::Validation { message }) => {
                assert!(message.contains("Task prompt cannot be empty"));
            }
            _ => panic!("Expected Validation error"),
        }
        cleanup();
    }

    #[test]
    fn test_remove_nonexistent_task() {
        setup();
        let mut scheduler = TaskScheduler::new().unwrap();
        let result = scheduler.remove_task("nonexistent");
        assert!(result.is_err());
        match result {
            Err(NuClawError::NotFound { message }) => {
                assert!(message.contains("Task not found"));
            }
            _ => panic!("Expected NotFound error"),
        }
        cleanup();
    }

    #[test]
    fn test_mark_running_nonexistent_task() {
        setup();
        let mut scheduler = TaskScheduler::new().unwrap();
        let result = scheduler.mark_running("nonexistent");
        assert!(result.is_err());
        match result {
            Err(NuClawError::NotFound { message }) => {
                assert!(message.contains("Task not found"));
            }
            _ => panic!("Expected NotFound error"),
        }
        cleanup();
    }

    #[test]
    fn test_mark_completed_nonexistent_task() {
        setup();
        let mut scheduler = TaskScheduler::new().unwrap();
        let result = scheduler.mark_completed("nonexistent", None);
        assert!(result.is_err());
        match result {
            Err(NuClawError::NotFound { message }) => {
                assert!(message.contains("Task not found"));
            }
            _ => panic!("Expected NotFound error"),
        }
        cleanup();
    }

    #[test]
    fn test_config_zero_max_concurrent() {
        setup();
        let config = TaskSchedulerConfig {
            poll_interval_ms: 60000,
            max_concurrent_tasks: 0,
            task_timeout_ms: 300000,
            enable_retries: true,
            max_retries: 3,
        };
        let result = TaskScheduler::with_config(config);
        assert!(result.is_err());
        match result {
            Err(NuClawError::Validation { message }) => {
                assert!(message.contains("max_concurrent_tasks must be greater than 0"));
            }
            _ => panic!("Expected Validation error"),
        }
        cleanup();
    }

    #[test]
    fn test_config_zero_poll_interval() {
        setup();
        let config = TaskSchedulerConfig {
            poll_interval_ms: 0,
            max_concurrent_tasks: 5,
            task_timeout_ms: 300000,
            enable_retries: true,
            max_retries: 3,
        };
        let result = TaskScheduler::with_config(config);
        assert!(result.is_err());
        match result {
            Err(NuClawError::Validation { message }) => {
                assert!(message.contains("poll_interval_ms must be greater than 0"));
            }
            _ => panic!("Expected Validation error"),
        }
        cleanup();
    }

    // =========================================================================
    // Edge Case Tests
    // =========================================================================

    #[test]
    fn test_concurrent_task_limit() {
        setup();
        let config = TaskSchedulerConfig {
            poll_interval_ms: 60000,
            max_concurrent_tasks: 2,
            task_timeout_ms: 300000,
            enable_retries: true,
            max_retries: 3,
        };
        let mut scheduler = TaskScheduler::with_config(config).unwrap();
        // Add 3 tasks
        for i in 0..3 {
            let task = ScheduledTask {
                id: format!("task_{}", i),
                group_folder: "test_group".to_string(),
                chat_jid: "12345@s.whatsapp.net".to_string(),
                prompt: "Test prompt".to_string(),
                schedule_type: "cron".to_string(),
                schedule_value: "0 * * * *".to_string(),
                next_run: Some(Utc::now()),
                last_run: None,
                last_result: None,
                status: TaskStatus::Active,
                created_at: Utc::now(),
                context_mode: "isolated".to_string(),
            };
            scheduler.add_task(task).unwrap();
        }
        // Mark 2 tasks as running (max concurrent)
        assert!(scheduler.mark_running("task_0").is_ok());
        assert!(scheduler.mark_running("task_1").is_ok());
        // Third task should fail due to concurrent limit
        let result = scheduler.mark_running("task_2");
        assert!(result.is_err());
        match result {
            Err(NuClawError::Validation { message }) => {
                assert!(message.contains("Max concurrent tasks"));
            }
            _ => panic!("Expected Validation error"),
        }
        assert_eq!(scheduler.running_count(), 2);
        cleanup();
    }

    #[test]
    fn test_mark_task_completed() {
        setup();
        let mut scheduler = TaskScheduler::new().unwrap();
        let task = ScheduledTask {
            id: "task_complete".to_string(),
            group_folder: "test_group".to_string(),
            chat_jid: "12345@s.whatsapp.net".to_string(),
            prompt: "Test prompt".to_string(),
            schedule_type: "cron".to_string(),
            schedule_value: "0 * * * *".to_string(),
            next_run: Some(Utc::now()),
            last_run: None,
            last_result: None,
            status: TaskStatus::Active,
            created_at: Utc::now(),
            context_mode: "isolated".to_string(),
        };
        scheduler.add_task(task).unwrap();
        scheduler.mark_running("task_complete").unwrap();
        assert!(scheduler.is_running("task_complete"));
        let result = scheduler.mark_completed("task_complete", Some("Success".to_string()));
        assert!(result.is_ok());
        assert!(!scheduler.is_running("task_complete"));
        assert_eq!(scheduler.running_count(), 0);
        let task = scheduler.get_task("task_complete").unwrap();
        assert!(task.last_run.is_some());
        assert_eq!(task.last_result, Some("Success".to_string()));
        cleanup();
    }

    #[test]
    fn test_get_due_tasks() {
        setup();
        let mut scheduler = TaskScheduler::new().unwrap();
        let past_time = Utc::now() - chrono::Duration::hours(1);
        let future_time = Utc::now() + chrono::Duration::hours(1);
        let due_task = ScheduledTask {
            id: "due_task".to_string(),
            group_folder: "test_group".to_string(),
            chat_jid: "12345@s.whatsapp.net".to_string(),
            prompt: "Due task".to_string(),
            schedule_type: "cron".to_string(),
            schedule_value: "0 * * * *".to_string(),
            next_run: Some(past_time),
            last_run: None,
            last_result: None,
            status: TaskStatus::Active,
            created_at: Utc::now(),
            context_mode: "isolated".to_string(),
        };
        let future_task = ScheduledTask {
            id: "future_task".to_string(),
            group_folder: "test_group".to_string(),
            chat_jid: "12345@s.whatsapp.net".to_string(),
            prompt: "Future task".to_string(),
            schedule_type: "cron".to_string(),
            schedule_value: "0 * * * *".to_string(),
            next_run: Some(future_time),
            last_run: None,
            last_result: None,
            status: TaskStatus::Active,
            created_at: Utc::now(),
            context_mode: "isolated".to_string(),
        };
        let paused_task = ScheduledTask {
            id: "paused_task".to_string(),
            group_folder: "test_group".to_string(),
            chat_jid: "12345@s.whatsapp.net".to_string(),
            prompt: "Paused task".to_string(),
            schedule_type: "cron".to_string(),
            schedule_value: "0 * * * *".to_string(),
            next_run: Some(past_time),
            last_run: None,
            last_result: None,
            status: TaskStatus::Paused,
            created_at: Utc::now(),
            context_mode: "isolated".to_string(),
        };
        scheduler.add_task(due_task).unwrap();
        scheduler.add_task(future_task).unwrap();
        scheduler.add_task(paused_task).unwrap();
        let due_tasks = scheduler.get_due_tasks();
        assert_eq!(due_tasks.len(), 1);
        assert_eq!(due_tasks[0].id, "due_task");
        cleanup();
    }

    #[test]
    fn test_scheduler_stats() {
        setup();
        let mut scheduler = TaskScheduler::new().unwrap();
        let active_task = ScheduledTask {
            id: "stats_active".to_string(),
            group_folder: "test_group".to_string(),
            chat_jid: "12345@s.whatsapp.net".to_string(),
            prompt: "Active task".to_string(),
            schedule_type: "cron".to_string(),
            schedule_value: "0 * * * *".to_string(),
            next_run: Some(Utc::now()),
            last_run: None,
            last_result: None,
            status: TaskStatus::Active,
            created_at: Utc::now(),
            context_mode: "isolated".to_string(),
        };
        let paused_task = ScheduledTask {
            id: "stats_paused".to_string(),
            group_folder: "test_group".to_string(),
            chat_jid: "12345@s.whatsapp.net".to_string(),
            prompt: "Paused task".to_string(),
            schedule_type: "cron".to_string(),
            schedule_value: "0 * * * *".to_string(),
            next_run: Some(Utc::now()),
            last_run: None,
            last_result: None,
            status: TaskStatus::Paused,
            created_at: Utc::now(),
            context_mode: "isolated".to_string(),
        };
        scheduler.add_task(active_task).unwrap();
        scheduler.add_task(paused_task).unwrap();
        scheduler.mark_running("stats_active").unwrap();
        let stats = scheduler.stats();
        assert_eq!(stats.total_tasks, 2);
        assert_eq!(stats.active_tasks, 1);
        assert_eq!(stats.paused_tasks, 1);
        assert_eq!(stats.running_tasks, 1);
        cleanup();
    }

    #[test]
    fn test_clone_scheduler() {
        setup();
        let mut scheduler = TaskScheduler::new().unwrap();
        let task = ScheduledTask {
            id: "clone_task".to_string(),
            group_folder: "test_group".to_string(),
            chat_jid: "12345@s.whatsapp.net".to_string(),
            prompt: "Clone task".to_string(),
            schedule_type: "cron".to_string(),
            schedule_value: "0 * * * *".to_string(),
            next_run: Some(Utc::now()),
            last_run: None,
            last_result: None,
            status: TaskStatus::Active,
            created_at: Utc::now(),
            context_mode: "isolated".to_string(),
        };
        scheduler.add_task(task).unwrap();
        let scheduler2 = scheduler.clone();
        assert_eq!(scheduler2.get_all_tasks().len(), 1);
        assert_eq!(
            scheduler2.get_task("clone_task").unwrap().prompt,
            "Clone task"
        );
        cleanup();
    }

    // =========================================================================
    // Environment Variable Tests
    // =========================================================================

    #[test]
    fn test_config_defaults() {
        let _lock = ENV_LOCK.lock().unwrap();
        std::env::remove_var("SCHEDULER_POLL_INTERVAL_MS");
        std::env::remove_var("SCHEDULER_MAX_CONCURRENT");
        std::env::remove_var("SCHEDULER_TASK_TIMEOUT_MS");
        std::env::remove_var("SCHEDULER_ENABLE_RETRIES");
        std::env::remove_var("SCHEDULER_MAX_RETRIES");
        let config = TaskSchedulerConfig::default();
        assert_eq!(config.poll_interval_ms, 60000);
        assert_eq!(config.max_concurrent_tasks, 5);
        assert_eq!(config.task_timeout_ms, 300000);
        assert_eq!(config.enable_retries, true);
        assert_eq!(config.max_retries, 3);
    }

    #[test]
    fn test_config_from_env() {
        let _lock = ENV_LOCK.lock().unwrap();
        std::env::set_var("SCHEDULER_POLL_INTERVAL_MS", "120000");
        std::env::set_var("SCHEDULER_MAX_CONCURRENT", "10");
        std::env::set_var("SCHEDULER_TASK_TIMEOUT_MS", "600000");
        std::env::set_var("SCHEDULER_ENABLE_RETRIES", "false");
        std::env::set_var("SCHEDULER_MAX_RETRIES", "5");
        let config = TaskSchedulerConfig::default();
        assert_eq!(config.poll_interval_ms, 120000);
        assert_eq!(config.max_concurrent_tasks, 10);
        assert_eq!(config.task_timeout_ms, 600000);
        assert_eq!(config.enable_retries, false);
        assert_eq!(config.max_retries, 5);
        // Cleanup
        std::env::remove_var("SCHEDULER_POLL_INTERVAL_MS");
        std::env::remove_var("SCHEDULER_MAX_CONCURRENT");
        std::env::remove_var("SCHEDULER_TASK_TIMEOUT_MS");
        std::env::remove_var("SCHEDULER_ENABLE_RETRIES");
        std::env::remove_var("SCHEDULER_MAX_RETRIES");
    }

    #[test]
    fn test_config_invalid_env_values() {
        let _lock = ENV_LOCK.lock().unwrap();
        // Invalid values should fall back to defaults
        std::env::set_var("SCHEDULER_POLL_INTERVAL_MS", "invalid");
        std::env::set_var("SCHEDULER_MAX_CONCURRENT", "not_a_number");
        let config = TaskSchedulerConfig::default();
        assert_eq!(config.poll_interval_ms, 60000); // Default
        assert_eq!(config.max_concurrent_tasks, 5); // Default
        std::env::remove_var("SCHEDULER_POLL_INTERVAL_MS");
        std::env::remove_var("SCHEDULER_MAX_CONCURRENT");
    }
}
