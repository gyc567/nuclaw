//! Git-based skill installer

use std::path::{Path, PathBuf};
use std::time::Duration;

use tokio::process::Command;

use crate::config::skills_dir;

use super::error::{InstallError, Result};
use super::parser::{InstallRequest, validate_skill_name};

/// Result of successful skill installation
#[derive(Debug, Clone)]
pub struct InstallResult {
    /// Skill name
    pub name: String,
    /// Installation path
    pub path: PathBuf,
    /// Repository info
    pub repo_url: String,
}

/// Git installer configuration
#[derive(Debug, Clone)]
pub struct GitInstallerConfig {
    /// Clone depth (1 for shallow clone)
    pub depth: u32,
    /// Operation timeout
    pub timeout: Duration,
    /// Maximum skill size in MB
    pub max_size_mb: u32,
}

impl Default for GitInstallerConfig {
    fn default() -> Self {
        Self {
            depth: 1,
            timeout: Duration::from_secs(120), // 2 minutes
            max_size_mb: 50,
        }
    }
}

/// Git-based skill installer
pub struct GitInstaller {
    config: GitInstallerConfig,
    temp_dir: PathBuf,
}

impl GitInstaller {
    /// Create a new GitInstaller
    pub fn new(config: GitInstallerConfig) -> Self {
        let temp_dir = std::env::temp_dir().join("nuclaw-skill-install");
        Self { config, temp_dir }
    }

    /// Create with default config
    pub fn with_defaults() -> Self {
        Self::new(GitInstallerConfig::default())
    }

    /// Install a skill from GitHub URL
    pub async fn install(&self, request: &InstallRequest) -> Result<InstallResult> {
        // 1. Validate skill name
        let skill_name = request.skill_name();
        validate_skill_name(skill_name)?;

        // 2. Check if skill already exists
        let target_dir = skills_dir().join(skill_name);
        if target_dir.exists() && !request.force {
            return Err(InstallError::AlreadyExists(skill_name.to_string()));
        }

        // 3. Ensure temp directory exists
        self.ensure_temp_dir()?;

        // 4. Create unique temp directory for this install
        let temp_skill_dir = self.temp_dir.join(format!(
            "{}_{}",
            skill_name,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis()
        ));

        // 5. Clone the repository (shallow)
        tracing::info!("Cloning {} to {:?}", request.source_url, temp_skill_dir);
        self.clone_shallow(&request.source_url, &temp_skill_dir).await?;

        // 6. Validate cloned content
        self.validate_clone(&temp_skill_dir)?;

        // 7. Move to final location (atomic operation)
        let final_dir = skills_dir().join(skill_name);
        self.move_to_final(&temp_skill_dir, &final_dir, request.force)?;

        // 8. Cleanup temp directory
        let _ = self.cleanup_temp_dir();

        tracing::info!("Successfully installed skill '{}' at {:?}", skill_name, final_dir);

        Ok(InstallResult {
            name: skill_name.to_string(),
            path: final_dir,
            repo_url: request.source_url.clone(),
        })
    }

    /// Ensure temp directory exists
    fn ensure_temp_dir(&self) -> Result<()> {
        std::fs::create_dir_all(&self.temp_dir)?;
        Ok(())
    }

