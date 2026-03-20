//!
//! A Rust implementation of NuClaw project structure.
//! Features:
//! - WhatsApp integration via MCP
//! - Telegram integration via Bot API
//! - Containerized agent execution
//! - Scheduled task management
//! - SQLite persistence
//! - Built-in Skills system
//! - Provider/Channel registry

pub mod agent_runner;
pub mod auth;
pub mod autoresearch;
pub mod channels;
pub mod config;
pub mod container_runner;
pub mod db;
pub mod discord;
pub mod error;
pub mod logging;
pub mod maintenance;
pub mod memory;
pub mod observer;
pub mod onboard;
pub mod orchestrator;
pub mod providers;
pub mod router;
pub mod runtime;
pub mod security;
pub mod skills;
pub mod skill_to_rig;
pub mod tool_registry;
pub mod skill_watcher;
pub mod hot_reload_registry;
pub mod skill_hot_reloader;
pub mod wasm_executor;
pub mod task_scheduler;
pub mod telegram;
pub mod types;
pub mod utils;
pub mod whatsapp;
pub mod workflow;
pub mod workspace;
pub mod skill_creator;
pub mod workspace_manager;
pub mod context;

// Re-exports for convenience
pub use agent_runner::{agent_runner_mode, create_runner, AgentRunner, AgentRunnerMode};
pub use autoresearch::{
    AutoResearchRunner, EvalError, Evaluator, ExperimentConfig, ExperimentHistory,
    ExperimentResult, Metric, Program, ProgramError, RunnerError,
};
pub use channels::{Channel, ChannelRegistry};
pub use config::ensure_directories;
pub use container_runner::{
    container_timeout, create_group_ipc_directory, ensure_container_system_running,
    max_output_size, run_container,
};
pub use error::{NuClawError, Result};
pub use onboard::{load_config, print_config_status, run_onboard, save_config};
pub use orchestrator::{
    Executor, ExecutorConfig, ExecutorEvent, ExecutorStats, Metrics, MetricsSnapshot, Priority,
    Task, TaskId, TaskQueue, TaskResult, TaskSource, TaskStatus,
};
pub use providers::{ProviderConfig, ProviderRegistry, ProviderSpec, PROVIDERS};
pub use skills::{Skill, SkillRegistry, SkillValidationError};
pub use skill_watcher::{SkillChangeEvent, SkillEvent, SkillWatcher, SkillWatcherError};
pub use hot_reload_registry::HotReloadSkillRegistry;
pub use skill_hot_reloader::{create_hot_reloader, init_hot_reload, SkillHotReloader};
pub use tool_registry::{InMemoryToolRegistry, Tool, ToolContext, ToolDefinition, ToolError, ToolParam, ToolRegistry, ToolResult};
pub use skill_to_rig::{all_skills_to_tools, skills_to_tools, SkillAsTool, SkillExecutor};
pub use wasm_executor::WasmExecutor;
pub use task_scheduler::TaskScheduler;
pub use telegram::{
    chunk_text_advanced, chunk_text_pure, extract_chat_id_pure, is_allowed_group_pure,
    is_duplicate_message_pure, load_registered_groups, load_router_state, truncate, ChunkMode,
    DMPolicy, GroupPolicy, ReplyMode, StreamMode, TelegramChat, TelegramClient, TelegramMessage,
    TelegramUpdate, TelegramUser, DEFAULT_TEXT_CHUNK_LIMIT,
};
pub use workflow::{
    HookRunner, HookType, WorkflowConfig, WorkflowLoader, WorkflowLoaderError, WorkflowWatcher,
};
pub use workspace_manager::{ResolvedWorkspace, WorkspaceManager, WorkspaceType};

pub use skill_creator::intent::{IntentDetector, SkillIntent, SkillIntentType};
pub use skill_creator::writer::SkillWriter;
pub use skill_creator::eval::{EvalCase, EvalResult, EvalRunner, EvalConfig};
