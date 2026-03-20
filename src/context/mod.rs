//! Context Module - File-driven context loading for Agent memory system
//! 
//! This module provides:
//! - Security layer (path validation, content sanitization)
//! - Performance layer (caching, async loading)
//! - Core context loading (Identity, User, Rules, Memory)
//! - Memory bridge (TieredMemory ↔ File sync)
//! - Agent coordination

pub mod security;
pub mod cache;
pub mod loader;
pub mod builder;
pub mod bridge;
pub mod tracker;
pub mod coordinator;

pub use security::{PathValidator, ContentSanitizer, PermissionChecker, SecurityError};
pub use cache::ContextCache;
pub use loader::{ContextLoader, AgentContext, Identity, User, AgentRules, Memory};
pub use builder::PromptBuilder;
pub use bridge::MemoryBridge;
pub use tracker::AccessTracker;
pub use coordinator::AgentCoordinator;
