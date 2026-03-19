use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, RwLock};
use tokio::sync::mpsc;

use crate::config::skills_dir;
use crate::skill_watcher::{SkillChangeEvent, SkillEvent};
use crate::skills::{Skill, SkillRegistry};

pub struct HotReloadSkillRegistry {
    inner: RwLock<HashMap<String, Arc<Skill>>>,
}

impl HotReloadSkillRegistry {
    pub fn new() -> Self {
        Self {
            inner: RwLock::new(HashMap::new()),
        }
    }

    pub fn load_from_directory(&self, dir: &Path) -> std::io::Result<()> {
        if !dir.is_dir() {
            return Ok(());
        }

        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                if let Some(skill) = Skill::from_directory(&path) {
                    self.register(skill);
                }
            }
        }
        Ok(())
    }

    pub fn register(&self, skill: Skill) {
        let name = skill.name.clone();
        let arc_skill = Arc::new(skill);
        self.inner.write().unwrap().insert(name, arc_skill);
    }

    pub fn unregister(&self, name: &str) {
        self.inner.write().unwrap().remove(name);
    }

    pub fn reload_skill(&self, path: &Path) -> std::io::Result<()> {
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            self.unregister(name);
            if let Some(skill) = Skill::from_directory(path) {
                self.register(skill);
            }
        }
        Ok(())
    }

    pub fn handle_change_event(&self, event: &SkillChangeEvent) {
        match event.event {
            SkillEvent::Created(_) | SkillEvent::Modified(_) => {
                let _ = self.reload_skill(&event.path);
            }
            SkillEvent::Removed(ref name) => {
                self.unregister(name);
            }
        }
    }

    pub async fn start_watcher(&self) -> mpsc::Receiver<SkillChangeEvent> {
        let (tx, rx) = mpsc::channel(100);
        
        let dir = skills_dir();
        if let Ok(_) = self.load_from_directory(&dir) {
            tracing::info!("Loaded skills from {:?}", dir);
        }

        tokio::spawn(async move {
            let _ = tx;
        });

        rx
    }
}

impl Default for HotReloadSkillRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl SkillRegistry for HotReloadSkillRegistry {
    fn get(&self, name: &str) -> Option<Arc<Skill>> {
        self.inner.read().unwrap().get(name).cloned()
    }

    fn list(&self) -> Vec<Arc<Skill>> {
        self.inner.read().unwrap().values().cloned().collect()
    }

    fn names(&self) -> Vec<String> {
        self.inner.read().unwrap().keys().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_hot_reload_registry_new() {
        let registry = HotReloadSkillRegistry::new();
        assert!(registry.list().is_empty());
    }

    #[test]
    fn test_hot_reload_registry_register() {
        let registry = HotReloadSkillRegistry::new();
        let skill = Skill::new("test-skill", "Test description", "Test content");
        
        registry.register(skill);
        
        assert!(registry.get("test-skill").is_some());
        assert_eq!(registry.names(), vec!["test-skill"]);
    }

    #[test]
    fn test_hot_reload_registry_unregister() {
        let registry = HotReloadSkillRegistry::new();
        let skill = Skill::new("test-skill", "Test description", "Test content");
        
        registry.register(skill);
        assert!(registry.get("test-skill").is_some());
        
        registry.unregister("test-skill");
        assert!(registry.get("test-skill").is_none());
    }

    #[test]
    fn test_hot_reload_registry_update() {
        let registry = HotReloadSkillRegistry::new();
        
        let skill_v1 = Skill::new("test-skill", "V1 description", "V1 content");
        registry.register(skill_v1);
        
        let skill_v2 = Skill::new("test-skill", "V2 description", "V2 content");
        registry.register(skill_v2);
        
        let retrieved = registry.get("test-skill").unwrap();
        assert_eq!(retrieved.description, "V2 description");
    }

    #[test]
    fn test_hot_reload_registry_load_from_directory() {
        let temp_dir = TempDir::new().unwrap();
        let skill_dir = temp_dir.path().join("test-skill");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(skill_dir.join("SKILL.md"), "# Test\n\nContent").unwrap();
        
        let registry = HotReloadSkillRegistry::new();
        let result = registry.load_from_directory(temp_dir.path());
        
        assert!(result.is_ok());
    }
}
