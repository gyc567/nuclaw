//! Task Scheduler - Runs scheduled tasks in isolated containers
//!
//! Supports three schedule types:
//! - `cron`: Cron expression (e.g., "0 9 * * *" for daily at 9am)
//! - `interval`: Fixed interval in milliseconds (e.g., "3600000" for 1 hour)
//! - `once`: Single execution at specific timestamp
//!
//! Features:
//! - Persistent task storage in SQLite
//! - Task run logging
//! - Concurrent task execution
//! - Graceful shutdown

use crate::config::timezone;
use crate::container_runner::{log_container_output, run_container};
use crate::db::Database;
use crate::error::{NuClawError, Result};
use crate::types::{ContainerInput, ContainerOutput, ScheduledTask};
use chrono::{DateTime, Utc};
use cron::Schedule;
use std::str::FromStr;
use tokio::sync::mpsc;
use tokio::time::{interval, Duration, MissedTickBehavior};

/// Default poll interval: 60 seconds
const DEFAULT_POLL_INTERVAL_SECS: u64 = 60;
/// Max concurrent tasks
const MAX_CONCURRENT_TASKS: usize = 4;
/// Default task timeout: 10 minutes
const DEFAULT_TASK_TIMEOUT_SECS: u64 = 600;

/// Get poll interval from environment or default
pub fn poll_interval() -> Duration {
    let interval_secs = std::env::var("SCHEDULER_POLL_INTERVAL")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_POLL_INTERVAL_SECS);
    Duration::from_secs(interval_secs)
}

