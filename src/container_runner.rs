//! Container Runner - Spawns AI agent containers with isolation
//!
//! Supports:
//! - macOS: Apple Container via `container` CLI
//! - Linux: Docker via `docker` CLI
//!
//! Features:
//! - Filesystem isolation per group
//! - IPC namespace isolation
//! - Configurable timeout
//! - Output parsing with sentinel markers

use crate::config::{assistant_name, data_dir, groups_dir, logs_dir};
use crate::error::{NuClawError, Result};
use crate::types::{ContainerInput, ContainerOutput};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Command as AsyncCommand, ChildStdout};
use tokio::time::{timeout, Duration, Instant};

/// Default container timeout: 5 minutes
const DEFAULT_TIMEOUT_MS: u64 = 300_000;
/// Default max output size: 10MB
const DEFAULT_MAX_OUTPUT: usize = 10 * 1024 * 1024;
/// Sentinel markers for output parsing
const OUTPUT_START_MARKER: &str = "--NANOCLAW_OUTPUT_START--";
const OUTPUT_END_MARKER: &str = "--NANOCLAW_OUTPUT_END--";

/// Get container timeout from environment or default
pub fn container_timeout() -> Duration {
    let timeout_ms = std::env::var("CONTAINER_TIMEOUT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_TIMEOUT_MS);
    Duration::from_millis(timeout_ms)
}

/// Get max output size from environment or default
pub fn max_output_size() -> usize {
    std::env::var("CONTAINER_MAX_OUTPUT_SIZE")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_MAX_OUTPUT)
}

/// Get the container command based on platform
fn get_container_command() -> &'static str {
    if cfg!(target_os = "macos") {
        "container"
    } else {
        "docker"
    }
}

/// Create IPC directory for a group
pub fn create_group_ipc_directory(group_folder: &str) -> Result<PathBuf> {
    let ipc_dir = data_dir().join("ipc").join(group_folder);
    fs::create_dir_all(&ipc_dir).map_err(|e| NuClawError::FileSystem {
        message: format!("Failed to create IPC directory: {}", e),
    })?;
    Ok(ipc_dir)
}

/// Write IPC files for container context
fn write_ipc_files(group_folder: &str, input: &ContainerInput) -> Result<()> {
    let ipc_dir = create_group_ipc_directory(group_folder)?;

    // Write current_tasks.json
    let tasks_path = ipc_dir.join("current_tasks.json");
    let tasks_data = serde_json::json!({
        "tasks": [{
            "id": input.session_id.clone().unwrap_or_else(|| "interactive".to_string()),
            "prompt": input.prompt,
            "is_scheduled": input.is_scheduled_task
        }]
    });
    let tasks_json = serde_json::to_string_pretty(&tasks_data).map_err(|e| NuClawError::FileSystem {
        message: format!("Failed to serialize tasks: {}", e),
    })?;
    fs::write(&tasks_path, tasks_json).map_err(|e| NuClawError::FileSystem {
        message: format!("Failed to write tasks file: {}", e),
    })?;

    // Write available_groups.json
    let groups_path = ipc_dir.join("available_groups.json");
    let groups_data = serde_json::json!({
        "groups": {
            group_folder: {
                "name": group_folder,
                "registered": true
            }
        }
    });
    let groups_json = serde_json::to_string_pretty(&groups_data).map_err(|e| NuClawError::FileSystem {
        message: format!("Failed to serialize groups: {}", e),
    })?;
    fs::write(&groups_path, groups_json).map_err(|e| NuClawError::FileSystem {
        message: format!("Failed to write groups file: {}", e),
    })?;

    Ok(())
}

/// Prepare group context directory
fn prepare_group_context(group_folder: &str) -> Result<PathBuf> {
    let group_dir = groups_dir().join(group_folder);
    if !group_dir.exists() {
        fs::create_dir_all(&group_dir).map_err(|e| NuClawError::FileSystem {
            message: format!("Failed to create group directory: {}", e),
        })?;
    }
    Ok(group_dir)
}

