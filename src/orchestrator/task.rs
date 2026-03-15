//! Task types for the orchestrator

use serde::{Deserialize, Serialize};
use std::fmt;

/// Unique identifier for a task
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TaskId(pub String);

impl TaskId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }
}

impl Default for TaskId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for TaskId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "TaskId({})", self.0)
    }
}

/// Source of the task
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskSource {
    /// Task from scheduled job
    Scheduled,
    /// Task from user message
    UserMessage,
    /// Task from API
    Api,
    /// Task from webhook
    Webhook,
    /// Task from other source
    Other(String),
}

impl Default for TaskSource {
    fn default() -> Self {
        Self::Other("unknown".to_string())
    }
}

/// Status of a task
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskStatus {
    /// Task is pending in queue
    Pending,
    /// Task is currently running
    Running,
    /// Task completed successfully
    Completed,
    /// Task failed
    Failed { error: String },
    /// Task is queued for retry
    Retrying { attempt: u32 },
}

impl Default for TaskStatus {
    fn default() -> Self {
        Self::Pending
    }
}

/// Priority level for tasks
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Priority {
    /// Lowest priority
    Low = 0,
    /// Normal priority
    Normal = 1,
    /// High priority
    High = 2,
    /// Critical priority
    Critical = 3,
}

impl Default for Priority {
    fn default() -> Self {
        Self::Normal
    }
}

/// A task to be executed by the orchestrator
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    /// Unique identifier
    pub id: TaskId,
    /// Task payload/data
    pub payload: String,
    /// Source of the task
    pub source: TaskSource,
    /// Priority level
    pub priority: Priority,
    /// Current status
    pub status: TaskStatus,
    /// Number of retry attempts
    pub retry_count: u32,
    /// Maximum retry attempts
    pub max_retries: u32,
    /// Created timestamp (ISO 8601)
    pub created_at: String,
    /// Started timestamp (ISO 8601)
    pub started_at: Option<String>,
    /// Completed timestamp (ISO 8601)
    pub completed_at: Option<String>,
}

impl Task {
    /// Create a new task with default values
    pub fn new(payload: String) -> Self {
        Self {
            id: TaskId::new(),
            payload,
            source: TaskSource::default(),
            priority: Priority::default(),
            status: TaskStatus::default(),
            retry_count: 0,
            max_retries: 3,
            created_at: chrono::Utc::now().to_rfc3339(),
            started_at: None,
            completed_at: None,
        }
    }

    /// Create a task with custom source
    pub fn with_source(mut self, source: TaskSource) -> Self {
        self.source = source;
        self
    }

    /// Create a task with custom priority
    pub fn with_priority(mut self, priority: Priority) -> Self {
        self.priority = priority;
        self
    }

    /// Create a task with custom max retries
    pub fn with_max_retries(mut self, max_retries: u32) -> Self {
        self.max_retries = max_retries;
        self
    }

    /// Check if task can be retried
    pub fn can_retry(&self) -> bool {
        self.retry_count < self.max_retries
    }

    /// Mark task as running
    pub fn start(&mut self) {
        self.status = TaskStatus::Running;
        self.started_at = Some(chrono::Utc::now().to_rfc3339());
    }

    /// Mark task as completed
    pub fn complete(&mut self) {
        self.status = TaskStatus::Completed;
        self.completed_at = Some(chrono::Utc::now().to_rfc3339());
    }

    /// Mark task as failed
    pub fn fail(&mut self, error: String) {
        if self.can_retry() {
            self.retry_count += 1;
            self.status = TaskStatus::Retrying {
                attempt: self.retry_count,
            };
        } else {
            self.status = TaskStatus::Failed { error };
            self.completed_at = Some(chrono::Utc::now().to_rfc3339());
        }
    }
}

/// Result of task execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResult {
    /// Task ID
    pub task_id: TaskId,
    /// Whether execution was successful
    pub success: bool,
    /// Output/result of the task
    pub output: Option<String>,
    /// Error message if failed
    pub error: Option<String>,
    /// Execution duration in milliseconds
    pub duration_ms: u64,
}

impl TaskResult {
    /// Create a successful result
    pub fn success(task_id: TaskId, output: String, duration_ms: u64) -> Self {
        Self {
            task_id,
            success: true,
            output: Some(output),
            error: None,
            duration_ms,
        }
    }