/// Get task timeout from environment or default
pub fn task_timeout() -> Duration {
    let timeout_secs = std::env::var("TASK_TIMEOUT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_TASK_TIMEOUT_SECS);
    Duration::from_secs(timeout_secs)
}

/// Task scheduler state
#[derive(Clone)]
pub struct TaskScheduler {
    db: Database,
    poll_interval: Duration,
    task_timeout: Duration,
}

impl TaskScheduler {
    /// Create a new task scheduler
    pub fn new(db: Database) -> Self {
        Self {
            db,
            poll_interval: poll_interval(),
            task_timeout: task_timeout(),
        }
    }

    /// Run the scheduler loop
    pub async fn run(&mut self) -> Result<()> {
        let (shutdown_tx, mut shutdown_rx) = mpsc::channel::<()>(1);

        let mut interval = interval(self.poll_interval);
        interval.set_missed_tick_behavior(MissedTickBehavior::Skip);

        tracing::info!(
            "Task scheduler started with poll interval: {:?}",
            self.poll_interval
        );

        loop {
            tokio::select! {
                _ = interval.tick() => {
                    if let Err(e) = self.poll_and_execute_tasks().await {
                        tracing::error!("Error executing tasks: {}", e);
                    }
                }
                _ = shutdown_rx.recv() => {
                    tracing::info!("Task scheduler shutting down");
                    break;
                }
                _ = shutdown_tx.closed() => {
                    break;
                }
            }
        }

        Ok(())
    }

    /// Poll for due tasks and execute them
    async fn poll_and_execute_tasks(&mut self) -> Result<()> {
        let now = Utc::now().to_rfc3339();

        // Load active tasks that are due
        let tasks = self.load_due_tasks(&now).await?;

        if tasks.is_empty() {
            tracing::debug!("No tasks due for execution");
            return Ok(());
        }

        tracing::info!("Found {} tasks due for execution", tasks.len());

        // Execute tasks concurrently with limit
        let mut handles = Vec::new();
        for task in tasks {
            // Check if we've reached max concurrent tasks
            while handles.len() >= MAX_CONCURRENT_TASKS {
                // Wait for at least one to complete
                let _ = tokio::join!(handles.remove(0));
            }

            let mut scheduler = TaskScheduler::new(self.db.clone());
            let handle = tokio::spawn(async move {
                let result = scheduler.execute_single_task(&task).await;
                (task.id.clone(), result)
            });
            handles.push(handle);
        }

        // Wait for remaining tasks
        for handle in handles {
            let (task_id, result) = handle.await.map_err(|e| NuClawError::Scheduler {
                message: format!("Task execution panic: {}", e),
            })?;

            if let Err(e) = &result {
                tracing::error!("Task {} failed: {}", task_id, e);
            }
        }

        Ok(())
    }

    /// Execute a single task
    async fn execute_single_task(&mut self, task: &ScheduledTask) -> Result<()> {
        tracing::info!("Executing task: {} (group: {})", task.id, task.group_folder);

        let start_time = chrono::Utc::now();

        // Verify task is still active (may have been paused/cancelled)
        let current_task =
            self.load_task(&task.id)
                .await?
                .ok_or_else(|| NuClawError::Scheduler {
                    message: format!("Task {} not found", task.id),
                })?;

        if current_task.status != "active" {
            tracing::info!("Task {} is no longer active, skipping", task.id);
            return Ok(());
        }

        // Create container input
        let session_id = format!("scheduled_{}", task.id);
        let input = ContainerInput {
            prompt: task.prompt.clone(),
            session_id: Some(session_id.clone()),
            group_folder: task.group_folder.clone(),
            chat_jid: task.chat_jid.clone(),
            is_main: false,
            is_scheduled_task: true,
        };

        // Execute container with timeout
        let result = tokio::time::timeout(self.task_timeout, run_container(input)).await;

        let end_time = chrono::Utc::now();
        let duration_ms = (end_time - start_time).num_milliseconds();

        // Process result and log
        match result {
            Ok(Ok(output)) => {
                // Log successful execution
                self.log_task_run(task, &output, duration_ms, "success")
                    .await?;

                // Log to file
                let _ = log_container_output(&task.group_folder, &session_id, &output);

                // Calculate next run time
                if task.schedule_type == "once" {
                    // Single execution task - mark as completed
                    self.mark_task_completed(&task.id).await?;
                } else {
                    // Recurring task - calculate next run
                    if let Some(next_run) = self.calculate_next_run(task) {
                        self.update_next_run(&task.id, &next_run).await?;
                    }
                }
            }
            Ok(Err(e)) => {
                // Container execution failed
                let output = ContainerOutput {
                    status: "error".to_string(),
                    result: None,
                    new_session_id: None,
                    error: Some(e.to_string()),
                };
                self.log_task_run(task, &output, duration_ms, "error")
                    .await?;
                self.mark_task_failed(&task.id).await?;
            }
            Err(_) => {
                // Timeout
                let output = ContainerOutput {
                    status: "timeout".to_string(),
                    result: None,
                    new_session_id: None,
                    error: Some("Task execution timed out".to_string()),
                };
                self.log_task_run(task, &output, duration_ms, "timeout")
                    .await?;
                self.mark_task_failed(&task.id).await?;
            }
        }

        Ok(())
    }

    /// Calculate next run time for a task
    pub fn calculate_next_run(&self, task: &ScheduledTask) -> Option<String> {
        match task.schedule_type.as_str() {
            "cron" => self.calculate_next_cron_run(task.schedule_value.clone()),
            "interval" => self.calculate_next_interval_run(task.schedule_value.clone()),
            "once" => None,
            _ => None,
        }
    }

    /// Calculate next run time from cron expression
    fn calculate_next_cron_run(&self, cron_expr: String) -> Option<String> {
        let _tz = timezone();
        match Schedule::from_str(&cron_expr) {
            Ok(schedule) => {
                // Get next run in the specified timezone
                let next = schedule.after(&chrono::Utc::now()).next()?;
                Some(next.to_rfc3339())
            }
            Err(e) => {
                tracing::error!("Invalid cron expression '{}': {}", cron_expr, e);
                None
            }
        }
    }

    /// Calculate next run time from interval
    fn calculate_next_interval_run(&self, interval_str: String) -> Option<String> {
        let millis: i64 = interval_str.parse().ok()?;
        let next_run = chrono::Utc::now() + chrono::Duration::milliseconds(millis);
        Some(next_run.to_rfc3339())
    }

    /// Load tasks that are due for execution
    async fn load_due_tasks(&self, now: &str) -> Result<Vec<ScheduledTask>> {
        let conn = self
            .db
            .get_connection()
            .map_err(|e| NuClawError::Database {
                message: e.to_string(),
            })?;

        let mut stmt = conn
            .prepare(
                "SELECT id, group_folder, chat_jid, prompt, schedule_type, schedule_value,
                    next_run, last_run, last_result, status, created_at, context_mode
             FROM scheduled_tasks
             WHERE status = 'active'
               AND (next_run IS NULL OR next_run <= ?)
             ORDER BY next_run ASC",
            )
            .map_err(|e| NuClawError::Database {
                message: format!("Failed to prepare statement: {}", e),
            })?;

        let tasks: rusqlite::Result<Vec<ScheduledTask>> = stmt
            .query_map([now], |row| {
                Ok(ScheduledTask {
                    id: row.get(0)?,
                    group_folder: row.get(1)?,
                    chat_jid: row.get(2)?,
                    prompt: row.get(3)?,
                    schedule_type: row.get(4)?,
                    schedule_value: row.get(5)?,
                    next_run: row.get(6)?,
                    last_run: row.get(7)?,
                    last_result: row.get(8)?,
                    status: row.get(9)?,
                    created_at: row.get(10)?,
                    context_mode: row.get(11)?,
                })
            })?
            .collect();

        tasks.map_err(|e| NuClawError::Database {
            message: format!("Failed to load tasks: {}", e),
        })
    }

    /// Load a single task by ID
    async fn load_task(&self, task_id: &str) -> Result<Option<ScheduledTask>> {
        let conn = self
            .db
            .get_connection()
            .map_err(|e| NuClawError::Database {
                message: e.to_string(),
            })?;

        let mut stmt = conn
            .prepare(
                "SELECT id, group_folder, chat_jid, prompt, schedule_type, schedule_value,
                    next_run, last_run, last_result, status, created_at, context_mode
             FROM scheduled_tasks WHERE id = ?",
            )
            .map_err(|e| NuClawError::Database {
                message: format!("Failed to prepare statement: {}", e),
            })?;

        stmt.query_row([task_id], |row| {
            Ok(ScheduledTask {
                id: row.get(0)?,
                group_folder: row.get(1)?,
                chat_jid: row.get(2)?,
                prompt: row.get(3)?,
                schedule_type: row.get(4)?,
                schedule_value: row.get(5)?,
                next_run: row.get(6)?,
                last_run: row.get(7)?,
                last_result: row.get(8)?,
                status: row.get(9)?,
                created_at: row.get(10)?,
                context_mode: row.get(11)?,
            })
        })
        .map(Some)
        .or_else(|e| {
            if e == rusqlite::Error::QueryReturnedNoRows {
                Ok(None)
            } else {
                Err(NuClawError::Database {
                    message: format!("Failed to load task: {}", e),
                })
            }
        })
    }

    /// Log a task run
    async fn log_task_run(
        &self,
        task: &ScheduledTask,
        output: &ContainerOutput,
        duration_ms: i64,
        run_status: &str,
    ) -> Result<()> {
        let conn = self
            .db
            .get_connection()
            .map_err(|e| NuClawError::Database {
                message: e.to_string(),
            })?;

        let now = chrono::Utc::now().to_rfc3339();

        conn.execute(
            "INSERT INTO task_run_logs (task_id, run_at, duration_ms, status, result, error)
             VALUES (?, ?, ?, ?, ?, ?)",
            rusqlite::params![
                task.id,
                now,
                duration_ms,
                run_status,
                output.result.clone().unwrap_or_default(),
                output.error.clone().unwrap_or_default(),
            ],
        )
        .map_err(|e| NuClawError::Database {
            message: format!("Failed to log task run: {}", e),
        })?;

        // Update last_run and last_result
        let last_result = if output.status == "success" {
            output.result.clone()
        } else {
            output.error.clone()
        };

        conn.execute(
            "UPDATE scheduled_tasks SET last_run = ?, last_result = ? WHERE id = ?",
            rusqlite::params![now, last_result, task.id],
        )
        .map_err(|e| NuClawError::Database {
            message: format!("Failed to update task: {}", e),
        })?;

        Ok(())
    }

    /// Update next run time for a task
    async fn update_next_run(&self, task_id: &str, next_run: &str) -> Result<()> {
        let conn = self
            .db
            .get_connection()
            .map_err(|e| NuClawError::Database {
                message: e.to_string(),
            })?;

        conn.execute(
            "UPDATE scheduled_tasks SET next_run = ? WHERE id = ?",
            [next_run, task_id],
        )
        .map_err(|e| NuClawError::Database {
            message: format!("Failed to update next run: {}", e),
        })?;

        Ok(())
    }

    /// Mark a task as completed (for once-type tasks)
    async fn mark_task_completed(&self, task_id: &str) -> Result<()> {
        let conn = self
            .db
            .get_connection()
            .map_err(|e| NuClawError::Database {
                message: e.to_string(),
            })?;

        conn.execute(
            "UPDATE scheduled_tasks SET status = 'completed', next_run = NULL WHERE id = ?",
            [task_id],
        )
        .map_err(|e| NuClawError::Database {
            message: format!("Failed to mark task completed: {}", e),
        })?;

        Ok(())
    }

    /// Mark a task as failed
    async fn mark_task_failed(&self, task_id: &str) -> Result<()> {
        let conn = self
            .db
            .get_connection()
            .map_err(|e| NuClawError::Database {
                message: e.to_string(),
            })?;

        conn.execute(
            "UPDATE scheduled_tasks SET status = 'failed' WHERE id = ?",
            [task_id],
        )
        .map_err(|e| NuClawError::Database {
            message: format!("Failed to mark task failed: {}", e),
        })?;

        Ok(())
    }
}

/// Parse cron expression and get next run time
pub fn parse_cron_expression(expr: &str) -> Result<Schedule> {
    Schedule::from_str(expr).map_err(|e| NuClawError::Scheduler {
        message: format!("Invalid cron expression '{}': {}", expr, e),
    })
}

/// Get next run time from schedule
pub fn get_next_run_time(schedule: &Schedule) -> DateTime<Utc> {
    schedule
        .after(&chrono::Utc::now())
        .next()
        .unwrap_or_else(chrono::Utc::now)
}

/// Check if a task is due for execution
pub fn is_task_due(task: &ScheduledTask, now: &str) -> bool {
    if task.status != "active" {
        return false;
    }
    match &task.next_run {
        Some(next_run) => next_run.as_str() <= now,
        None => true,
    }
}

/// Determine task status based on execution result
pub fn determine_task_status(success: bool, is_once: bool) -> &'static str {
    if !success {
        "failed"
    } else if is_once {
        "completed"
    } else {
        "active"
    }
}

