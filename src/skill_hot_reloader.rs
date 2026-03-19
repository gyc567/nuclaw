use std::path::Path;
use std::sync::Arc;
use std::sync::RwLock;

use crate::config::skills_dir;
use crate::hot_reload_registry::HotReloadSkillRegistry;
use crate::skill_watcher::{SkillWatcher, SkillWatcherError};

pub struct SkillHotReloader {
    registry: Arc<HotReloadSkillRegistry>,
    watcher: Option<SkillWatcher>,
    is_running: Arc<RwLock<bool>>,
}

impl SkillHotReloader {
    pub fn new() -> Result<Self, SkillWatcherError> {
        let registry = Arc::new(HotReloadSkillRegistry::new());
        let watcher = SkillWatcher::new()?;

        Ok(Self {
            registry,
            watcher: Some(watcher),
            is_running: Arc::new(RwLock::new(false)),
        })
    }

    pub fn registry(&self) -> Arc<HotReloadSkillRegistry> {
        Arc::clone(&self.registry)
    }

    pub fn start(&mut self, path: &Path) -> Result<(), SkillWatcherError> {
        if let Some(ref mut watcher) = self.watcher {
            watcher.watch(path)?;
        }

        self.registry
            .load_from_directory(path)
            .map_err(|e| SkillWatcherError::WatchPath(e.to_string()))?;

        *self.is_running.write().unwrap() = true;

        Ok(())
    }

    pub fn start_with_default_path(&mut self) -> Result<(), SkillWatcherError> {
        let path = skills_dir();
        self.start(&path)
    }

    pub fn is_running(&self) -> bool {
        *self.is_running.read().unwrap()
    }

    pub fn stop(&self) {
        *self.is_running.write().unwrap() = false;
    }

    pub fn take_watcher(&mut self) -> Option<SkillWatcher> {
        self.watcher.take()
    }
}

impl Default for SkillHotReloader {
    fn default() -> Self {
        Self::new().expect("Failed to create skill hot reloader")
    }
}

pub fn create_hot_reloader() -> Result<SkillHotReloader, SkillWatcherError> {
    SkillHotReloader::new()
}

pub fn init_hot_reload() -> Result<SkillHotReloader, SkillWatcherError> {
    let mut reloader = SkillHotReloader::new()?;
    reloader.start_with_default_path()?;
    Ok(reloader)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skills::SkillRegistry;
    use tempfile::TempDir;

    #[test]
    fn test_skill_hot_reloader_new() {
        let reloader = SkillHotReloader::new();
        assert!(reloader.is_ok());
    }

    #[test]
    fn test_skill_hot_reloader_default() {
        let reloader = SkillHotReloader::default();
        assert!(reloader.registry().list().is_empty());
    }

    #[test]
    fn test_create_hot_reloader() {
        let reloader = create_hot_reloader();
        assert!(reloader.is_ok());
    }

    #[test]
    fn test_hot_reloader_start_with_temp_dir() {
        let temp_dir = TempDir::new().unwrap();
        let mut reloader = SkillHotReloader::new().unwrap();

        let result = reloader.start(temp_dir.path());
        assert!(result.is_ok());
    }

    #[test]
    fn test_hot_reloader_registry() {
        let reloader = SkillHotReloader::new().unwrap();
        let registry = reloader.registry();

        assert!(registry.list().is_empty());
    }

    #[test]
    fn test_hot_reloader_is_running() {
        let reloader = SkillHotReloader::new().unwrap();
        assert!(!reloader.is_running());
    }

    #[test]
    fn test_hot_reloader_stop() {
        let reloader = SkillHotReloader::new().unwrap();
        reloader.stop();
        assert!(!reloader.is_running());
    }
}