/// Run a container with the given input
pub async fn run_container(input: ContainerInput) -> Result<ContainerOutput> {
    let group_folder = &input.group_folder;
    let group_dir = prepare_group_context(group_folder)?;
    write_ipc_files(group_folder, &input)?;
    let (mut cmd, input_path) = build_container_command(&input, &group_dir).await?;
    let timeout_duration = container_timeout();
    let output = run_container_with_output(&mut cmd, timeout_duration).await?;
    let _ = fs::remove_file(&input_path);
    Ok(output)
}

async fn build_container_command(
    input: &ContainerInput,
    group_dir: &Path,
) -> Result<(AsyncCommand, PathBuf)> {
    let temp_dir = data_dir().join("temp");
    fs::create_dir_all(&temp_dir).map_err(|e| NuClawError::FileSystem {
        message: format!("Failed to create temp directory: {}", e),
    })?;
    let input_path = temp_dir.join(format!("input_{}.json", input.session_id.clone().unwrap_or_else(|| "default".to_string())));
    let input_json = serde_json::to_string(input).map_err(|e| NuClawError::Container {
        message: format!("Failed to serialize input: {}", e),
    })?;
    fs::write(&input_path, &input_json).map_err(|e| NuClawError::FileSystem {
        message: format!("Failed to write input file: {}", e),
    })?;
    let mut cmd = AsyncCommand::new(get_container_command());
    if cfg!(target_os = "macos") {
        cmd.arg("exec").arg("--workspace").arg(group_dir).arg("--input").arg(&input_path).arg("--name").arg(assistant_name());
    } else {
        let image = std::env::var("CONTAINER_IMAGE").unwrap_or_else(|_| "anthropic/claude-code:latest".to_string());
        cmd.arg("run").arg("--rm").arg("-v").arg(format!("{}:/workspace/group", group_dir.display())).arg("-e").arg("CLAUDE_CODE_OAUTH_TOKEN").arg("-e").arg("ANTHROPIC_API_KEY").arg("--entrypoint").arg("/bin/sh").arg(image).arg("-c").arg("cat /workspace/input.json | /usr/local/bin/claude");
    }
    cmd.stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
    Ok((cmd, input_path))
}

async fn run_container_with_output(cmd: &mut AsyncCommand, timeout_duration: Duration) -> Result<ContainerOutput> {
    let mut child = cmd.spawn().map_err(|e| NuClawError::Container {
        message: format!("Failed to spawn container: {}", e),
    })?;
    let start_time = Instant::now();
    if let Some(mut stdin) = child.stdin.take() {
        let input_path = data_dir().join("temp/input.json");
        if input_path.exists() {
            let input_content = fs::read_to_string(&input_path).unwrap_or_default();
            stdin.write_all(input_content.as_bytes()).await.map_err(|e| NuClawError::Container {
                message: format!("Failed to write to stdin: {}", e),
            })?;
        }
        stdin.shutdown().await.map_err(|e| NuClawError::Container {
            message: format!("Failed to close stdin: {}", e),
        })?;
    }
    let stdout = child.stdout.take().unwrap();
    let output_result = timeout(timeout_duration, capture_output(stdout)).await;
    let exit_status = child.wait().await.map_err(|e| NuClawError::Container {
        message: format!("Failed to wait for container: {}", e),
    })?;
    let duration_ms = start_time.elapsed().as_millis() as i64;
    match output_result {
        Ok(output) => {
            let output = output?;
            parse_container_output(&output, exit_status.success(), duration_ms)
        }
        Err(_) => {
            let _ = child.kill().await;
            parse_container_output("", false, duration_ms)
        }
    }
}

async fn capture_output(stdout: ChildStdout) -> Result<String> {
    let reader = BufReader::new(stdout);
    let mut lines = reader.lines();
    let mut output = String::new();
    let max_size = max_output_size();
    while let Some(line) = lines.next_line().await.ok().flatten() {
        if output.len() + line.len() > max_size {
            output.push_str("\n[OUTPUT TRUNCATED - exceeded max size]");
            break;
        }
        output.push_str(&line);
        output.push('\n');
    }
    Ok(output)
}

