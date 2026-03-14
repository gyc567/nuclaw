use std::fs;
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProgramError {
    #[error("Failed to read program: {0}")]
    ReadError(#[from] std::io::Error),
    #[error("Invalid program format")]
    InvalidFormat,
}

#[derive(Debug, Clone, Default)]
pub struct Program {
    pub name: String,
    pub description: String,
    pub content: String,
    pub compatibility: Option<String>,
}

impl Program {
    pub fn load(path: &Path) -> Result<Self, ProgramError> {
        let content = fs::read_to_string(path)?;
        Self::parse(&content)
    }

    pub fn parse(content: &str) -> Result<Self, ProgramError> {
        if content.trim().is_empty() {
            return Err(ProgramError::InvalidFormat);
        }

        if content.starts_with("---") {
            Self::parse_frontmatter(content)
        } else {
            Self::parse_simple(content)
        }
    }

    fn parse_frontmatter(content: &str) -> Result<Self, ProgramError> {
        let end_idx = content[3..]
            .find("---")
            .ok_or(ProgramError::InvalidFormat)?;
        let frontmatter = &content[3..3 + end_idx];
        let body = content[3 + end_idx + 3..].trim().to_string();

        let mut name = "autoresearch".to_string();
        let mut description = String::new();
        let mut compatibility = None;

        for line in frontmatter.lines() {
            let line = line.trim();
            if let Some(s) = line.strip_prefix("name:") {
                name = s.trim().to_string();
            } else if let Some(s) = line.strip_prefix("description:") {
                description = s.trim().to_string();
            } else if let Some(s) = line.strip_prefix("compatibility:") {
                compatibility = Some(s.trim().to_string());
            }
        }

        Ok(Self {
            name,
            description: if description.is_empty() {
                "Auto research program".to_string()
            } else {
                description
            },
            compatibility,
            content: body,
        })
    }

    fn parse_simple(content: &str) -> Result<Self, ProgramError> {
        let first_line = content.lines().next().unwrap_or("");
        let name = first_line.trim_start_matches('#').trim().to_string();

        Ok(Self {
            name: if name.is_empty() {
                "autoresearch".to_string()
            } else {
                name
            },
            description: "Auto research program".to_string(),
            compatibility: None,
            content: content.to_string(),
        })
    }

    pub fn default_program() -> Self {
        Self {
            name: "autoresearch".to_string(),
            description: "Default auto research program".to_string(),
            compatibility: Some("Single GPU, Python 3.10+".to_string()),
            content: Default::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple() {
        let content = "# Research Program\n\nDo research.";
        let program = Program::parse(content).unwrap();
        assert_eq!(program.name, "Research Program");
    }

    #[test]
    fn test_parse_frontmatter() {
        let content = r#"---
name: my-research
description: Test program
---
# Body"#;
        let program = Program::parse(content).unwrap();
        assert_eq!(program.name, "my-research");
        assert_eq!(program.description, "Test program");
    }

    #[test]
    fn test_default() {
        let program = Program::default_program();
        assert_eq!(program.name, "autoresearch");
    }
}
