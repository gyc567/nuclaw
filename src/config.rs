//! Configuration for NuClaw

use std::env;
use std::path::PathBuf;

pub fn project_root() -> PathBuf {
    env::current_dir().expect("Failed to get current directory")
}

pub fn store_dir() -> PathBuf {
    project_root().join("store")
}

pub fn groups_dir() -> PathBuf {
    project_root().join("groups")
}

pub fn data_dir() -> PathBuf {
    project_root().join("data")
}

pub fn logs_dir() -> PathBuf {
    groups_dir().join("logs")
}

pub fn mount_allowlist_path() -> PathBuf {
    let home = home::home_dir().unwrap_or_else(|| PathBuf::from("/Users/user"));
    home.join(".config")
        .join("nuclaw")
        .join("mount-allowlist.json")
}

pub fn assistant_name() -> String {
    env::var("ASSISTANT_NAME").unwrap_or_else(|_| "Andy".to_string())
}

pub fn timezone() -> String {
    env::var("TZ").unwrap_or_else(|_| "UTC".to_string())
}

pub fn ensure_directories() -> std::io::Result<()> {
    let dirs = [
        store_dir(),
        groups_dir(),
        data_dir(),
        mount_allowlist_path().parent().unwrap().to_path_buf(),
    ];
    for dir in dirs {
        if !dir.exists() {
            std::fs::create_dir_all(&dir)?;
        }
    }
    Ok(())
}