fn parse_container_output(output: &str, success: bool, _duration_ms: i64) -> Result<ContainerOutput> {
    if let Some(content) = extract_marked_output(output) {
        return parse_marked_content(&content, success);
    }
    let last_line = output.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("").trim();
    if let Ok(parsed) = serde_json::from_str::<ContainerOutput>(last_line) {
        return Ok(parsed);
    }
    Ok(ContainerOutput {
        status: if success { "success".to_string() } else { "error".to_string() },
        result: Some(output.to_string()),
        new_session_id: None,
        error: if success { None } else { Some("Container execution failed".to_string()) },
    })
}

fn extract_marked_output(output: &str) -> Option<String> {
    let start_idx = output.find(OUTPUT_START_MARKER)?;
    let end_idx = output.find(OUTPUT_END_MARKER)?;
    if start_idx < end_idx {
        Some(output[start_idx + OUTPUT_START_MARKER.len()..end_idx].to_string())
    } else {
        None
    }
}

fn parse_marked_content(content: &str, success: bool) -> Result<ContainerOutput> {
    if let Ok(parsed) = serde_json::from_str::<ContainerOutput>(content) {
        return Ok(parsed);
    }
    Ok(ContainerOutput {
        status: if success { "success".to_string() } else { "error".to_string() },
        result: Some(content.to_string()),
        new_session_id: None,
        error: if success { None } else { Some("Container execution failed".to_string()) },
    })
}

pub fn ensure_container_system_running() -> Result<()> {
    let output = Command::new(get_container_command()).args(&["system", "status"]).output();
    match output {
        Ok(_) => Ok(()),
        Err(_) => {
            let output = Command::new(get_container_command()).args(&["system", "start"]).output();
            match output {
                Ok(_) => Ok(()),
                Err(e) => Err(NuClawError::Container { message: format!("Failed to start container system: {}", e) }),
            }
        }
    }
}

