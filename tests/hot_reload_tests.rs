#[cfg(test)]
mod tests {
    use nuclaw::hot_reload_registry::HotReloadSkillRegistry;
    use nuclaw::skill_hot_reloader::{create_hot_reloader, SkillHotReloader};
    use nuclaw::skill_watcher::{SkillEvent, SkillWatcher};
    use nuclaw::skills::{Skill, SkillRegistry};
    use tempfile::TempDir;

    #[test]
    fn test_hot_reload_registry_integration() {
        let temp_dir = TempDir::new().unwrap();

        let skill_dir = temp_dir.path().join("test-skill");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(
            skill_dir.join("SKILL.md"),
            r#"---
name: test-skill
description: Test skill description
---

# Test Skill

This is a test skill content.
"#,
        )
        .unwrap();

        let registry = HotReloadSkillRegistry::new();
        let result = registry.load_from_directory(temp_dir.path());

        assert!(result.is_ok());
        assert!(registry.get("test-skill").is_some());

        let skill = registry.get("test-skill").unwrap();
        assert_eq!(skill.name, "test-skill");
        assert_eq!(skill.description, "Test skill description");
    }

    #[test]
    fn test_hot_reload_registry_reload() {
        let temp_dir = TempDir::new().unwrap();

        let skill_dir = temp_dir.path().join("reload-skill");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(
            skill_dir.join("SKILL.md"),
            r#"---
name: reload-skill
description: Version 1
---

Content V1
"#,
        )
        .unwrap();

        let registry = HotReloadSkillRegistry::new();
        registry.load_from_directory(temp_dir.path()).unwrap();

        let v1 = registry.get("reload-skill").unwrap();
        assert_eq!(v1.description, "Version 1");

        std::fs::write(
            skill_dir.join("SKILL.md"),
            r#"---
name: reload-skill
description: Version 2
---

Content V2
"#,
        )
        .unwrap();

        let _ = registry.reload_skill(&skill_dir);

        let v2 = registry.get("reload-skill").unwrap();
        assert_eq!(v2.description, "Version 2");
    }

    #[test]
    fn test_hot_reload_registry_unregister() {
        let temp_dir = TempDir::new().unwrap();

        let skill_dir = temp_dir.path().join("remove-skill");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(
            skill_dir.join("SKILL.md"),
            r#"---
name: remove-skill
description: To be removed
---

Content
"#,
        )
        .unwrap();

        let registry = HotReloadSkillRegistry::new();
        registry.load_from_directory(temp_dir.path()).unwrap();

        assert!(registry.get("remove-skill").is_some());

        registry.unregister("remove-skill");

        assert!(registry.get("remove-skill").is_none());
    }

    #[test]
    fn test_skill_watcher_integration() {
        let temp_dir = TempDir::new().unwrap();

        let skill_dir = temp_dir.path().join("watch-skill");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(
            skill_dir.join("SKILL.md"),
            r#"---
name: watch-skill
description: Watched skill
---

Content
"#,
        )
        .unwrap();

        let mut watcher = SkillWatcher::new().unwrap();
        let result = watcher.watch(temp_dir.path());

        assert!(result.is_ok());
    }

    #[test]
    fn test_skill_hot_reloader_integration() {
        let temp_dir = TempDir::new().unwrap();

        let skill_dir = temp_dir.path().join("reloader-skill");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(
            skill_dir.join("SKILL.md"),
            r#"---
name: reloader-skill
description: Hot reloaded skill
---

Content
"#,
        )
        .unwrap();

        let mut reloader = create_hot_reloader().unwrap();
        let result = reloader.start(temp_dir.path());

        assert!(result.is_ok());

        let registry = reloader.registry();
        assert!(registry.get("reloader-skill").is_some());
    }

    #[test]
    fn test_multiple_skills_integration() {
        let temp_dir = TempDir::new().unwrap();

        for i in 0..3 {
            let skill_dir = temp_dir.path().join(format!("skill-{}", i));
            std::fs::create_dir_all(&skill_dir).unwrap();
            std::fs::write(
                skill_dir.join("SKILL.md"),
                format!(
                    r#"---
name: skill-{0}
description: Skill number {0}
---

Content {0}
"#,
                    i
                ),
            )
            .unwrap();
        }

        let registry = HotReloadSkillRegistry::new();
        registry.load_from_directory(temp_dir.path()).unwrap();

        assert_eq!(registry.list().len(), 3);
        assert!(registry.get("skill-0").is_some());
        assert!(registry.get("skill-1").is_some());
        assert!(registry.get("skill-2").is_some());
    }

    #[test]
    fn test_skill_change_event_handling() {
        use nuclaw::skill_watcher::SkillChangeEvent;
        use std::path::PathBuf;

        let temp_dir = TempDir::new().unwrap();

        let skill_dir = temp_dir.path().join("event-skill");
        std::fs::create_dir_all(&skill_dir).unwrap();

        let registry = HotReloadSkillRegistry::new();

        let created_event = SkillChangeEvent {
            event: SkillEvent::Created("event-skill".to_string()),
            path: skill_dir.clone(),
        };

        std::fs::write(
            skill_dir.join("SKILL.md"),
            r#"---
name: event-skill
description: Created via event
---

Content
"#,
        )
        .unwrap();

        registry.handle_change_event(&created_event);

        assert!(registry.get("event-skill").is_some());

        let removed_event = SkillChangeEvent {
            event: SkillEvent::Removed("event-skill".to_string()),
            path: skill_dir,
        };

        registry.handle_change_event(&removed_event);

        assert!(registry.get("event-skill").is_none());
    }
}
