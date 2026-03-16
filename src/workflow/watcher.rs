//! WORKFLOW.md Hot Reload Watcher
//!
//! This module provides file system watching for WORKFLOW.md files,
//! enabling hot reload when workflow configurations change.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use notify::{Config, Error as NotifyError, Event, RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};

use crate::error::NuClawError;

type NotifyResult = std::result::Result<Event, NotifyError>;
type NotifyReceiver = Receiver<NotifyResult>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WatchEvent {
    Modified(PathBuf),
    Created(PathBuf),
    Removed(PathBuf),
}

pub struct WorkflowWatcher {
    path: PathBuf,
    watcher: Arc<Mutex<Option<RecommendedWatcher>>>,
    receiver: Arc<Mutex<Option<NotifyReceiver>>>,
    running: bool,
    debounce: Duration,
}

impl Clone for WorkflowWatcher {
    fn clone(&self) -> Self {
        WorkflowWatcher {
            path: self.path.clone(),
            watcher: Arc::clone(&self.watcher),
            receiver: Arc::clone(&self.receiver),
            running: self.running,
            debounce: self.debounce,
        }
    }
}

impl std::fmt::Debug for WorkflowWatcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WorkflowWatcher")
            .field("path", &self.path)
            .field("running", &self.running)
            .field("debounce", &self.debounce)
            .finish()
    }
}

impl WorkflowWatcher {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        WorkflowWatcher {
            path: path.into(),
            watcher: Arc::new(Mutex::new(None)),
            receiver: Arc::new(Mutex::new(None)),
            running: false,
            debounce: Duration::from_millis(300),
        }
    }

    pub fn start(&mut self) -> Result<(), NuClawError> {
        if self.running {
            return Ok(());
        }

        let path = self.path.clone();
        let (tx, rx) = channel();

        let mut watcher = RecommendedWatcher::new(
            move |res: std::result::Result<Event, NotifyError>| {
                let _ = tx.send(res);
            },
            Config::default().with_poll_interval(self.debounce),
        )
        .map_err(|e| NuClawError::FileSystem {
            message: format!("Failed to create watcher: {}", e),
        })?;

        watcher
            .watch(&path, RecursiveMode::NonRecursive)
            .map_err(|e| NuClawError::FileSystem {
                message: format!("Failed to watch path: {}", e),
            })?;

        let mut watcher_guard = self.watcher.lock().map_err(|e| NuClawError::FileSystem {
            message: format!("Failed to lock watcher: {}", e),
        })?;
        *watcher_guard = Some(watcher);

        let mut receiver_guard = self.receiver.lock().map_err(|e| NuClawError::FileSystem {
            message: format!("Failed to lock receiver: {}", e),
        })?;
        *receiver_guard = Some(rx);
        self.running = true;
        Ok(())
    }

    pub fn stop(&mut self) -> Result<(), NuClawError> {
        if let Ok(mut guard) = self.watcher.lock() {
            if let Some(ref mut w) = *guard {
                let _ = w.unwatch(&self.path);
            }
            *guard = None;
        }
        if let Ok(mut guard) = self.receiver.lock() {
            *guard = None;
        }
        self.running = false;
        Ok(())
    }

    pub fn is_running(&self) -> bool {
        self.running
    }

    pub fn watch_path(&self) -> &Path {
        &self.path
    }

    pub fn poll_events(&self) -> Vec<WatchEvent> {
        let mut events = Vec::new();

        if let Ok(guard) = self.receiver.lock() {
            if let Some(ref rx) = *guard {
                while let Ok(Ok(event)) = rx.try_recv() {
                    for path in event.paths {
                        let watch_event = match event.kind {
                            notify::EventKind::Create(_) => Some(WatchEvent::Created(path)),
                            notify::EventKind::Modify(_) => Some(WatchEvent::Modified(path)),
                            notify::EventKind::Remove(_) => Some(WatchEvent::Removed(path)),
                            notify::EventKind::Any => {
                                if path.exists() {
                                    Some(WatchEvent::Modified(path))
                                } else {
                                    Some(WatchEvent::Removed(path))
                                }
                            }
                            _ => None,
                        };

                        if let Some(we) = watch_event {
                            events.push(we);
                        }
                    }
                }
            }
        }

        events
    }

    pub fn set_debounce_duration(&mut self, duration: Duration) {
        self.debounce = duration;
    }
}