/// Validate schedule type
pub fn is_valid_schedule_type(schedule_type: &str) -> bool {
    matches!(schedule_type, "cron" | "interval" | "once")
}

/// Format duration for logging
pub fn format_duration(duration_ms: i64) -> String {
    if duration_ms < 1000 {
        format!("{}ms", duration_ms)
    } else if duration_ms < 60000 {
        format!("{}s", duration_ms / 1000)
    } else {
        format!("{}m", duration_ms / 60000)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_test_dirs() {
        use std::fs;
        use crate::config::store_dir;
        let store = store_dir();
        if !store.exists() {
            let _ = fs::create_dir_all(&store);
        }
    }

    #[test]
    fn test_parse_cron_expression() {
        // Use 6-field format with seconds (cron crate standard)
        let result = parse_cron_expression("0 0 9 * * *");
        assert!(result.is_ok(), "Expected valid cron expression");
    }

    #[test]
    fn test_parse_cron_expression_with_seconds() {
        let result = parse_cron_expression("0 0 0 9 * * *");
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_invalid_cron() {
        let result = parse_cron_expression("invalid cron");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_empty_cron() {
        let result = parse_cron_expression("");
        assert!(result.is_err());
    }

    #[test]
    fn test_get_next_run_time() {
        let schedule = parse_cron_expression("0 0 9 * * *").unwrap();
        let next = get_next_run_time(&schedule);
        let now = chrono::Utc::now();
        // Next run should be in the future
        assert!(next >= now);
    }

    #[test]
    fn test_calculate_interval_next_run() {
        setup_test_dirs();
        let scheduler = TaskScheduler::new(Database::new().unwrap());
        let next = scheduler.calculate_next_interval_run("3600000".to_string());
        assert!(next.is_some());
        // Should be approximately 1 hour from now
        let next_time: DateTime<Utc> = DateTime::from_str(&next.unwrap()).unwrap();
        let now = chrono::Utc::now();
        let diff = next_time.signed_duration_since(now).num_seconds();
        // Allow some tolerance
        assert!(diff >= 3590 && diff <= 3610);
    }

    #[test]
    fn test_calculate_interval_next_run_invalid() {
        setup_test_dirs();
        let scheduler = TaskScheduler::new(Database::new().unwrap());
        let next = scheduler.calculate_next_interval_run("not_a_number".to_string());
        assert!(next.is_none());
    }

    #[test]
    fn test_calculate_interval_next_run_zero() {
        setup_test_dirs();
        let scheduler = TaskScheduler::new(Database::new().unwrap());
        let next = scheduler.calculate_next_interval_run("0".to_string());
        assert!(next.is_some());
        // Should be essentially now
        let next_time: DateTime<Utc> = DateTime::from_str(&next.unwrap()).unwrap();
        let now = chrono::Utc::now();
        let diff = next_time.signed_duration_since(now).num_seconds();
        assert!(diff <= 1);
    }

    #[test]
    fn test_calculate_next_cron_run() {
        setup_test_dirs();
        let scheduler = TaskScheduler::new(Database::new().unwrap());
        let task = ScheduledTask {
            id: "test".to_string(),
            group_folder: "test".to_string(),
            chat_jid: "test".to_string(),
            prompt: "test".to_string(),
            schedule_type: "cron".to_string(),
            schedule_value: "0 0 9 * * *".to_string(),
            next_run: None,
            last_run: None,
            last_result: None,
            status: "active".to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
            context_mode: "isolated".to_string(),
        };
        let next = scheduler.calculate_next_run(&task);
        assert!(next.is_some());
    }

    #[test]
    fn test_calculate_next_run_once() {
        setup_test_dirs();
        let scheduler = TaskScheduler::new(Database::new().unwrap());
        let task = ScheduledTask {
            id: "test".to_string(),
            group_folder: "test".to_string(),
            chat_jid: "test".to_string(),
            prompt: "test".to_string(),
            schedule_type: "once".to_string(),
            schedule_value: "2025-01-01T00:00:00Z".to_string(),
            next_run: None,
            last_run: None,
            last_result: None,
            status: "active".to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
            context_mode: "isolated".to_string(),
        };
        let next = scheduler.calculate_next_run(&task);
        assert!(next.is_none());
    }

    #[test]
    fn test_calculate_next_run_invalid_type() {
        setup_test_dirs();
        let scheduler = TaskScheduler::new(Database::new().unwrap());
        let task = ScheduledTask {
            id: "test".to_string(),
            group_folder: "test".to_string(),
            chat_jid: "test".to_string(),
            prompt: "test".to_string(),
            schedule_type: "unknown".to_string(),
            schedule_value: "value".to_string(),
            next_run: None,
            last_run: None,
            last_result: None,
            status: "active".to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
            context_mode: "isolated".to_string(),
        };
        let next = scheduler.calculate_next_run(&task);
        assert!(next.is_none());
    }

    #[test]
    fn test_poll_interval_default() {
        let interval = poll_interval();
        assert_eq!(interval, Duration::from_secs(DEFAULT_POLL_INTERVAL_SECS));
    }

    #[test]
    fn test_poll_interval_from_env() {
        // Save original
        let original = std::env::var("SCHEDULER_POLL_INTERVAL").ok();

        std::env::set_var("SCHEDULER_POLL_INTERVAL", "120");
        let interval = poll_interval();
        assert_eq!(interval, Duration::from_secs(120));

        // Restore
        match original {
            Some(val) => std::env::set_var("SCHEDULER_POLL_INTERVAL", val),
            None => std::env::remove_var("SCHEDULER_POLL_INTERVAL"),
        }
    }

    #[test]
    fn test_poll_interval_invalid_env() {
        // Save original
        let original = std::env::var("SCHEDULER_POLL_INTERVAL").ok();

        std::env::set_var("SCHEDULER_POLL_INTERVAL", "invalid");
        let interval = poll_interval();
        // Should fall back to default
        assert_eq!(interval, Duration::from_secs(DEFAULT_POLL_INTERVAL_SECS));

        // Restore
        match original {
            Some(val) => std::env::set_var("SCHEDULER_POLL_INTERVAL", val),
            None => std::env::remove_var("SCHEDULER_POLL_INTERVAL"),
        }
    }

    #[test]
    fn test_task_timeout_default() {
        let timeout = task_timeout();
        assert_eq!(timeout, Duration::from_secs(DEFAULT_TASK_TIMEOUT_SECS));
    }

    #[test]
    fn test_task_timeout_from_env() {
        // Save original
        let original = std::env::var("TASK_TIMEOUT").ok();

        std::env::set_var("TASK_TIMEOUT", "300");
        let timeout = task_timeout();
        assert_eq!(timeout, Duration::from_secs(300));

        // Restore
        match original {
            Some(val) => std::env::set_var("TASK_TIMEOUT", val),
            None => std::env::remove_var("TASK_TIMEOUT"),
        }
    }

    #[test]
    fn test_task_scheduler_new() {
        setup_test_dirs();
        let db = Database::new().unwrap();
        let scheduler = TaskScheduler::new(db);
        // Just verify it was created
        assert_eq!(scheduler.poll_interval, poll_interval());
        assert_eq!(scheduler.task_timeout, task_timeout());
    }

    #[test]
    fn test_scheduler_clone() {
        setup_test_dirs();
        let db = Database::new().unwrap();
        let scheduler = TaskScheduler::new(db);
        let _cloned = scheduler.clone();
    }

    #[test]
    fn test_is_task_due_active_no_next_run() {
        let task = ScheduledTask {
            id: "test".to_string(),
            group_folder: "test".to_string(),
            chat_jid: "test".to_string(),
            prompt: "test".to_string(),
            schedule_type: "interval".to_string(),
            schedule_value: "3600000".to_string(),
            next_run: None,
            last_run: None,
            last_result: None,
            status: "active".to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
            context_mode: "isolated".to_string(),
        };
        let now = chrono::Utc::now().to_rfc3339();
        assert!(is_task_due(&task, &now));
    }

    #[test]
    fn test_is_task_due_active_with_past_next_run() {
        let now = chrono::Utc::now();
        let past = (now - chrono::Duration::hours(1)).to_rfc3339();
        let task = ScheduledTask {
            id: "test".to_string(),
            group_folder: "test".to_string(),
            chat_jid: "test".to_string(),
            prompt: "test".to_string(),
            schedule_type: "interval".to_string(),
            schedule_value: "3600000".to_string(),
            next_run: Some(past),
            last_run: None,
            last_result: None,
            status: "active".to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
            context_mode: "isolated".to_string(),
        };
        let now_str = now.to_rfc3339();
        assert!(is_task_due(&task, &now_str));
    }

    #[test]
    fn test_is_task_due_active_with_future_nextRun() {
        let now = chrono::Utc::now();
        let future = (now + chrono::Duration::hours(1)).to_rfc3339();
        let task = ScheduledTask {
            id: "test".to_string(),
            group_folder: "test".to_string(),
            chat_jid: "test".to_string(),
            prompt: "test".to_string(),
            schedule_type: "interval".to_string(),
            schedule_value: "3600000".to_string(),
            next_run: Some(future),
            last_run: None,
            last_result: None,
            status: "active".to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
            context_mode: "isolated".to_string(),
        };
        let now_str = now.to_rfc3339();
        assert!(!is_task_due(&task, &now_str));
    }

    #[test]
    fn test_is_task_due_inactive() {
        let now = chrono::Utc::now().to_rfc3339();
        let task = ScheduledTask {
            id: "test".to_string(),
            group_folder: "test".to_string(),
            chat_jid: "test".to_string(),
            prompt: "test".to_string(),
            schedule_type: "interval".to_string(),
            schedule_value: "3600000".to_string(),
            next_run: None,
            last_run: None,
            last_result: None,
            status: "paused".to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
            context_mode: "isolated".to_string(),
        };
        assert!(!is_task_due(&task, &now));
    }

    #[test]
    fn test_determine_task_status_success_once() {
        assert_eq!(determine_task_status(true, true), "completed");
    }

    #[test]
    fn test_determine_task_status_success_recurring() {
        assert_eq!(determine_task_status(true, false), "active");
    }

    #[test]
    fn test_determine_task_status_failed() {
        assert_eq!(determine_task_status(false, true), "failed");
        assert_eq!(determine_task_status(false, false), "failed");
    }

    #[test]
    fn test_is_valid_schedule_type() {
        assert!(is_valid_schedule_type("cron"));
        assert!(is_valid_schedule_type("interval"));
        assert!(is_valid_schedule_type("once"));
        assert!(!is_valid_schedule_type("invalid"));
        assert!(!is_valid_schedule_type(""));
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(500), "500ms");
        assert_eq!(format_duration(1000), "1s");
        assert_eq!(format_duration(30000), "30s");
        assert_eq!(format_duration(60000), "1m");
        assert_eq!(format_duration(120000), "2m");
    }
}
