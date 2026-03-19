#[cfg(test)]
mod e2e_tests {
    use nuclaw::hot_reload_registry::HotReloadSkillRegistry;
    use nuclaw::skill_hot_reloader::SkillHotReloader;
    use nuclaw::skill_to_rig::{all_skills_to_tools, skills_to_tools, SkillAsTool};
    use nuclaw::skill_watcher::SkillWatcher;
    use nuclaw::skills::{Skill, SkillRegistry, SkillType};
    use nuclaw::tool_registry::{InMemoryToolRegistry, Tool, ToolDefinition, ToolRegistry};
    use nuclaw::wasm_executor::WasmExecutor;
    use std::sync::Arc;
    use tempfile::TempDir;

    #[test]
    fn test_full_skill_to_tool_pipeline() {
        let registry = HotReloadSkillRegistry::new();

        let mut skill = Skill::new("pipeline-skill", "Full pipeline test", "Content");
        skill.skill_type = SkillType::Tool;
        skill.tools = vec!["bash".to_string(), "http".to_string()];

        registry.register(skill);

        let skills = registry.list();
        assert_eq!(skills.len(), 1);

        let tools = skills_to_tools(skills);
        assert_eq!(tools.len(), 1);

        let mut tool_registry = InMemoryToolRegistry::new();
        for tool in tools {
            tool_registry.register(tool).unwrap();
        }

        let defs = tool_registry.definitions();
        assert_eq!(defs.len(), 1);
        assert_eq!(defs[0].name, "pipeline-skill");
    }

    #[test]
    fn test_tool_registry_with_multiple_skills() {
        let registry = HotReloadSkillRegistry::new();

        let skill1 = Skill::new("tool-1", "Tool 1", "Content 1");
        let skill2 = Skill::new("tool-2", "Tool 2", "Content 2");

        registry.register(skill1);
        registry.register(skill2);

        let skills = registry.list();
        let tools = all_skills_to_tools(skills);

        let mut tool_registry = InMemoryToolRegistry::new();
        for tool in tools {
            tool_registry.register(tool).unwrap();
        }

        assert_eq!(tool_registry.list().len(), 2);
        assert!(tool_registry.get("tool-1").is_some());
        assert!(tool_registry.get("tool-2").is_some());
    }

    #[test]
    fn test_wasm_executor_creation() {
        let _executor = WasmExecutor::new();

        let mut skill = Skill::new("wasm-test", "WASM Test", "test content");
        skill.skill_type = SkillType::Wasm;
        skill.config.insert(
            "function".to_string(),
            serde_json::Value::String("run".to_string()),
        );

        let _executor2 = WasmExecutor::new();
        let _skill2 = Skill::new("wasm-test2", "WASM Test 2", "test content 2");
    }

    #[test]
    fn test_skill_type_conversions() {
        let text_skill = Skill::new("text", "Text skill", "Content");
        assert_eq!(text_skill.skill_type, SkillType::Text);

        let mut tool_skill = Skill::new("tool", "Tool skill", "Content");
        tool_skill.skill_type = SkillType::Tool;
        tool_skill.tools = vec!["bash".to_string()];
        assert_eq!(tool_skill.skill_type, SkillType::Tool);

        let mut wasm_skill = Skill::new("wasm", "WASM skill", "Content");
        wasm_skill.skill_type = SkillType::Wasm;
        assert_eq!(wasm_skill.skill_type, SkillType::Wasm);
    }

    #[test]
    fn test_skill_as_tool_with_config() {
        let mut skill = Skill::new("config-skill", "Config skill", "Content");
        skill.skill_type = SkillType::Tool;
        skill
            .config
            .insert("timeout".to_string(), serde_json::Value::Number(30.into()));
        skill
            .config
            .insert("enabled".to_string(), serde_json::Value::Bool(true));

        let tool = SkillAsTool::new(Arc::new(skill));
        let def = tool.definition();

        assert_eq!(def.name, "config-skill");
        assert!(def.params.len() >= 2);
    }

    #[test]
    fn test_end_to_end_skill_lifecycle() {
        let temp_dir = TempDir::new().unwrap();

        let mut reloader = SkillHotReloader::new().unwrap();
        reloader.start(temp_dir.path()).unwrap();

        let skill_dir = temp_dir.path().join("lifecycle-skill");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(
            skill_dir.join("SKILL.md"),
            r#"---
name: lifecycle-skill
description: Lifecycle test
---

Initial content
"#,
        )
        .unwrap();

        let registry = reloader.registry();

        // Trigger reload to pick up new file
        let _ = registry.reload_skill(&skill_dir);

        assert!(registry.get("lifecycle-skill").is_some());

        std::fs::write(
            skill_dir.join("SKILL.md"),
            r#"---
name: lifecycle-skill
description: Updated lifecycle test
---

Updated content
"#,
        )
        .unwrap();

        let _ = registry.reload_skill(&skill_dir);
        let updated = registry.get("lifecycle-skill").unwrap();
        assert_eq!(updated.description, "Updated lifecycle test");

        registry.unregister("lifecycle-skill");
        assert!(registry.get("lifecycle-skill").is_none());
    }

    #[test]
    fn test_concurrent_registry_operations() {
        use std::thread;

        let registry = Arc::new(HotReloadSkillRegistry::new());

        let registry1 = Arc::clone(&registry);
        let registry2 = Arc::clone(&registry);

        let handle1 = thread::spawn(move || {
            for i in 0..10 {
                let skill = Skill::new(
                    format!("skill-{}", i),
                    format!("Description {}", i),
                    format!("Content {}", i),
                );
                registry1.register(skill);
            }
        });

        let handle2 = thread::spawn(move || {
            for i in 10..20 {
                let skill = Skill::new(
                    format!("skill-{}", i),
                    format!("Description {}", i),
                    format!("Content {}", i),
                );
                registry2.register(skill);
            }
        });

        handle1.join().unwrap();
        handle2.join().unwrap();

        assert_eq!(registry.list().len(), 20);
    }

    #[test]
    fn test_tool_registry_sequential_operations() {
        let mut registry = InMemoryToolRegistry::new();

        for i in 0..50 {
            let skill = Skill::new(format!("tool-{}", i), "Description", "Content");
            let tool = SkillAsTool::new(Arc::new(skill));
            registry.register(Arc::new(tool)).unwrap();
        }

        assert_eq!(registry.list().len(), 50);
    }
}
