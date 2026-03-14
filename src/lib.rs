//!
//! A Rust implementation of NanoClaw project structure.
//! Features:
//! - WhatsApp integration via MCP
//! - Telegram integration via Bot API
//! - Containerized agent execution
//! - Scheduled task management
//! - SQLite persistence
//! - Built-in Skills system
//! - Provider/Channel registry

pub mod agent_runner;
pub mod autoresearch;
pub mod channels;
pub mod config;
pub mod container_runner;
pub mod db;
pub mod error;
pub mod logging;
pub mod maintenance;
pub mod memory;
pub mod observer;
pub mod onboard;
pub mod providers;
pub mod security;
pub mod skills;
pub mod task_scheduler;
pub mod telegram;
pub mod types;
pub mod utils;
pub mod whatsapp;

// Re-exports for convenience
pub use agent_runner::{agent_runner_mode, create_runner, AgentRunner, AgentRunnerMode};
pub use channels::{Channel, ChannelRegistry};
pub use config::ensure_directories;
pub use container_runner::{
    container_timeout, create_group_ipc_directory, ensure_container_system_running,
    max_output_size, run_container,
};
pub use error::{NuClawError, Result};
pub use onboard::{load_config, print_config_status, run_onboard, save_config};
pub use providers::{ProviderConfig, ProviderRegistry, ProviderSpec, PROVIDERS};
pub use skills::{Skill, SkillRegistry};
pub use task_scheduler::TaskScheduler;
pub use telegram::{
    chunk_text_advanced, chunk_text_pure, extract_chat_id_pure, is_allowed_group_pure,
    is_duplicate_message_pure, load_registered_groups, load_router_state, truncate, ChunkMode,
    DMPolicy, GroupPolicy, ReplyMode, StreamMode, TelegramChat, TelegramClient, TelegramMessage,
    TelegramUpdate, TelegramUser, DEFAULT_TEXT_CHUNK_LIMIT,
};
pub use autoresearch::{
    AutoResearchRunner, EvalError, Evaluator, ExperimentConfig, ExperimentHistory, ExperimentResult,
    Metric, Program, ProgramError, RunnerError,
};
