//! Prompt Builder - Builds system prompts from context

use crate::context::loader::{AgentContext, AgentRules, Identity, Memory, User};

// ============================================================================
// PromptBuilder
// ============================================================================

/// Builds system prompts from loaded context
pub struct PromptBuilder;

impl PromptBuilder {
    /// Build a complete system prompt from context
    pub fn build(identity: &Identity, user: &User, rules: &AgentRules, memory: &Memory) -> String {
        let mut prompt = String::new();

        // Identity section
        prompt.push_str("=== IDENTITY ===\n");
        prompt.push_str(&format!(
            "You are {} ({}) - {}\n",
            identity.name, identity.role, identity.vibe
        ));
        if !identity.emoji.is_empty() {
            prompt.push_str(&format!("Style: {}\n", identity.emoji));
        }
        if !identity.persona.is_empty() {
            prompt.push_str(&format!("\n{}\n", identity.persona));
        }

        // User section
        prompt.push_str("\n=== USER ===\n");
        prompt.push_str(&format!("User: {}\n", user.name));
        prompt.push_str(&format!("Timezone: {}\n", user.timezone));
        prompt.push_str(&format!("Language: {}\n", user.language));

        if !user.preferences.is_empty() {
            prompt.push_str("\nUser Preferences:\n");
            for pref in &user.preferences {
                prompt.push_str(&format!("- {}\n", pref));
            }
        }

        // Rules section
        prompt.push_str("\n=== RULES ===\n");
        prompt.push_str(&format!("Version: {}\n", rules.version));
        prompt.push_str(&format!(
            "Startup: {}\n",
            rules.startup_sequence.join(" → ")
        ));
        prompt.push_str(&format!("{}\n", rules.memory_rules));

        if !rules.safety_boundaries.is_empty() {
            prompt.push_str("\nSafety:\n");
            for bound in &rules.safety_boundaries {
                prompt.push_str(&format!("- {}\n", bound));
            }
        }

        // Memory section
        prompt.push_str("\n=== MEMORY ===\n");
        prompt.push_str(&format!("Last updated: {}\n", memory.last_updated));

        if !memory.preferences.is_empty() {
            prompt.push_str("\nUser's Preferences (from past interactions):\n");
            for pref in &memory.preferences {
                prompt.push_str(&format!("- {}\n", pref));
            }
        }

        if !memory.lessons_learned.is_empty() {
            prompt.push_str("\nLessons Learned:\n");
            for lesson in &memory.lessons_learned {
                prompt.push_str(&format!("- {}\n", lesson));
            }
        }

        if !memory.technical_context.is_empty() {
            prompt.push_str(&format!(
                "\nTechnical Context:\n{}\n",
                memory.technical_context
            ));
        }

        prompt
    }

    /// Build from complete AgentContext
    pub fn build_from_context(ctx: &AgentContext) -> String {
        Self::build(&ctx.identity, &ctx.user, &ctx.rules, &ctx.memory)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_prompt_complete() {
        let identity = Identity {
            name: "Andy".to_string(),
            role: "Research".to_string(),
            vibe: "Professional".to_string(),
            emoji: "🔍".to_string(),
            traits: vec!["thorough".to_string()],
            persona: "Like Dwight from The Office".to_string(),
        };

        let user = User {
            name: "John".to_string(),
            timezone: "UTC".to_string(),
            language: "en".to_string(),
            preferences: vec!["bullet_points".to_string()],
            background: "Engineer".to_string(),
        };

        let rules = AgentRules {
            version: "1.0".to_string(),
            startup_sequence: vec!["load_identity".to_string()],
            memory_rules: "Remember important".to_string(),
            safety_boundaries: vec!["Private data".to_string()],
        };

        let memory = Memory {
            last_updated: "2026-03-19".to_string(),
            version: 1,
            preferences: vec!["short_responses".to_string()],
            lessons_learned: vec!["No steak restaurants".to_string()],
            technical_context: "Rust developer".to_string(),
        };

        let prompt = PromptBuilder::build(&identity, &user, &rules, &memory);

        assert!(prompt.contains("Andy"));
        assert!(prompt.contains("John"));
        assert!(prompt.contains("1.0"));
        assert!(prompt.contains("2026-03-19"));
        assert!(prompt.contains("=== IDENTITY ==="));
        assert!(prompt.contains("=== USER ==="));
        assert!(prompt.contains("=== RULES ==="));
        assert!(prompt.contains("=== MEMORY ==="));
    }

    #[test]
    fn test_build_prompt_empty_sections() {
        let identity = Identity::default_identity();
        let user = User::default_user();
        let rules = AgentRules::default_rules();
        let memory = Memory::default_memory();

        let prompt = PromptBuilder::build(&identity, &user, &rules, &memory);

        // Should still have structure
        assert!(prompt.contains("=== IDENTITY ==="));
        assert!(prompt.contains("=== USER ==="));
        assert!(prompt.contains("=== RULES ==="));
        assert!(prompt.contains("=== MEMORY ==="));
        assert!(prompt.contains("NuClaw"));
    }

    #[test]
    fn test_build_from_context() {
        let ctx = AgentContext {
            identity: Identity {
                name: "TestBot".to_string(),
                ..Default::default()
            },
            ..Default::default()
        };

        let prompt = PromptBuilder::build_from_context(&ctx);
        assert!(prompt.contains("TestBot"));
    }
}
