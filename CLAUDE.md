# CLAUDE.md

This file provides guidance for Claude Code when working with this codebase.

## Project Overview

NuClaw is a personal Claude assistant that:
- Connects to WhatsApp for messaging
- Runs AI agents in isolated containers (Docker/Apple Container)
- Supports scheduled tasks with cron expressions
- Uses SQLite for persistent storage

## Core Architecture

### Module Structure

```
src/
├── main.rs           # Application entry point, CLI parsing
├── config.rs         # Configuration constants and env loading
├── types.rs          # Core data structures (ScheduledTask, RegisteredGroup, etc.)
├── db.rs             # SQLite database operations with sqlx
├── utils.rs          # JSON, file, and string utilities
├── error.rs          # Custom error types (NuClawError)
├── whatsapp.rs       # WhatsApp Web connection and message handling
├── whatsapp_auth.rs  # QR code authentication
├── container_runner.rs # Container spawning and IPC
├── task_scheduler.rs # Cron-based task scheduling
└── mount_security.rs # Additional mount validation
```

### Key Types

- `ScheduledTask` - Task with cron/interval schedule
- `RegisteredGroup` - Chat group with isolated context
- `ContainerConfig` - Per-group container configuration
- `ContainerInput/Output` - Agent execution data
- `NuClawError` - Error enum with source chaining

### Async Runtime

- Uses `tokio` for async operations
- Database: `sqlx` with `runtime-tokio`
- Process spawning: `tokio::process`
- Timers: `tokio::time`

## Code Guidelines

### Error Handling

```rust
// Use custom error types
use crate::error::{NuClawError, Result};

fn example() -> Result<String> {
    Err(NuClawError::validation("Invalid input"))
}
```

### Async Functions

```rust
// Always return Result in async functions
async fn process_message(&self) -> Result<Option<String>> {
    // Implementation
}
```

### Database Operations

```rust
// Use sqlx for type-safe queries
let task = sqlx::query_as!(ScheduledTask, "SELECT * FROM tasks WHERE id = ?", id)
    .fetch_one(&self.pool)
    .await?;
```

### Container Spawning

```rust
// Use tokio::process for async command execution
use tokio::process::Command;

let output = Command::new("docker")
    .args(&["run", "--rm", "image"])
    .output()
    .await?;
```

## Critical Paths

1. **Message Processing** (`whatsapp.rs::process_message`)
   - Receives message → builds prompt → runs agent → sends response

2. **Task Scheduling** (`task_scheduler.rs`)
   - Polls due tasks → runs container agent → logs results

3. **Container Execution** (`container_runner.rs`)
   - Builds mounts → spawns container → parses output

4. **Mount Validation** (`mount_security.rs`)
   - Loads allowlist → validates paths → returns validated mounts

## Testing Strategy

- Unit tests for pure functions
- Integration tests for database operations
- No E2E tests (requires WhatsApp connection)

Run tests: `cargo test`

## Build Commands

```bash
cargo build --release    # Production build
cargo build              # Debug build
cargo check              # Type checking only
cargo clippy             # Linting
cargo doc --open         # Generate documentation
```

## Common Patterns

### State Management

```rust
// Load state from disk
let state = load_json(path, Default::default());

// Save state to disk
save_json(path, &state);
```

### Async Initialization

```rust
#[tokio::main]
async fn main() -> Result<()> {
    let db = Database::new().await?;
    // Continue...
}
```

### Error Context

```rust
.map_err(|e| NuClawError::container_with_source("Failed to run agent", e))
```
