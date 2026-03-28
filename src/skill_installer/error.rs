//! Error types for skill installer

use thiserror::Error;

/// Result type for skill installer
pub type Result<T> = std::result::Result<T, InstallError>;

/// Errors that can occur during skill installation
#[derive(Error, Debug)]
pub enum InstallError {
    /// Invalid skill name
    #[error("Invalid skill name: {0}")]
    InvalidName(String),

    /// Skill already exists
    #[error("Skill already exists: {0}. Use --force to overwrite.")]
    AlreadyExists(String),

    /// Invalid skill structure
    #[error("Invalid skill: {0}")]
    InvalidSkill(String),

    /// Git operation failed
    #[error("Git operation failed: {0}")]
    GitError(String),

    /// Network operation timed out
    #[error("Operation timed out after {0} seconds")]
    Timeout(u64),

    /// IO error
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    /// Validation failed
    #[error("Validation failed: {0}")]
    ValidationError(String),

    /// Network error
    #[error("Network error: {0}")]
    NetworkError(String),

    /// Skill tool not allowed (security)
    #[error("Tool '{0}' is not allowed for external skills")]
    ToolNotAllowed(String),

    /// Skill source not allowed
    #[error("Skill source not allowed: {0}")]
    SourceNotAllowed(String),

    /// Skill not found
    #[error("Skill not found: {0}")]
    NotFound(String),
}

impl From<reqwest::Error> for InstallError {
    fn from(e: reqwest::Error) -> Self {
        InstallError::NetworkError(e.to_string())
    }
}
