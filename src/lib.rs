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
pub mod task_scheduler;
pub mod telegram;
pub mod types;
pub mod utils;
pub mod whatsapp;

// Re-exports for convenience
pub use config::ensure_directories;
pub use container_runner::{run_container, ensure_container_system_running, container_timeout, max_output_size, create_group_ipc_directory};
pub use error::{NuClawError, Result};
pub use task_scheduler::TaskScheduler;
