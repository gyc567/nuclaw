//! Workflow configuration system for NuClaw
//!
//! This module provides:
//! - Loading WORKFLOW.md with YAML front matter
//! - Environment variable resolution
//! - Config validation
//! - Hook execution for workspace lifecycle

pub mod config;
pub mod hooks;
pub mod loader;

// Re-exports
pub use config::{AgentSettings, ChannelConfig, ChannelSettings, ContainerSettings, HookSettings, WorkflowConfig};
pub use hooks::{HookRunner, HookType};
pub use loader::{WorkflowLoader, WorkflowLoaderError};