    /// Create a failed result
    pub fn failure(task_id: TaskId, error: String, duration_ms: u64) -> Self {
        Self {
            task_id,
            success: false,
            output: None,
            error: Some(error),
            duration_ms,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_id_new() {
        let id1 = TaskId::new();
        let id2 = TaskId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_task_id_display() {
        let id = TaskId("test-123".to_string());
        assert_eq!(format!("{}", id), "TaskId(test-123)");
    }

    #[test]
    fn test_task_id_default() {
        let id1 = TaskId::default();
        let id2 = TaskId::default();
        // Should be different (random UUID)
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_task_source_variants() {
        let _ = TaskSource::Scheduled;
        let _ = TaskSource::UserMessage;
        let _ = TaskSource::Api;
        let _ = TaskSource::Webhook;
        let _ = TaskSource::Other("custom".to_string());
    }

    #[test]
    fn test_task_status_variants() {
        let _ = TaskStatus::Pending;
        let _ = TaskStatus::Running;
        let _ = TaskStatus::Completed;
        let _ = TaskStatus::Failed {
            error: "error".to_string(),
        };
        let _ = TaskStatus::Retrying { attempt: 1 };
    }

    #[test]
    fn test_priority_ordering() {
        assert!(Priority::Critical > Priority::High);
        assert!(Priority::High > Priority::Normal);
        assert!(Priority::Normal > Priority::Low);
    }

    #[test]
    fn test_task_new() {
        let task = Task::new("test payload".to_string());
        assert_eq!(task.payload, "test payload");
        assert_eq!(task.status, TaskStatus::Pending);
        assert_eq!(task.priority, Priority::Normal);
        assert_eq!(task.retry_count, 0);
        assert_eq!(task.max_retries, 3);
    }

    #[test]
    fn test_task_with_source() {
        let task = Task::new("test".to_string()).with_source(TaskSource::Scheduled);
        assert_eq!(task.source, TaskSource::Scheduled);
    }

    #[test]
    fn test_task_with_priority() {
        let task = Task::new("test".to_string()).with_priority(Priority::High);
        assert_eq!(task.priority, Priority::High);
    }

    #[test]
    fn test_task_with_max_retries() {
        let task = Task::new("test".to_string()).with_max_retries(5);
        assert_eq!(task.max_retries, 5);
    }

    #[test]
    fn test_task_can_retry() {
        let mut task = Task::new("test".to_string());
        assert!(task.can_retry());

        task.retry_count = 2;
        assert!(task.can_retry());

        task.retry_count = 3;
        assert!(!task.can_retry());
    }

    #[test]
    fn test_task_start() {
        let mut task = Task::new("test".to_string());
        task.start();

        assert_eq!(task.status, TaskStatus::Running);
        assert!(task.started_at.is_some());
    }

    #[test]
    fn test_task_complete() {
        let mut task = Task::new("test".to_string());
        task.start();
        task.complete();

        assert_eq!(task.status, TaskStatus::Completed);
        assert!(task.completed_at.is_some());
    }

    #[test]
    fn test_task_fail_triggers_retry() {
        let mut task = Task::new("test".to_string()).with_max_retries(3);

        task.fail("error 1".to_string());

        match &task.status {
            TaskStatus::Retrying { attempt } => {
                assert_eq!(*attempt, 1);
                assert!(task.can_retry());
            }
            _ => panic!("Expected Retrying status"),
        }
    }

    #[test]
    fn test_task_fail_exhausts_retries() {
        let mut task = Task::new("test".to_string()).with_max_retries(2);

        task.retry_count = 2; // Already at max
        task.fail("error".to_string());

        match &task.status {
            TaskStatus::Failed { error } => {
                assert_eq!(error, "error");
                assert!(!task.can_retry());
            }
            _ => panic!("Expected Failed status"),
        }
    }

    #[test]
    fn test_task_result_success() {
        let id = TaskId::new();
        let result = TaskResult::success(id.clone(), "output".to_string(), 100);

        assert!(result.success);
        assert_eq!(result.output, Some("output".to_string()));
        assert_eq!(result.error, None);
        assert_eq!(result.duration_ms, 100);
    }

    #[test]
    fn test_task_result_failure() {
        let id = TaskId::new();
        let result = TaskResult::failure(id.clone(), "error".to_string(), 50);

        assert!(!result.success);
        assert_eq!(result.output, None);
        assert_eq!(result.error, Some("error".to_string()));
        assert_eq!(result.duration_ms, 50);
    }
}
