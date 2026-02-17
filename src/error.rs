//! Error handling for NuClaw

use thiserror::Error;

#[derive(Error, Debug)]
pub enum NuClawError {
    #[error("Database error: {message}")]
    Database { message: String },

    #[error("Container error: {message}")]
    Container { message: String },

    #[error("API error: {message}")]
    Api { message: String },

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = NuClawError::Database {
            message: "test error".to_string(),
        };
        let msg = format!("{}", err);
        assert!(msg.contains("Database error"));
        assert!(msg.contains("test error"));
    }

    #[test]
    fn test_error_from_sqlite() {
        let sqlite_err = rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error {
                code: rusqlite::ErrorCode::DatabaseBusy,
                extended_code: 5,
            },
            Some("busy".to_string()),
        );
        let err: NuClawError = sqlite_err.into();
        match err {
            NuClawError::Database { message } => {
                assert!(message.contains("busy"));
            }
            _ => panic!("Expected Database error"),
        }
    }

    #[test]
    fn test_error_from_io() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let err: NuClawError = io_err.into();
        match err {
            NuClawError::FileSystem { message } => {
                assert!(message.contains("file not found"));
            }
            _ => panic!("Expected FileSystem error"),
        }
    }

    #[test]
    fn test_all_error_variants() {
        let _ = NuClawError::Database {
            message: "test".to_string(),
        };
        let _ = NuClawError::Container {
            message: "test".to_string(),
        };
        let _ = NuClawError::Api {
            message: "test".to_string(),
        };
        let _ = NuClawError::WhatsApp {
            message: "test".to_string(),
        };
        let _ = NuClawError::Telegram {
            message: "test".to_string(),
        };
        let _ = NuClawError::Config {
            message: "test".to_string(),
        };
        let _ = NuClawError::FileSystem {
            message: "test".to_string(),
        };
        let _ = NuClawError::Validation {
            message: "test".to_string(),
        };
        let _ = NuClawError::Timeout {
            operation: "test".to_string(),
        };
        let _ = NuClawError::Auth {
            message: "test".to_string(),
        };
        let _ = NuClawError::Scheduler {
            message: "test".to_string(),
        };
    }
}
