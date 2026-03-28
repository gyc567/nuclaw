//! Skill Installer Module
//!
//! Provides functionality for installing and managing skills from git repositories.

pub mod error;
pub mod git_installer;
pub mod parser;
pub mod validator;

pub use error::{InstallError, Result};
pub use git_installer::{GitInstaller, InstallResult};
pub use parser::{parse_install_request, InstallRequest, validate_skill_name};
