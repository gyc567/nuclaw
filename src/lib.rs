//!
//! A Rust implementation of NanoClaw project structure.
//! Features:
//! - WhatsApp integration via MCP
//! - Telegram integration via Bot API
//! - Containerized agent execution
//! - Scheduled task management
//! - SQLite persistence

pub mod config;
pub mod container_runner;
pub mod db;
pub mod error;
pub mod logging;
pub mod task_scheduler;
pub mod telegram;
pub mod types;
pub mod utils;
pub mod whatsapp;

// Re-exports for convenience
pub use config::ensure_directories;
pub use container_runner::{
    container_timeout, create_group_ipc_directory, ensure_container_system_running,
    max_output_size, run_container,
};
pub use error::{NuClawError, Result};
pub use task_scheduler::TaskScheduler;
