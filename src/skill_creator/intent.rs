//! Intent Capture Module
//!
//! Detects and parses user intent to create a skill from natural language

use serde::{Deserialize, Serialize};

/// Represents a user's intent to create a skill
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SkillIntent {
    /// Skill identifier (kebab-case)
    pub name: String,
    /// When to trigger, what it does
    pub description: String,
    /// Skill body/instructions
    pub body: String,
    /// Test cases (optional)
    pub test_cases: Vec<String>,
    /// Skill type: text, tool, or wasm
    #[serde(default)]
    pub skill_type: SkillIntentType,
    /// Required tools for tool-type skills
    #[serde(default)]
    pub tools: Vec<String>,
}

impl Default for SkillIntent {
    fn default() -> Self {
        Self {
            name: String::new(),
            description: String::new(),
            body: String::new(),
            test_cases: Vec::new(),
            skill_type: SkillIntentType::Text,
            tools: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SkillIntentType {
    #[default]
    Text,
    Tool,
    Wasm,
}

/// Intent detector that parses user messages to extract skill creation intent
pub struct IntentDetector {
    keywords: Vec<&'static str>,
}

impl IntentDetector {
    pub fn new() -> Self {
        Self {
            keywords: vec![
                "创建skill",
                "make a skill",
                "turn this into a skill",
                "/skill-create",
                "创建一个skill",
                "create",
                "new skill",
                "skill for",
            ],
        }
    }

    /// Check if the message contains skill creation intent
    pub fn has_intent(&self, message: &str) -> bool {
        let lower = message.to_lowercase();
        self.keywords.iter().any(|kw| lower.contains(kw))
    }

    /// Extract skill intent from message
    /// Returns None if no intent detected, or partial intent if some info missing
    pub fn extract(&self, message: &str) -> Option<SkillIntent> {
        if !self.has_intent(message) {
            return None;
        }

        // Try to extract name from message
        let name = self.extract_name(message);
        let description = self.extract_description(message);
        let body = self.extract_body(message);
        let skill_type = self.extract_type(message);
        let tools = self.extract_tools(message);

        Some(SkillIntent {
            name,
            description,
            body,
            test_cases: Vec::new(),
            skill_type,
            tools,
        })
    }

    fn extract_name(&self, message: &str) -> String {
        // Try pattern: "skill for X" or "create skill X" or "named X"
        let patterns = [
            r"(?i)skill for ([\w-]+)",
            r"(?i)create(?: a)? skill (?:named? )?(?:for )?([\w-]+)",
            r"(?i)named? ([\w-]+)",
            r"(?i)name[:\s]+([\w-]+)",
        ];

        for pattern in patterns {
            if let Ok(re) = regex::Regex::new(pattern) {
                if let Some(caps) = re.captures(message) {
                    if let Some(name) = caps.get(1) {
                        return self.normalize_name(name.as_str());
                    }
                }
            }
        }

        // Default name based on timestamp
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() % 10000)
            .unwrap_or(0);
        format!("skill-{}", timestamp)
    }

    fn extract_description(&self, message: &str) -> String {
        // Extract description after trigger keywords
        let lower = message.to_lowercase();
        
        for kw in &self.keywords {
            if let Some(pos) = lower.find(kw) {
                let after = &message[pos + kw.len()..];
                let trimmed = after.trim();
                if !trimmed.is_empty() {
                    // Take first sentence or first 200 chars
                    let desc = if let Some(period) = trimmed.find('.') {
                        &trimmed[..period]
                    } else if trimmed.len() > 200 {
                        &trimmed[..200]
                    } else {
                        trimmed
                    };
                    return desc.trim().to_string();
                }
            }
        }

        // Fallback: use the whole message
        if message.len() > 200 {
            message[..200].to_string()
        } else {
            message.to_string()
        }
    }

    fn extract_body(&self, message: &str) -> String {
        // If message contains detailed instructions, use as body
        if message.len() > 100 {
            message.to_string()
        } else {
            String::new()
        }
    }

    fn extract_type(&self, message: &str) -> SkillIntentType {
        let lower = message.to_lowercase();
        if lower.contains("tool") || lower.contains("executable") || lower.contains("bash") {
            SkillIntentType::Tool
        } else if lower.contains("wasm") {
            SkillIntentType::Wasm
        } else {
            SkillIntentType::Text
        }
    }

    fn extract_tools(&self, message: &str) -> Vec<String> {
        let lower = message.to_lowercase();
        let mut tools = Vec::new();
        
        // Common tool names
        let tool_names = ["bash", "read", "write", "glob", "grep", "webfetch", "task"];
        for tool in tool_names {
            if lower.contains(tool) {
                tools.push(tool.to_string());
            }
        }
        
        tools
    }

    fn normalize_name(&self, name: &str) -> String {
        // Convert to kebab-case
        // Handle consecutive uppercase specially: "JSONParser" -> "json-parser"
        let mut result = String::new();
        let chars: Vec<char> = name.chars().collect();
        
        for (i, c) in chars.iter().enumerate() {
            if c.is_alphanumeric() {
                if c.is_uppercase() {
                    // Check if previous was lowercase or next is lowercase
                    let prev_lower = i > 0 && chars[i-1].is_lowercase();
                    let next_lower = i + 1 < chars.len() && chars[i+1].is_lowercase();
                    
                    if prev_lower || (next_lower && i > 0) {
                        result.push('-');
                    }
                    result.push(c.to_ascii_lowercase());
                } else {
                    result.push(*c);
                }
            } else if *c == '_' || *c == '-' {
                result.push('-');
            }
        }
        
        // Clean up
        let cleaned = result.trim_matches('-').replace("--", "-").replace(" -", "-").replace("- ", "-");
        
        if cleaned.is_empty() {
            let timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs() % 10000)
                .unwrap_or(0);
            format!("skill-{}", timestamp)
        } else {
            cleaned
        }
    }
}

impl Default for IntentDetector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detector_has_intent_chinese() {
        let detector = IntentDetector::new();
        assert!(detector.has_intent("我想创建一个skill来处理JSON"));
        assert!(detector.has_intent("创建skill: 用于解析CSV"));
    }