impl Default for WorkflowWatcher {
    fn default() -> Self {
        Self::new("WORKFLOW.md")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_watcher_new_creates_instance() {
        let watcher = WorkflowWatcher::new("test.md");
        assert!(!watcher.is_running());
    }

    #[test]
    fn test_watcher_with_path() {
        let watcher = WorkflowWatcher::new("/path/to/workflow.md");
        assert_eq!(watcher.watch_path(), Path::new("/path/to/workflow.md"));
    }

    #[test]
    fn test_watcher_default_path() {
        let watcher = WorkflowWatcher::default();
        assert_eq!(watcher.watch_path(), Path::new("WORKFLOW.md"));
    }

    #[test]
    fn test_watcher_stop_clears_running() {
        let mut watcher = WorkflowWatcher::new("test.md");
        let _ = watcher.stop();
        assert!(!watcher.running);
    }

    #[test]
    fn test_watcher_poll_events_returns_empty_initially() {
        let watcher = WorkflowWatcher::new("test.md");
        let events = watcher.poll_events();
        assert!(events.is_empty());
    }

    #[test]
    fn test_watcher_set_debounce_duration() {
        let mut watcher = WorkflowWatcher::new("test.md");
        watcher.set_debounce_duration(Duration::from_millis(500));
    }

    #[test]
    fn test_watch_event_modified() {
        let event = WatchEvent::Modified(PathBuf::from("test.md"));
        match event {
            WatchEvent::Modified(p) => assert_eq!(p, PathBuf::from("test.md")),
            _ => panic!("Expected Modified event"),
        }
    }

    #[test]
    fn test_watch_event_created() {
        let event = WatchEvent::Created(PathBuf::from("new.md"));
        match event {
            WatchEvent::Created(p) => assert_eq!(p, PathBuf::from("new.md")),
            _ => panic!("Expected Created event"),
        }
    }

    #[test]
    fn test_watch_event_removed() {
        let event = WatchEvent::Removed(PathBuf::from("deleted.md"));
        match event {
            WatchEvent::Removed(p) => assert_eq!(p, PathBuf::from("deleted.md")),
            _ => panic!("Expected Removed event"),
        }
    }

    #[test]
    fn test_watcher_clone_is_independent() {
        let watcher1 = WorkflowWatcher::new("test.md");
        let watcher2 = watcher1.clone();
        assert!(!watcher2.running);
    }

    #[test]
    fn test_watcher_debug_format() {
        let watcher = WorkflowWatcher::new("test.md");
        let debug_str = format!("{:?}", watcher);
        assert!(debug_str.contains("WorkflowWatcher"));
    }

    #[test]
    fn test_watcher_path_conversion() {
        let watcher = WorkflowWatcher::new("workflow.md");
        let path = watcher.watch_path();
        assert_eq!(path.to_string_lossy(), "workflow.md");
    }

    #[test]
    fn test_watcher_default_debounce() {
        let watcher = WorkflowWatcher::new("test.md");
        assert_eq!(watcher.debounce, Duration::from_millis(300));
    }

    #[test]
    fn test_watcher_clone_preserves_path() {
        let watcher1 = WorkflowWatcher::new("test.md");
        let watcher2 = watcher1.clone();
        assert_eq!(watcher1.watch_path(), watcher2.watch_path());
    }

    #[test]
    fn test_watcher_start_stop_idempotent() {
        let mut watcher = WorkflowWatcher::new("test.md");
        let _ = watcher.stop();
        assert!(!watcher.is_running());
        let _ = watcher.stop();
        assert!(!watcher.is_running());
    }
}