    /// Perform shallow clone with timeout
    async fn clone_shallow(&self, url: &str, target: &Path) -> Result<()> {
        let mut cmd = Command::new("git");
        cmd.arg("clone")
           .arg("--depth")
           .arg(self.config.depth.to_string())
           .arg("--single-branch")
           .arg("--branch")
           .arg("main")
           .arg(url)
           .arg(target.as_os_str())
           // Suppress progress output
           .arg("-q")
           .arg("--no-show-current-forced")
           // Disable interactive prompts
           .env("GIT_TERMINAL_PROMPT", "0")
           .env("GIT_ASKPASS", "echo")
           .env("GIT_EDITOR", "echo");

        // Try main branch first, then master
        let output = self.run_with_timeout(cmd).await;

        match output {
            Ok(o) if o.status.success() => Ok(()),
            Ok(o) => {
                // Try master branch
                let mut cmd = Command::new("git");
                cmd.arg("clone")
                   .arg("--depth")
                   .arg(self.config.depth.to_string())
                   .arg("--single-branch")
                   .arg("--branch")
                   .arg("master")
                   .arg(url)
                   .arg(target.as_os_str())
                   .arg("-q")
                   .env("GIT_TERMINAL_PROMPT", "0")
                   .env("GIT_ASKPASS", "echo")
                   .env("GIT_EDITOR", "echo");

                let output = self.run_with_timeout(cmd).await?;

                if output.status.success() {
                    Ok(())
                } else {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    Err(InstallError::GitError(stderr.to_string()))
                }
            }
            Err(e) => Err(e),
        }
    }

    /// Run command with timeout
    async fn run_with_timeout(&self, mut cmd: Command) -> Result<std::process::Output> {
        use tokio::time::timeout;

        match timeout(self.config.timeout, cmd.output()).await {
            Ok(Ok(output)) => Ok(output),
            Ok(Err(e)) => Err(InstallError::IoError(e)),
            Err(_) => Err(InstallError::Timeout(self.config.timeout.as_secs())),
        }
    }

    /// Validate the cloned repository
    fn validate_clone(&self, dir: &Path) -> Result<()> {
        // Check directory exists
        if !dir.is_dir() {
            return Err(InstallError::InvalidSkill("Clone directory not found".to_string()));
        }

        // Check SKILL.md exists
        let skill_md = dir.join("SKILL.md");
        if !skill_md.exists() {
            return Err(InstallError::InvalidSkill(
                "SKILL.md not found in repository".to_string()
            ));
        }

        // Check size
        let size_mb = self.get_dir_size_mb(dir)?;
        if size_mb > self.config.max_size_mb as u64 {
            return Err(InstallError::InvalidSkill(
                format!("Skill too large: {}MB (max {}MB)", size_mb, self.config.max_size_mb)
            ));
        }

        // Validate skill name matches directory name
        if let Some(name) = dir.file_name().and_then(|n| n.to_str()) {
            validate_skill_name(name)?;
        }

        Ok(())
    }

    /// Get directory size in MB
    fn get_dir_size_mb(&self, dir: &Path) -> Result<u64> {
        let mut size = 0u64;
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let metadata = entry.metadata()?;
            if metadata.is_file() {
                size += metadata.len();
            }
        }
        Ok(size / (1024 * 1024))
    }

    /// Move cloned directory to final location (atomic)
    fn move_to_final(&self, temp_dir: &Path, final_dir: &Path, force: bool) -> Result<()> {
        // If force and final dir exists, remove it first
        if final_dir.exists() {
            if force {
                std::fs::remove_dir_all(final_dir)?;
            } else {
                return Err(InstallError::AlreadyExists(
                    final_dir.file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("unknown")
                        .to_string()
                ));
            }
        }

        // Rename is atomic on same filesystem
        std::fs::rename(temp_dir, final_dir)?;

        Ok(())
    }

    /// Clean up temporary directory
    fn cleanup_temp_dir(&self) -> Result<()> {
        if self.temp_dir.exists() {
            std::fs::remove_dir_all(&self.temp_dir)?;
        }
        Ok(())
    }
}

/// Uninstall a skill
pub fn uninstall_skill(name: &str) -> Result<()> {
    let skill_dir = skills_dir().join(name);
    
    if !skill_dir.exists() {
        return Err(InstallError::NotFound(format!("Skill '{}' not found", name)));
    }

    std::fs::remove_dir_all(&skill_dir)?;
    
    tracing::info!("Uninstalled skill '{}'", name);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_git_installer_config_default() {
        let config = GitInstallerConfig::default();
        assert_eq!(config.depth, 1);
        assert_eq!(config.timeout, Duration::from_secs(120));
        assert_eq!(config.max_size_mb, 50);
    }
}
