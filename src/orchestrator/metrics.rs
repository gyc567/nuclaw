//! Metrics collection for task execution

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Metrics collector for task execution
#[derive(Clone)]
pub struct Metrics {
    submitted: Arc<AtomicU64>,
    started: Arc<AtomicU64>,
    completed: Arc<AtomicU64>,
    failed: Arc<AtomicU64>,
    retries: Arc<AtomicU64>,
    total_duration_ms: Arc<AtomicU64>,
    start_time: Instant,
}

impl Metrics {
    /// Create new metrics
    pub fn new() -> Self {
        Self {
            submitted: Arc::new(AtomicU64::new(0)),
            started: Arc::new(AtomicU64::new(0)),
            completed: Arc::new(AtomicU64::new(0)),
            failed: Arc::new(AtomicU64::new(0)),
            retries: Arc::new(AtomicU64::new(0)),
            total_duration_ms: Arc::new(AtomicU64::new(0)),
            start_time: Instant::now(),
        }
    }

    /// Record a task was submitted
    pub fn record_task_submitted(&self) {
        self.submitted.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a task started
    pub fn record_task_started(&self) {
        self.started.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a task completed
    pub fn record_task_completed(&self, duration_ms: u64) {
        self.completed.fetch_add(1, Ordering::Relaxed);
        self.total_duration_ms
            .fetch_add(duration_ms, Ordering::Relaxed);
    }

    /// Record a task failed
    pub fn record_task_failed(&self) {
        self.failed.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a retry
    pub fn record_retry(&self) {
        self.retries.fetch_add(1, Ordering::Relaxed);
    }

    /// Get total submitted tasks
    pub fn total_submitted(&self) -> u64 {
        self.submitted.load(Ordering::Relaxed)
    }

    /// Get total started tasks
    pub fn total_started(&self) -> u64 {
        self.started.load(Ordering::Relaxed)
    }

    /// Get total completed tasks
    pub fn total_completed(&self) -> u64 {
        self.completed.load(Ordering::Relaxed)
    }

    /// Get total failed tasks
    pub fn total_failed(&self) -> u64 {
        self.failed.load(Ordering::Relaxed)
    }

    /// Get total retries
    pub fn total_retries(&self) -> u64 {
        self.retries.load(Ordering::Relaxed)
    }

    /// Get total execution time in milliseconds
    pub fn total_duration_ms(&self) -> u64 {
        self.total_duration_ms.load(Ordering::Relaxed)
    }

    /// Get average task duration in milliseconds
    pub fn avg_duration_ms(&self) -> u64 {
        let completed = self.total_completed();
        if completed == 0 {
            return 0;
        }
        self.total_duration_ms() / completed
    }

    /// Get uptime duration
    pub fn uptime(&self) -> Duration {
        self.start_time.elapsed()
    }

    /// Get success rate (0.0 to 1.0)
    pub fn success_rate(&self) -> f64 {
        let completed = self.total_completed();
        let failed = self.total_failed();
        let total = completed + failed;

        if total == 0 {
            return 1.0;
        }

        completed as f64 / total as f64
    }

    /// Get throughput (tasks per second)
    pub fn throughput(&self) -> f64 {
        let uptime_secs = self.uptime().as_secs_f64();
        if uptime_secs == 0.0 {
            return 0.0;
        }

        let total = self.total_completed() + self.total_failed();
        total as f64 / uptime_secs
    }

    /// Get all metrics as a snapshot
    pub fn snapshot(&self) -> MetricsSnapshot {
        MetricsSnapshot {
            submitted: self.total_submitted(),
            started: self.total_started(),
            completed: self.total_completed(),
            failed: self.total_failed(),
            retries: self.total_retries(),
            total_duration_ms: self.total_duration_ms(),
            avg_duration_ms: self.avg_duration_ms(),
            uptime_ms: self.uptime().as_millis() as u64,
            success_rate: self.success_rate(),
            throughput: self.throughput(),
        }
    }
}

impl Default for Metrics {
    fn default() -> Self {
        Self::new()
    }
}

/// Snapshot of metrics at a point in time
#[derive(Debug, Clone)]
pub struct MetricsSnapshot {
    pub submitted: u64,
    pub started: u64,
    pub completed: u64,
    pub failed: u64,
    pub retries: u64,
    pub total_duration_ms: u64,
    pub avg_duration_ms: u64,
    pub uptime_ms: u64,
    pub success_rate: f64,
    pub throughput: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_new() {
        let metrics = Metrics::new();

        assert_eq!(metrics.total_submitted(), 0);
        assert_eq!(metrics.total_started(), 0);
        assert_eq!(metrics.total_completed(), 0);
        assert_eq!(metrics.total_failed(), 0);
        assert_eq!(metrics.total_retries(), 0);
    }

    #[test]
    fn test_metrics_record_submitted() {
        let metrics = Metrics::new();

        metrics.record_task_submitted();
        metrics.record_task_submitted();
        metrics.record_task_submitted();

        assert_eq!(metrics.total_submitted(), 3);
    }

    #[test]
    fn test_metrics_record_started() {
        let metrics = Metrics::new();

        metrics.record_task_started();
        metrics.record_task_started();

        assert_eq!(metrics.total_started(), 2);
    }

    #[test]
    fn test_metrics_record_completed() {
        let metrics = Metrics::new();

        metrics.record_task_started();
        metrics.record_task_completed(100);
        metrics.record_task_started();
        metrics.record_task_completed(200);

        assert_eq!(metrics.total_completed(), 2);
        assert_eq!(metrics.total_duration_ms(), 300);
        assert_eq!(metrics.avg_duration_ms(), 150);
    }

    #[test]
    fn test_metrics_record_failed() {
        let metrics = Metrics::new();

        metrics.record_task_started();
        metrics.record_task_failed();
        metrics.record_task_started();
        metrics.record_task_failed();

        assert_eq!(metrics.total_failed(), 2);
    }

    #[test]
    fn test_metrics_record_retry() {
        let metrics = Metrics::new();

        metrics.record_retry();
        metrics.record_retry();
        metrics.record_retry();

        assert_eq!(metrics.total_retries(), 3);
    }

    #[test]
    fn test_metrics_success_rate() {
        let metrics = Metrics::new();

        // No tasks yet - should be 1.0
        assert_eq!(metrics.success_rate(), 1.0);

        // Some completed, no failures
        metrics.record_task_started();
        metrics.record_task_completed(100);
        assert_eq!(metrics.success_rate(), 1.0);

        // Add a failure
        metrics.record_task_started();
        metrics.record_task_failed();

        // 1 completed, 1 failed = 0.5
        assert!((metrics.success_rate() - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_metrics_avg_duration() {
        let metrics = Metrics::new();

        // No completed tasks
        assert_eq!(metrics.avg_duration_ms(), 0);

        // Add some completed tasks
        metrics.record_task_started();
        metrics.record_task_completed(100);
        metrics.record_task_started();
        metrics.record_task_completed(200);
        metrics.record_task_started();
        metrics.record_task_completed(300);

        assert_eq!(metrics.avg_duration_ms(), 200);
    }

    #[test]
    fn test_metrics_throughput() {
        let metrics = Metrics::new();

        // No tasks - throughput should be 0
        let initial = metrics.throughput();
        assert_eq!(initial, 0.0);

        // Add some tasks
        metrics.record_task_started();
        metrics.record_task_completed(100);
        metrics.record_task_started();
        metrics.record_task_failed();

        // After some time, throughput should be calculable
        std::thread::sleep(Duration::from_millis(10));

        let throughput = metrics.throughput();
        assert!(throughput > 0.0);
    }

    #[test]
    fn test_metrics_snapshot() {
        let metrics = Metrics::new();

        metrics.record_task_submitted();
        metrics.record_task_started();
        metrics.record_task_completed(100);
        metrics.record_task_started();
        metrics.record_task_failed();
        metrics.record_retry();

        // Small delay to ensure uptime > 0
        std::thread::sleep(Duration::from_millis(1));

        let snapshot = metrics.snapshot();

        assert_eq!(snapshot.submitted, 1);
        assert_eq!(snapshot.started, 2);
        assert_eq!(snapshot.completed, 1);
        assert_eq!(snapshot.failed, 1);
        assert_eq!(snapshot.retries, 1);
        assert_eq!(snapshot.total_duration_ms, 100);
        assert_eq!(snapshot.avg_duration_ms, 100);
        assert!(snapshot.uptime_ms > 0);
    }

    #[test]
    fn test_metrics_uptime() {
        let metrics = Metrics::new();

        std::thread::sleep(Duration::from_millis(10));

        let uptime = metrics.uptime();
        assert!(uptime.as_millis() >= 10);
    }

    #[test]
    fn test_metrics_clone() {
        let metrics = Metrics::new();
        metrics.record_task_submitted();

        let cloned = metrics.clone();

        assert_eq!(cloned.total_submitted(), 1);
    }
}
