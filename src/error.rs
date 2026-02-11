//! Error handling for NuClaw

use thiserror::Error;

#[derive(Error, Debug)]
pub enum NuClawError {
    #[error("Database error: {message}")]
    Database { message: String },

    #[error("Container error: {message}")]
    Container { message: String },

    #[error("WhatsApp error: {message}")]
    WhatsApp { message: String },

    #[error("Telegram error: {message}")]
    Telegram { message: String },

    #[error("Configuration error: {message}")]
    Config { message: String },

    #[error("File system error: {message}")]
    FileSystem { message: String },

    #[error("Validation error: {message}")]
    Validation { message: String },

    #[error("Timeout error: {operation}")]
    Timeout { operation: String },

    #[error("Authentication error: {message}")]
    Auth { message: String },

    #[error("Scheduler error: {message}")]
    Scheduler { message: String },
}

pub type Result<T> = std::result::Result<T, NuClawError>;

impl From<rusqlite::Error> for NuClawError {
    fn from(e: rusqlite::Error) -> Self {
        NuClawError::Database {
            message: e.to_string(),
        }
    }
}

impl From<std::io::Error> for NuClawError {
    fn from(e: std::io::Error) -> Self {
        NuClawError::FileSystem {
            message: e.to_string(),
        }
    }
}
