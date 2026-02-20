use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

const DEFAULT_BLOCKED_PATHS: &[&str] = &["/etc", "/root", "/proc", "/sys", "/boot", "/dev", "/var"];

pub struct WorkspaceIsolation {
    allowed_roots: RwLock<Vec<PathBuf>>,
    blocked_paths: HashSet<PathBuf>,
    workspace_only: bool,
}

impl WorkspaceIsolation {
    pub fn new(workspace_only: bool) -> Self {
        Self {
            allowed_roots: RwLock::new(Vec::new()),
            blocked_paths: DEFAULT_BLOCKED_PATHS.iter().map(PathBuf::from).collect(),
            workspace_only,
        }
    }

    pub fn add_allowed_root(&self, path: PathBuf) {
        if let Ok(mut roots) = self.allowed_roots.write() {
            roots.push(path);
        }
    }

    pub fn is_path_allowed(&self, path: &Path) -> bool {
        let path_str = path.to_string_lossy();

        for blocked in &self.blocked_paths {
            let blocked_str = blocked.to_string_lossy();
            if path_str.starts_with(blocked_str.as_ref()) {
                return false;
            }
        }

        if self.workspace_only {
            if let Ok(roots) = self.allowed_roots.read() {
                return roots.iter().any(|root| {
                    let root_str = root.to_string_lossy();
                    path_str.starts_with(root_str.as_ref())
                });
            }
            return false;
        }

        true
    }

    pub fn sanitize_path(&self, input: &str) -> Option<PathBuf> {
        if input.contains('\0') {
            return None;
        }

        let path = PathBuf::from(input);

        if path
            .components()
            .any(|c| c == std::path::Component::ParentDir)
        {
            return None;
        }

        Some(path)
    }

    pub fn detect_symlink_escape(&self, path: &Path, base: &Path) -> bool {
        let resolved = match path.canonicalize() {
            Ok(p) => p,
            Err(_) => return false,
        };

        let base_resolved = match base.canonicalize() {
            Ok(p) => p,
            Err(_) => return true,
        };

        !resolved.starts_with(&base_resolved)
    }
}

pub struct CommandAllowlist {
    allowed_commands: RwLock<HashSet<String>>,
}

impl CommandAllowlist {
    pub fn new() -> Self {
        Self {
            allowed_commands: RwLock::new(HashSet::new()),
        }
    }

    pub fn add_command(&self, cmd: &str) {
        if let Ok(mut commands) = self.allowed_commands.write() {
            commands.insert(cmd.to_string());
        }
    }

    pub fn is_allowed(&self, command: &str) -> bool {
        let cmd_name = command.split_whitespace().next().unwrap_or("");

        if let Ok(commands) = self.allowed_commands.read() {
            return commands.contains(cmd_name);
        }

        false
    }

    pub fn validate(&self, command: &str) -> Result<(), String> {
        let cmd_name = command.split_whitespace().next().ok_or("Empty command")?;

        if let Ok(commands) = self.allowed_commands.read() {
            if commands.is_empty() {
                return Ok(());
            }

            if !commands.contains(cmd_name) {
                return Err(format!(
                    "Command '{}' not in allowlist: {:?}",
                    cmd_name,
                    commands.iter().collect::<Vec<_>>()
                ));
            }
        }

        let dangerous = ["rm -rf", "mkfs", "dd if=", ":(){:|:&};:"];
        let lower = command.to_lowercase();
        for pattern in dangerous {
            if lower.contains(pattern) {
                return Err(format!("Dangerous pattern detected: {}", pattern));
            }
        }

        Ok(())
    }
}

impl Default for CommandAllowlist {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_workspace_isolation_new() {
        let isolation = WorkspaceIsolation::new(true);
        assert!(isolation.workspace_only);
    }

    #[test]
    fn test_add_allowed_root() {
        let isolation = WorkspaceIsolation::new(true);
        isolation.add_allowed_root(PathBuf::from("/tmp/test"));

        let roots = isolation.allowed_roots.read().unwrap();
        assert!(roots.iter().any(|r| r.to_string_lossy().contains("test")));
    }

    #[test]
    fn test_is_path_allowed_with_workspace_only() {
        let isolation = WorkspaceIsolation::new(true);
        isolation.add_allowed_root(PathBuf::from("/tmp"));

        assert!(isolation.is_path_allowed(Path::new("/tmp/test.txt")));
        assert!(!isolation.is_path_allowed(Path::new("/etc/passwd")));
    }

    #[test]
    fn test_is_path_allowed_without_workspace_only() {
        let isolation = WorkspaceIsolation::new(false);

        assert!(isolation.is_path_allowed(Path::new("/tmp/test.txt")));
    }

    #[test]
    fn test_sanitize_path_valid() {
        let isolation = WorkspaceIsolation::new(false);
        let result = isolation.sanitize_path("/tmp/test.txt");
        assert!(result.is_some());
    }

    #[test]
    fn test_sanitize_path_null_byte() {
        let isolation = WorkspaceIsolation::new(false);
        let result = isolation.sanitize_path("/tmp/test.txt\0");
        assert!(result.is_none());
    }

    #[test]
    fn test_sanitize_path_parent_dir() {
        let isolation = WorkspaceIsolation::new(false);
        let result = isolation.sanitize_path("/tmp/../etc/passwd");
        assert!(result.is_none());
    }

    #[test]
    fn test_command_allowlist_new() {
        let allowlist = CommandAllowlist::new();
        assert!(!allowlist.is_allowed("ls"));
    }

    #[test]
    fn test_command_allowlist_add() {
        let allowlist = CommandAllowlist::new();
        allowlist.add_command("ls");
        allowlist.add_command("cat");

        assert!(allowlist.is_allowed("ls"));
        assert!(allowlist.is_allowed("cat"));
        assert!(!allowlist.is_allowed("rm"));
    }

    #[test]
    fn test_command_allowlist_validate() {
        let allowlist = CommandAllowlist::new();
        allowlist.add_command("ls");
        allowlist.add_command("git");

        assert!(allowlist.validate("ls /tmp").is_ok());
        assert!(allowlist.validate("git status").is_ok());
        assert!(allowlist.validate("rm -rf /").is_err());
    }

    #[test]
    fn test_command_allowlist_dangerous_patterns() {
        let allowlist = CommandAllowlist::new();
        allowlist.add_command("rm");

        assert!(allowlist.validate("rm -rf /").is_err());
        assert!(allowlist.validate("rm file.txt").is_ok());
    }

    #[test]
    fn test_default_blocked_paths() {
        let isolation = WorkspaceIsolation::new(false);

        assert!(!isolation.is_path_allowed(Path::new("/etc/passwd")));
        assert!(!isolation.is_path_allowed(Path::new("/root/.ssh")));
        assert!(!isolation.is_path_allowed(Path::new("/proc/1")));
    }
}
