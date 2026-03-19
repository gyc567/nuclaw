//! Skill Creator Module
//!
//! Provides skill creation capabilities:
//! - Intent capture from user messages
//! - SKILL.md generation
//! - Skill validation
//! - Basic eval runner

pub mod intent;
pub mod writer;
pub mod eval;

pub use intent::{IntentDetector, SkillIntent};
pub use writer::SkillWriter;
pub use eval::{EvalCase, EvalResult, EvalRunner};
