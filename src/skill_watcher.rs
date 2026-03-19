//! Skill Hot Reload Watcher
//!
//! Monitors the skills directory for changes and triggers reloads

use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver};
use std::sync::{Arc, RwLock};
use std::time::Duration;

/// Skill change event type
#[derive(Debug, Clone, PartialEq)]
pub enum SkillEvent {
    /// A new skill was created
    Created(String),
    /// A skill was modified
    Modified(String),
    /// A skill was removed
    Removed(String),
}

/// Skill change event with path
#[derive(Debug, Clone)]
pub struct SkillChangeEvent {
    pub event: SkillEvent,
    pub path: PathBuf,
}

/// Skill watcher for hot reload
pub struct SkillWatcher {
    watcher: Option<RecommendedWatcher>,
    events: Arc<RwLock<Receiver<SkillChangeEvent>>>,
    watched_skills: Arc<RwLock<HashMap<String, PathBuf>>>,
}

/// Error type for skill watcher
#[derive(Debug, thiserror::Error)]
pub enum SkillWatcherError {
    #[error("Failed to create watcher: {0}")]
    WatcherCreation(String),

    #[error("Failed to watch path: {0}")]
    WatchPath(String),

    #[error("Path is not a directory: {0}")]
    NotDirectory(PathBuf),

    #[error("Skill watcher is not running")]
    NotRunning,
}

impl SkillWatcher {
    /// Create a new skill watcher (doesn't start watching until start() is called)
    pub fn new() -> Result<Self, SkillWatcherError> {
        let (tx, rx) = channel();

        let events = Arc::new(RwLock::new(rx));
        let watched_skills = Arc::new(RwLock::new(HashMap::new()));

        // Create watcher with event handler
        let watcher = RecommendedWatcher::new(
            move |res: Result<Event, notify::Error>| {
                if let Ok(event) = res {
                    if let Some(change_event) = process_event(event) {
                        let _ = tx.send(change_event);
                    }
                }
            },
            Config::default().with_poll_interval(Duration::from_secs(2)),
        )
        .map_err(|e| SkillWatcherError::WatcherCreation(e.to_string()))?;

        Ok(Self {
            watcher: Some(watcher),
            events,
            watched_skills,
        })
    }

    /// Start watching a skills directory
    pub fn watch(&mut self, path: &Path) -> Result<(), SkillWatcherError> {
        if !path.is_dir() {
            return Err(SkillWatcherError::NotDirectory(path.to_path_buf()));
        }

        if let Some(ref mut watcher) = self.watcher {
            watcher
                .watch(path, RecursiveMode::Recursive)
                .map_err(|e| SkillWatcherError::WatchPath(e.to_string()))?;
        }

        // Index existing skills
        if let Ok(entries) = std::fs::read_dir(path) {
            let mut skills = self.watched_skills.write().unwrap();
            for entry in entries.flatten() {
                let entry_path = entry.path();
                if entry_path.is_dir() {
                    let name = entry_path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("")
                        .to_string();
                    skills.insert(name, entry_path);
                }
            }
        }

        Ok(())
    }

    /// Take the watcher (for running in background)
    pub fn take_watcher(&mut self) -> Option<RecommendedWatcher> {
        self.watcher.take()
    }

    /// Get the receiver for skill change events
    pub fn events(&self) -> Arc<RwLock<Receiver<SkillChangeEvent>>> {
        Arc::clone(&self.events)
    }

    /// Get list of watched skills
    pub fn watched_skills(&self) -> Vec<String> {
        self.watched_skills
            .read()
            .unwrap()
            .keys()
            .cloned()
            .collect()
    }

    /// Check if a path is a skill directory
    #[allow(dead_code)]
    fn is_skill_path(path: &Path) -> bool {
        path.join("SKILL.md").exists()
    }

    /// Get skill name from path
    fn get_skill_name(path: &Path) -> Option<String> {
        path.file_name()
            .and_then(|n| n.to_str())
            .map(|s| s.to_string())
    }
}

impl Default for SkillWatcher {
    fn default() -> Self {
        Self::new().expect("Failed to create default skill watcher")
    }
}

/// Process a notify event into a SkillChangeEvent
fn process_event(event: Event) -> Option<SkillChangeEvent> {
    use notify::EventKind;

    for path in event.paths {
        // Only care about SKILL.md files
        if !path.file_name().map(|n| n == "SKILL.md").unwrap_or(false) {
            continue;
        }

        // Get parent directory (the skill directory)
        let skill_dir = path.parent()?;
        if !skill_dir.is_dir() {
            continue;
        }

        let skill_name = SkillWatcher::get_skill_name(skill_dir)?;

        let skill_event = match event.kind {
            EventKind::Create(_) => SkillEvent::Created(skill_name),
            EventKind::Modify(_) => SkillEvent::Modified(skill_name),
            EventKind::Remove(_) => SkillEvent::Removed(skill_name),
            _ => continue,
        };

        return Some(SkillChangeEvent {
            event: skill_event,
            path: skill_dir.to_path_buf(),
        });
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_skill_watcher_new() {
        let watcher = SkillWatcher::new();
        assert!(watcher.is_ok());
    }

    #[test]
    fn test_skill_watcher_watch_directory() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path();

        let mut watcher = SkillWatcher::new().unwrap();
        let result = watcher.watch(path);

        assert!(result.is_ok());
    }

    #[test]
    fn test_skill_watcher_watch_non_directory() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        fs::write(&file_path, "test").unwrap();

        let mut watcher = SkillWatcher::new().unwrap();
        let result = watcher.watch(&file_path);

        assert!(result.is_err());
    }

    #[test]
    fn test_is_skill_path() {
        let temp_dir = TempDir::new().unwrap();
        let skill_dir = temp_dir.path().join("my-skill");
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(skill_dir.join("SKILL.md"), "# My Skill").unwrap();

        assert!(SkillWatcher::is_skill_path(&skill_dir));
    }

    #[test]
    fn test_is_skill_path_not_skill() {
        let temp_dir = TempDir::new().unwrap();
        let dir = temp_dir.path().join("not-a-skill");
        fs::create_dir_all(&dir).unwrap();

        assert!(!SkillWatcher::is_skill_path(&dir));
    }

    #[test]
    fn test_get_skill_name() {
        let path = PathBuf::from("/skills/my-skill");
        let name = SkillWatcher::get_skill_name(&path);

        assert_eq!(name, Some("my-skill".to_string()));
    }

    // Note: process_event tests require stable notify API
    // Testing the public interface through SkillWatcher::watch() instead
}