pub fn log_container_output(group_folder: &str, session_id: &str, output: &ContainerOutput) -> Result<()> {
    let log_dir = logs_dir().join(group_folder);
    fs::create_dir_all(&log_dir).map_err(|e| NuClawError::FileSystem {
        message: format!("Failed to create log directory: {}", e),
    })?;
    let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
    let log_path = log_dir.join(format!("container_{}_{}.log", session_id, timestamp));
    let log_data = serde_json::json!({
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "group_folder": group_folder,
        "session_id": session_id,
        "status": output.status,
        "result": output.result,
        "error": output.error,
        "new_session_id": output.new_session_id,
    });
    fs::write(&log_path, serde_json::to_string_pretty(&log_data).unwrap_or_default()).map_err(|e| NuClawError::FileSystem {
        message: format!("Failed to write log file: {}", e),
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_marked_output() {
        let output = "Some prefix\n--NANOCLAW_OUTPUT_START--\n{\"status\": \"success\", \"result\": \"test\"}\n--NANOCLAW_OUTPUT_END--\nSome suffix";
        let extracted = extract_marked_output(output);
        assert!(extracted.is_some());
        assert_eq!(extracted.unwrap(), "\n{\"status\": \"success\", \"result\": \"test\"}\n");
    }

    #[test]
    fn test_extract_marked_output_no_markers() {
        let output = "No markers here";
        let extracted = extract_marked_output(output);
        assert!(extracted.is_none());
    }

    #[test]
    fn test_extract_marked_output_only_start_marker() {
        let output = "--NANOCLAW_OUTPUT_START--\nsome content";
        let extracted = extract_marked_output(output);
        assert!(extracted.is_none());
    }

    #[test]
    fn test_extract_marked_output_reversed_markers() {
        // End marker before start marker should not match
        let output = "--NANOCLAW_OUTPUT_END--\ncontent\n--NANOCLAW_OUTPUT_START--";
        let extracted = extract_marked_output(output);
        assert!(extracted.is_none());
    }

    #[test]
    fn test_extract_marked_output_empty_content() {
        let output = "--NANOCLAW_OUTPUT_START----NANOCLAW_OUTPUT_END--";
        let extracted = extract_marked_output(output);
        assert!(extracted.is_some());
        assert_eq!(extracted.unwrap(), "");
    }

    #[test]
    fn test_container_timeout_default() {
        let timeout = container_timeout();
        assert_eq!(timeout, Duration::from_millis(DEFAULT_TIMEOUT_MS));
    }

    #[test]
    fn test_container_timeout_from_env() {
        // Save original value
        let original = std::env::var("CONTAINER_TIMEOUT").ok();
        
        std::env::set_var("CONTAINER_TIMEOUT", "60000");
        let timeout = container_timeout();
        assert_eq!(timeout, Duration::from_millis(60000));
        
        // Restore original value
        match original {
            Some(val) => std::env::set_var("CONTAINER_TIMEOUT", val),
            None => std::env::remove_var("CONTAINER_TIMEOUT"),
        }
    }

    #[test]
    fn test_container_timeout_invalid_env() {
        // Save original value
        let original = std::env::var("CONTAINER_TIMEOUT").ok();
        
        std::env::set_var("CONTAINER_TIMEOUT", "invalid");
        let timeout = container_timeout();
        // Should fall back to default
        assert_eq!(timeout, Duration::from_millis(DEFAULT_TIMEOUT_MS));
        
        // Restore original value
        match original {
            Some(val) => std::env::set_var("CONTAINER_TIMEOUT", val),
            None => std::env::remove_var("CONTAINER_TIMEOUT"),
        }
    }

    #[test]
    fn test_max_output_size_default() {
        let max_size = max_output_size();
        assert_eq!(max_size, DEFAULT_MAX_OUTPUT);
    }

    #[test]
    fn test_max_output_size_from_env() {
        // Save original value
        let original = std::env::var("CONTAINER_MAX_OUTPUT_SIZE").ok();
        
        std::env::set_var("CONTAINER_MAX_OUTPUT_SIZE", "5242880");
        let max_size = max_output_size();
        assert_eq!(max_size, 5 * 1024 * 1024);
        
        // Restore original value
        match original {
            Some(val) => std::env::set_var("CONTAINER_MAX_OUTPUT_SIZE", val),
            None => std::env::remove_var("CONTAINER_MAX_OUTPUT_SIZE"),
        }
    }

    #[test]
    fn test_parse_container_output_json() {
        let output = r#"{"status": "success", "result": "test result"}"#;
        let result = parse_container_output(output, true, 100);
        assert!(result.is_ok());
        let output = result.unwrap();
        assert_eq!(output.status, "success");
        assert_eq!(output.result, Some("test result".to_string()));
    }

    #[test]
    fn test_parse_container_output_with_session_id() {
        let output = r#"{"status": "success", "result": "test", "new_session_id": "sess_123"}"#;
        let result = parse_container_output(output, true, 100);
        assert!(result.is_ok());
        let output = result.unwrap();
        assert_eq!(output.status, "success");
        assert_eq!(output.new_session_id, Some("sess_123".to_string()));
    }

    #[test]
    fn test_parse_container_output_error() {
        let output = "some error output";
        let result = parse_container_output(output, false, 100);
        assert!(result.is_ok());
        let output = result.unwrap();
        assert_eq!(output.status, "error");
        assert!(output.error.is_some());
    }

    #[test]
    fn test_parse_container_output_marked() {
        let output = "prefix\n--NANOCLAW_OUTPUT_START--\n{\"status\": \"success\", \"result\": \"marked\"}\n--NANOCLAW_OUTPUT_END--\nsuffix";
        let result = parse_container_output(output, true, 100);
        assert!(result.is_ok());
        let parsed = result.unwrap();
        assert_eq!(parsed.status, "success");
        assert_eq!(parsed.result, Some("marked".to_string()));
    }

    #[test]
    fn test_parse_container_output_empty() {
        let output = "";
        let result = parse_container_output(output, true, 100);
        assert!(result.is_ok());
        let parsed = result.unwrap();
        assert_eq!(parsed.status, "success");
        assert_eq!(parsed.result, Some("".to_string()));
    }

    #[test]
    fn test_parse_marked_content_success() {
        let content = r#"{"status": "success", "result": "test output"}"#;
        let result = parse_marked_content(content, true);
        assert!(result.is_ok());
        let parsed = result.unwrap();
        assert_eq!(parsed.status, "success");
        assert_eq!(parsed.result, Some("test output".to_string()));
    }

    #[test]
    fn test_parse_marked_content_invalid_json() {
        let content = "not valid json";
        let result = parse_marked_content(content, true);
        assert!(result.is_ok());
        let parsed = result.unwrap();
        assert_eq!(parsed.status, "success");
        assert_eq!(parsed.result, Some("not valid json".to_string()));
    }

    #[test]
    fn test_get_container_command() {
        // This test just verifies the function doesn't panic
        let cmd = get_container_command();
        assert!(!cmd.is_empty());
        // On Linux it should be "docker", on macOS "container"
        assert!(cmd == "docker" || cmd == "container");
    }

    #[test]
    fn test_create_group_ipc_directory() {
        let result = create_group_ipc_directory("test_group_123");
        assert!(result.is_ok());
        let path = result.unwrap();
        assert!(path.exists());
        
        // Cleanup
        let _ = fs::remove_dir_all(&path);
    }

    #[test]
    fn test_prepare_group_context() {
        let result = prepare_group_context("test_context_group");
        assert!(result.is_ok());
        let path = result.unwrap();
        assert!(path.exists());
        
        // Cleanup
        let _ = fs::remove_dir_all(&path);
    }

    #[test]
    fn test_prepare_group_context_existing() {
        // First call creates the directory
        let path1 = prepare_group_context("existing_group").unwrap();
        // Second call should succeed on existing directory
        let path2 = prepare_group_context("existing_group").unwrap();
        assert_eq!(path1, path2);
        
        // Cleanup
        let _ = fs::remove_dir_all(&path1);
    }

    #[test]
    fn test_write_ipc_files() {
        let input = ContainerInput {
            prompt: "test prompt".to_string(),
            session_id: Some("test_session".to_string()),
            group_folder: "test_ipc_group".to_string(),
            chat_jid: "test@chat".to_string(),
            is_main: true,
            is_scheduled_task: false,
        };
        
        let result = write_ipc_files("test_ipc_group", &input);
        assert!(result.is_ok());
        
        // Verify files were created
        let ipc_dir = create_group_ipc_directory("test_ipc_group").unwrap();
        assert!(ipc_dir.join("current_tasks.json").exists());
        assert!(ipc_dir.join("available_groups.json").exists());
        
        // Cleanup
        let _ = fs::remove_dir_all(&ipc_dir);
        let _ = fs::remove_dir_all(groups_dir().join("test_ipc_group"));
    }

    #[test]
    fn test_log_container_output() {
        let output = ContainerOutput {
            status: "success".to_string(),
            result: Some("test result".to_string()),
            new_session_id: Some("sess_123".to_string()),
            error: None,
        };
        
        let result = log_container_output("test_log_group", "test_session", &output);
        assert!(result.is_ok());
        
        // Verify log file was created
        let log_dir = logs_dir().join("test_log_group");
        assert!(log_dir.exists());
        
        // Cleanup
        let _ = fs::remove_dir_all(&log_dir);
    }

    #[test]
    fn test_log_container_output_error() {
        let output = ContainerOutput {
            status: "error".to_string(),
            result: None,
            new_session_id: None,
            error: Some("test error".to_string()),
        };
        
        let result = log_container_output("test_log_error_group", "test_session", &output);
        assert!(result.is_ok());
        
        // Cleanup
        let log_dir = logs_dir().join("test_log_error_group");
        let _ = fs::remove_dir_all(&log_dir);
    }
}