    #[test]
    fn test_detector_has_intent_english() {
        let detector = IntentDetector::new();
        assert!(detector.has_intent("make a skill for parsing JSON"));
        assert!(detector.has_intent("/skill-create for data processing"));
    }

    #[test]
    fn test_detector_no_intent() {
        let detector = IntentDetector::new();
        assert!(!detector.has_intent("Hello, how are you?"));
        assert!(!detector.has_intent("What's the weather today?"));
    }

    #[test]
    fn test_extract_name() {
        let detector = IntentDetector::new();
        
        let intent = detector.extract("create skill for json-parser").unwrap();
        // Should extract name or generate default
        assert!(!intent.name.is_empty());
    }

    #[test]
    fn test_extract_description() {
        let detector = IntentDetector::new();
        
        let intent = detector.extract("create skill for parsing JSON files").unwrap();
        assert!(!intent.description.is_empty());
    }

    #[test]
    fn test_extract_type_tool() {
        let detector = IntentDetector::new();
        
        let intent = detector.extract("create a tool skill that runs bash commands").unwrap();
        assert_eq!(intent.skill_type, SkillIntentType::Tool);
    }

    #[test]
    fn test_extract_type_text() {
        let detector = IntentDetector::new();
        
        let intent = detector.extract("create a skill for writing emails").unwrap();
        assert_eq!(intent.skill_type, SkillIntentType::Text);
    }

    #[test]
    fn test_extract_tools() {
        let detector = IntentDetector::new();
        
        let intent = detector.extract("create skill that uses bash and grep").unwrap();
        assert!(intent.tools.contains(&"bash".to_string()));
        assert!(intent.tools.contains(&"grep".to_string()));
    }

    #[test]
    fn test_normalize_name_simple() {
        let detector = IntentDetector::new();
        
        assert_eq!(detector.normalize_name("testSkill"), "test-skill");
        assert_eq!(detector.normalize_name("JSONParser"), "json-parser");
        assert_eq!(detector.normalize_name("test_skill"), "test-skill");
        assert_eq!(detector.normalize_name("my-api-client"), "my-api-client");
    }

    #[test]
    fn test_extract_no_intent() {
        let detector = IntentDetector::new();
        
        let result = detector.extract("Hello world");
        assert!(result.is_none());
    }

    #[test]
    fn test_skill_intent_default() {
        let intent = SkillIntent::default();
        assert_eq!(intent.name, "");
        assert_eq!(intent.description, "");
        assert_eq!(intent.skill_type, SkillIntentType::Text);
    }

    #[test]
    fn test_skill_intent_serialization() {
        let intent = SkillIntent {
            name: "test-skill".to_string(),
            description: "A test skill".to_string(),
            body: "Skill body content".to_string(),
            test_cases: vec!["test1".to_string()],
            skill_type: SkillIntentType::Tool,
            tools: vec!["bash".to_string()],
        };

        let json = serde_json::to_string(&intent).unwrap();
        let parsed: SkillIntent = serde_json::from_str(&json).unwrap();
        
        assert_eq!(parsed.name, "test-skill");
        assert_eq!(parsed.skill_type, SkillIntentType::Tool);
    }

    #[test]
    fn test_detector_new_has_keywords() {
        let detector = IntentDetector::new();
        assert!(!detector.keywords.is_empty());
    }
}
