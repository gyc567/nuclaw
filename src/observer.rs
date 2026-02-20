use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl LogLevel {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "trace" => LogLevel::Trace,
            "debug" => LogLevel::Debug,
            "info" => LogLevel::Info,
            "warn" | "warning" => LogLevel::Warn,
            "error" => LogLevel::Error,
            _ => LogLevel::Info,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub level: LogLevel,
    pub target: String,
    pub message: String,
    pub timestamp: String,
    pub fields: Option<serde_json::Value>,
}

impl LogEntry {
    pub fn new(level: LogLevel, target: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            level,
            target: target.into(),
            message: message.into(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            fields: None,
        }
    }

    pub fn with_fields(mut self, fields: serde_json::Value) -> Self {
        self.fields = Some(fields);
        self
    }
}

#[async_trait]
pub trait Observer: Send + Sync {
    fn name(&self) -> &str;
    async fn observe(&self, entry: LogEntry);
    async fn flush(&self) -> Result<(), String>;
}

pub struct NoopObserver;

#[async_trait]
impl Observer for NoopObserver {
    fn name(&self) -> &str {
        "noop"
    }

    async fn observe(&self, _entry: LogEntry) {}

    async fn flush(&self) -> Result<(), String> {
        Ok(())
    }
}

pub struct LogObserver {
    min_level: LogLevel,
}

impl LogObserver {
    pub fn new(min_level: LogLevel) -> Self {
        Self { min_level }
    }

    fn should_log(&self, level: &LogLevel) -> bool {
        let level_value = match level {
            LogLevel::Trace => 0,
            LogLevel::Debug => 1,
            LogLevel::Info => 2,
            LogLevel::Warn => 3,
            LogLevel::Error => 4,
        };
        
        let min_value = match self.min_level {
            LogLevel::Trace => 0,
            LogLevel::Debug => 1,
            LogLevel::Info => 2,
            LogLevel::Warn => 3,
            LogLevel::Error => 4,
        };
        
        level_value >= min_value
    }
}

#[async_trait]
impl Observer for LogObserver {
    fn name(&self) -> &str {
        "log"
    }

    async fn observe(&self, entry: LogEntry) {
        if !self.should_log(&entry.level) {
            return;
        }

        let target = if entry.target.is_empty() {
            "nuclaw".to_string()
        } else {
            entry.target.clone()
        };

        match entry.level {
            LogLevel::Trace => tracing::trace!(target = %target, "{}", entry.message),
            LogLevel::Debug => tracing::debug!(target = %target, "{}", entry.message),
            LogLevel::Info => tracing::info!(target = %target, "{}", entry.message),
            LogLevel::Warn => tracing::warn!(target = %target, "{}", entry.message),
            LogLevel::Error => tracing::error!(target = %target, "{}", entry.message),
        }
    }

    async fn flush(&self) -> Result<(), String> {
        Ok(())
    }
}

pub struct MultiObserver {
    observers: Vec<Arc<dyn Observer>>,
}

impl MultiObserver {
    pub fn new() -> Self {
        Self {
            observers: Vec::new(),
        }
    }

    pub fn add(&mut self, observer: Arc<dyn Observer>) {
        self.observers.push(observer);
    }
}

#[async_trait]
impl Observer for MultiObserver {
    fn name(&self) -> &str {
        "multi"
    }

    async fn observe(&self, entry: LogEntry) {
        for observer in &self.observers {
            observer.observe(entry.clone()).await;
        }
    }

    async fn flush(&self) -> Result<(), String> {
        for observer in &self.observers {
            observer.flush().await?;
        }
        Ok(())
    }
}

impl Default for MultiObserver {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_level_from_str() {
        assert_eq!(LogLevel::from_str("debug"), LogLevel::Debug);
        assert_eq!(LogLevel::from_str("INFO"), LogLevel::Info);
        assert_eq!(LogLevel::from_str("warning"), LogLevel::Warn);
        assert_eq!(LogLevel::from_str("ERROR"), LogLevel::Error);
    }

    #[test]
    #[ignore]
    fn test_log_entry_new() {
        let entry = LogEntry::new(LogLevel::Info, "test", "message");
        assert!(matches!(entry.level, LogLevel::Info));
        assert_eq!(entry.target, "test");
        assert_eq!(entry.message, "message");
        assert!(entry.timestamp.contains("Z"));
    }

    #[test]
    fn test_log_entry_with_fields() {
        let entry = LogEntry::new(LogLevel::Info, "test", "message")
            .with_fields(serde_json::json!({"key": "value"}));
        
        assert!(entry.fields.is_some());
    }

    #[test]
    fn test_noop_observer_name() {
        let observer = NoopObserver;
        assert_eq!(observer.name(), "noop");
    }

    #[tokio::test]
    async fn test_noop_observer_observe() {
        let observer = NoopObserver;
        let entry = LogEntry::new(LogLevel::Info, "test", "message");
        observer.observe(entry).await;
    }

    #[tokio::test]
    async fn test_noop_observer_flush() {
        let observer = NoopObserver;
        assert!(observer.flush().await.is_ok());
    }

    #[test]
    fn test_log_observer_new() {
        let observer = LogObserver::new(LogLevel::Info);
        assert_eq!(observer.name(), "log");
    }

    #[test]
    fn test_log_observer_should_log() {
        let observer = LogObserver::new(LogLevel::Info);
        
        assert!(!observer.should_log(&LogLevel::Trace));
        assert!(!observer.should_log(&LogLevel::Debug));
        assert!(observer.should_log(&LogLevel::Info));
        assert!(observer.should_log(&LogLevel::Warn));
        assert!(observer.should_log(&LogLevel::Error));
    }

    #[tokio::test]
    async fn test_multi_observer_new() {
        let observer = MultiObserver::new();
        assert_eq!(observer.name(), "multi");
    }

    #[tokio::test]
    async fn test_multi_observer_add() {
        let mut observer = MultiObserver::new();
        observer.add(Arc::new(NoopObserver));
        assert!(!observer.observers.is_empty());
    }
}
