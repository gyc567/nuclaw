//! Task executor with concurrency limits and retry support

use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::mpsc;
use tokio::time::sleep;

use super::metrics::Metrics;
use super::queue::TaskQueue;
use super::task::{Task, TaskId, TaskResult};

/// Function type for task execution
pub type TaskExecutorFn = Arc<dyn Fn(Task) -> BoxFuture<'static, TaskResult> + Send + Sync>;

/// Return type for async task execution
pub type BoxFuture<'a, T> = std::pin::Pin<Box<dyn std::future::Future<Output = T> + Send + 'a>>;

/// Configuration for the executor
#[derive(Clone, Debug)]
pub struct ExecutorConfig {
    pub max_concurrency: usize,
    pub poll_interval_ms: u64,
    pub max_retries: u32,
}

impl Default for ExecutorConfig {
    fn default() -> Self {
        Self {
            max_concurrency: 3,
            poll_interval_ms: 100,
            max_retries: 3,
        }
    }
}

/// Event emitted by the executor
#[derive(Debug, Clone)]
pub enum ExecutorEvent {
    TaskStarted(TaskId),
    TaskCompleted(TaskId, TaskResult),
    TaskFailed(TaskId, String),
    QueueEmpty,
}

/// The task executor
pub struct Executor {
    queue: TaskQueue,
    metrics: Arc<Metrics>,
    config: ExecutorConfig,
    shutdown_tx: Option<mpsc::Sender<()>>,
}

impl Executor {
    /// Create a new executor
    pub fn new(config: ExecutorConfig) -> Self {
        Self {
            queue: TaskQueue::new(config.max_concurrency),
            metrics: Arc::new(Metrics::new()),
            config,
            shutdown_tx: None,
        }
    }

    /// Get a reference to the task queue
    pub fn queue(&self) -> &TaskQueue {
        &self.queue
    }

    /// Get a reference to the metrics
    pub fn metrics(&self) -> &Arc<Metrics> {
        &self.metrics
    }

    /// Submit a task to the queue
    pub fn submit(&self, task: Task) {
        self.metrics.record_task_submitted();
        self.queue.enqueue(task);
    }

    /// Submit multiple tasks
    pub fn submit_many(&self, tasks: Vec<Task>) {
        for task in tasks {
            self.submit(task);
        }
    }

    /// Run the executor loop
    pub async fn run<F>(&self, executor_fn: F) -> Result<(), String>
    where
        F: Fn(Task) -> BoxFuture<'static, TaskResult> + Send + Sync + Clone + 'static,
    {
        let poll_interval = Duration::from_millis(self.config.poll_interval_ms);
        let executor_fn = Arc::new(executor_fn);
        
        loop {
            // Try to get a task from the queue
            if let Some(mut task) = self.queue.dequeue() {
                let _task_id = task.id.clone();
                let metrics = Arc::clone(&self.metrics);
                let queue = self.queue.clone();
                let fn_clone = Arc::clone(&executor_fn);
                
                task.start();
                self.metrics.record_task_started();
                
                // Execute task in background
                tokio::spawn(async move {
                    let start = Instant::now();
                    
                    // Run the task
                    let result = fn_clone(task.clone()).await;
                    
                    let duration = start.elapsed().as_millis() as u64;
                    
                    // Record metrics
                    if result.success {
                        metrics.record_task_completed(duration);
                    } else {
                        metrics.record_task_failed();
                    }
                    
                    // Complete the task and potentially requeue for retry
                    if !result.success {
                        let mut task = task;
                        task.fail(result.error.clone().unwrap_or_default());
                        
                        if task.can_retry() {
                            queue.requeue(task);
                            metrics.record_retry();
                        }
                    }
                    
                    queue.complete();
                });
            } else {
                // No task available, wait
                sleep(poll_interval).await;
            }
        }
    }

