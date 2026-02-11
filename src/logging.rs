//! Logging module for NuClaw
//!
//! Provides unified logging initialization with support for both
//! env_logger and tracing integration.

use log::LevelFilter;
use std::sync::OnceLock;

/// Global logging initialization status
static LOG_INIT: OnceLock<()> = OnceLock::new();

/// Logging configuration
#[derive(Debug, Clone)]
pub struct LoggingConfig {
    /// Default log level
    pub level: Level,
    /// Whether to use JSON formatting
    pub json_format: bool,
    /// Whether to include timestamps
    pub include_timestamp: bool,
}

/// Log level enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    /// Trace level (most verbose)
    Trace,
    /// Debug level
    Debug,
    /// Info level (default)
    Info,
    /// Warning level
    Warn,
    /// Error level
    Error,
    /// Disable logging
    Off,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: Level::from_env().unwrap_or(Level::Info),
            json_format: std::env::var("NUCLAW_LOG_JSON")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(false),
            include_timestamp: true,
        }
    }
}

impl Level {
    /// Get log level from RUST_LOG environment variable
    pub fn from_env() -> Option<Self> {
        let rust_log = std::env::var("RUST_LOG").ok()?;
        Self::from_env_str(&rust_log)
    }

    /// Parse level from string (for env vars)
    pub fn from_env_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "trace" => Some(Level::Trace),
            "debug" => Some(Level::Debug),
            "info" => Some(Level::Info),
            "warn" | "warning" => Some(Level::Warn),
            "error" => Some(Level::Error),
            "off" => Some(Level::Off),
            _ => None,
        }
    }

    /// Convert to LevelFilter
    fn to_filter(self) -> LevelFilter {
        match self {
            Level::Trace => LevelFilter::Trace,
            Level::Debug => LevelFilter::Debug,
            Level::Info => LevelFilter::Info,
            Level::Warn => LevelFilter::Warn,
            Level::Error => LevelFilter::Error,
            Level::Off => LevelFilter::Off,
        }
    }
}

/// Initialize logging with default configuration
pub fn init() {
    init_with_config(LoggingConfig::default());
}

/// Initialize logging with custom configuration
pub fn init_with_config(config: LoggingConfig) {
    // Ensure logging is only initialized once
    let _ = LOG_INIT.get_or_init(|| {
        setup_logging(&config);
    });
}

/// Setup logging based on configuration
fn setup_logging(config: &LoggingConfig) {
    // Set RUST_LOG for env_logger
    std::env::set_var("RUST_LOG", format!("{}", config.level).to_lowercase());

    // Clone config for the closure
    let config = config.clone();

    // Initialize env_logger with custom format
    env_logger::Builder::from_default_env()
        .format_timestamp(None)
        .format(move |buf, record| {
            use std::io::Write;

            let timestamp = if config.include_timestamp {
                let now = chrono::Utc::now();
                Some(format!("[{}]", now.to_rfc3339()))
            } else {
                None
            };

            let level = match record.level() {
                log::Level::Error => "ERROR",
                log::Level::Warn => "WARN",
                log::Level::Info => "INFO",
                log::Level::Debug => "DEBUG",
                log::Level::Trace => "TRACE",
            };

            if config.json_format {
                // JSON format for structured logging
                let output = serde_json::json!({
                    "timestamp": chrono::Utc::now().to_rfc3339(),
                    "level": level,
                    "message": record.args(),
                    "module": record.module_path().unwrap_or("unknown"),
                    "file": record.file().unwrap_or("unknown"),
                    "line": record.line(),
                });
                writeln!(buf, "{}", output)
            } else {
                // Human-readable format
                let mut output = String::new();
                if let Some(ts) = timestamp {
                    output.push_str(&ts);
                    output.push(' ');
                }
                output.push_str(level);
                output.push_str(": ");
                output.push_str(&format!("{}", record.args()));
                writeln!(buf, "{}", output)
            }
        })
        .filter(None, config.level.to_filter())
        .init();
}

/// Check if logging has been initialized
pub fn is_initialized() -> bool {
    LOG_INIT.get().is_some()
}

/// Get current log level from environment
pub fn get_log_level() -> Level {
    Level::from_env().unwrap_or(Level::Info)
}

impl std::fmt::Display for Level {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Level::Trace => write!(f, "trace"),
            Level::Debug => write!(f, "debug"),
            Level::Info => write!(f, "info"),
            Level::Warn => write!(f, "warn"),
            Level::Error => write!(f, "error"),
            Level::Off => write!(f, "off"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_level_from_str() {
        assert_eq!(Level::from_env_str("trace"), Some(Level::Trace));
        assert_eq!(Level::from_env_str("debug"), Some(Level::Debug));
        assert_eq!(Level::from_env_str("info"), Some(Level::Info));
        assert_eq!(Level::from_env_str("warn"), Some(Level::Warn));
        assert_eq!(Level::from_env_str("warning"), Some(Level::Warn));
        assert_eq!(Level::from_env_str("error"), Some(Level::Error));
        assert_eq!(Level::from_env_str("off"), Some(Level::Off));
        assert_eq!(Level::from_env_str("invalid"), None);
    }

    #[test]
    fn test_level_display() {
        assert_eq!(format!("{}", Level::Trace), "trace");
        assert_eq!(format!("{}", Level::Debug), "debug");
        assert_eq!(format!("{}", Level::Info), "info");
        assert_eq!(format!("{}", Level::Warn), "warn");
        assert_eq!(format!("{}", Level::Error), "error");
        assert_eq!(format!("{}", Level::Off), "off");
    }

    #[test]
    fn test_logging_config_defaults() {
        let config = LoggingConfig::default();
        // Should default to Info or whatever RUST_LOG is set to
        assert!(!config.json_format);
        assert!(config.include_timestamp);
    }

    #[test]
    fn test_logging_config_from_env() {
        // Save original values
        let original_json = std::env::var("NUCLAW_LOG_JSON").ok();

        std::env::set_var("NUCLAW_LOG_JSON", "true");
        let config = LoggingConfig::default();
        assert!(config.json_format);

        // Restore
        match original_json {
            Some(v) => std::env::set_var("NUCLAW_LOG_JSON", v),
            None => std::env::remove_var("NUCLAW_LOG_JSON"),
        }
    }

    #[test]
    fn test_is_initialized() {
        // is_initialized() is safe to call even if not initialized
        let _ = is_initialized();
    }

    #[test]
    fn test_get_log_level() {
        // Save original
        let original = std::env::var("RUST_LOG").ok();

        std::env::remove_var("RUST_LOG");
        let level = get_log_level();
        assert_eq!(level, Level::Info);

        std::env::set_var("RUST_LOG", "debug");
        let level = get_log_level();
        assert_eq!(level, Level::Debug);

        // Restore
        match original {
            Some(v) => std::env::set_var("RUST_LOG", v),
            None => std::env::remove_var("RUST_LOG"),
        }
    }

    #[test]
    fn test_init_with_config() {
        let config = LoggingConfig {
            level: Level::Debug,
            json_format: false,
            include_timestamp: false,
        };

        // Should not panic
        init_with_config(config);
        assert!(is_initialized());
    }
}
