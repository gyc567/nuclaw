//! Context Module - File-driven context loading for Agent memory system
//!
//! This module provides:
//! - Security layer (path validation, content sanitization)
//! - Performance layer (caching, async loading)
//! - Core context loading (Identity, User, Rules, Memory)
//! - Simple file-based memory management
//! - Agent coordination

pub mod bridge; // Keep for backward compatibility
pub mod builder;
pub mod cache;
pub mod coordinator;
pub mod loader;
pub mod memory; // NEW: Simplified memory management
pub mod security;
pub mod tracker;

pub use bridge::MemoryBridge;
pub use builder::PromptBuilder;
pub use cache::ContextCache;
pub use coordinator::AgentCoordinator;
pub use loader::{AgentContext, AgentRules, ContextLoader, Identity, Memory, User};
pub use memory::{FileMemory, Memory as ContextMemory, MemoryError}; // Use new memory module
pub use security::{ContentSanitizer, PathValidator, PermissionChecker, SecurityError};
pub use tracker::AccessTracker;