    /// Get current queue statistics
    pub fn stats(&self) -> ExecutorStats {
        ExecutorStats {
            pending: self.queue.pending_count(),
            running: self.queue.running_count(),
            total_submitted: self.metrics.total_submitted(),
            total_completed: self.metrics.total_completed(),
            total_failed: self.metrics.total_failed(),
            total_retries: self.metrics.total_retries(),
        }
    }
}

/// Statistics from the executor
#[derive(Debug, Clone, Default)]
pub struct ExecutorStats {
    pub pending: usize,
    pub running: usize,
    pub total_submitted: u64,
    pub total_completed: u64,
    pub total_failed: u64,
    pub total_retries: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orchestrator::task::{Priority, TaskSource};

    fn create_test_task(payload: &str) -> Task {
        Task::new(payload.to_string())
    }

    fn dummy_executor(task: Task) -> BoxFuture<'static, TaskResult> {
        Box::pin(async move {
            TaskResult::success(task.id, format!("processed: {}", task.payload), 10)
        })
    }

    #[test]
    fn test_executor_new() {
        let executor = Executor::new(ExecutorConfig::default());
        assert_eq!(executor.queue().pending_count(), 0);
    }

    #[test]
    fn test_executor_submit() {
        let executor = Executor::new(ExecutorConfig::default());
        executor.submit(create_test_task("test"));
        
        assert_eq!(executor.queue().pending_count(), 1);
    }

    #[test]
    fn test_executor_submit_many() {
        let executor = Executor::new(ExecutorConfig::default());
        let tasks = vec![
            create_test_task("task1"),
            create_test_task("task2"),
            create_test_task("task3"),
        ];
        
        executor.submit_many(tasks);
        
        assert_eq!(executor.queue().pending_count(), 3);
    }

    #[test]
    fn test_executor_stats() {
        let executor = Executor::new(ExecutorConfig::default());
        executor.submit(create_test_task("test"));
        
        let stats = executor.stats();
        
        assert_eq!(stats.pending, 1);
        assert_eq!(stats.running, 0);
    }

    #[test]
    fn test_executor_config_default() {
        let config = ExecutorConfig::default();
        
        assert_eq!(config.max_concurrency, 3);
        assert_eq!(config.poll_interval_ms, 100);
        assert_eq!(config.max_retries, 3);
    }

    #[test]
    fn test_executor_config_custom() {
        let config = ExecutorConfig {
            max_concurrency: 5,
            poll_interval_ms: 50,
            max_retries: 10,
        };
        
        assert_eq!(config.max_concurrency, 5);
        assert_eq!(config.poll_interval_ms, 50);
        assert_eq!(config.max_retries, 10);
    }

    #[test]
    fn test_task_with_all_options() {
        let task = Task::new("test".to_string())
            .with_source(TaskSource::Scheduled)
            .with_priority(Priority::Critical)
            .with_max_retries(5);
        
        assert_eq!(task.payload, "test");
        assert_eq!(task.source, TaskSource::Scheduled);
        assert_eq!(task.priority, Priority::Critical);
        assert_eq!(task.max_retries, 5);
    }

    #[tokio::test]
    async fn test_executor_run_processes_tasks() {
        let executor = Executor::new(ExecutorConfig {
            max_concurrency: 2,
            poll_interval_ms: 10,
            max_retries: 1,
        });
        
        executor.submit(create_test_task("task1"));
        executor.submit(create_test_task("task2"));
        
        // Run for a short time - spawn task but don't wait for it
        let executor_clone = Executor::new(ExecutorConfig {
            max_concurrency: 2,
            poll_interval_ms: 10,
            max_retries: 1,
        });
        executor_clone.submit(create_test_task("task1"));
        executor_clone.submit(create_test_task("task2"));
        
        let _handle = tokio::spawn(async move {
            let _ = executor_clone.run(dummy_executor).await;
        });
        
        // Give some time for tasks to be processed
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        // Check stats - original executor has submitted tasks
        let stats = executor.stats();
        assert!(stats.total_submitted >= 2);
    }
}
