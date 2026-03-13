# AGENTS.md

This file provides guidance for AI coding agents working on NuClaw.

## Project Overview

NuClaw is a personal Claude assistant that:
- Connects to WhatsApp/Telegram for messaging
- Runs AI agents in isolated containers (Docker/Apple Container)
- Supports scheduled tasks with cron expressions
- Uses SQLite for persistent storage (rusqlite)
- Provides built-in skills and Provider/Channel registry system

## Build Commands

```bash
# Debug build (default)
cargo build

# Production build
cargo build --release

# Type checking only
cargo check

# Linting with clippy
cargo clippy

# Run clippy with auto-fix
cargo clippy --fix

# Generate documentation
cargo doc --open

# Format code
cargo fmt

# Check formatting
cargo fmt --check
```

## Testing Commands

```bash
# Run all tests
cargo test

# Run a single test module
cargo test skills::
cargo test providers::
cargo test channels::

# Run a single test by name
cargo test test_parse_container_output

# Run tests with output
cargo test -- --nocapture

# Run integration tests only
cargo test --test integration_tests

# Run tests and show coverage (requires tarpaulin)
cargo tarpaulin --no-fail-in --out Html
```

## Module Structure

```
src/
├── main.rs              # CLI entry point
├── lib.rs               # Library exports and module declarations
├── agent_runner.rs      # Agent execution (container + API modes)
├── channels.rs          # Channel trait and registry
├── config.rs            # Configuration and env loading
├── container_runner.rs  # Docker/Apple Container spawning
├── db.rs               # SQLite database operations
├── error.rs            # NuClawError enum
├── logging.rs          # Logging setup
├── providers.rs        # Provider registry (LLM providers)
├── skills.rs           # Built-in skills system
├── task_scheduler.rs   # Cron-based task scheduling
├── telegram.rs         # Telegram Bot API integration
├── types.rs            # Core data structures
├── utils.rs            # JSON/file utilities
└── whatsapp.rs         # WhatsApp MCP integration
```

## Code Style Guidelines

### Imports

```rust
// Standard ordering: std → external → crate
use std::collections::HashMap;
use std::sync::RwLock;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::error::{NuClawError, Result};
```

### Error Handling

```rust
// Use thiserror for error enums
#[derive(Error, Debug)]
pub enum NuClawError {
    #[error("Database error: {message}")]
    Database { message: String },
    
    #[error("API error: {message}")]
    Api { message: String },
}

// Use ? operator and map_err
async fn fetch_data() -> Result<String> {
    let response = client.get(url).send().await
        .map_err(|e| NuClawError::Api {
            message: format!("Request failed: {}", e)
        })?;
    Ok(response)
}

// Implement From traits for ergonomic error conversion
impl From<std::io::Error> for NuClawError {
    fn from(e: std::io::Error) -> Self {
        NuClawError::FileSystem { message: e.to_string() }
    }
}
```

### Async Functions

```rust
// All async functions return Result
async fn process_message(&self) -> Result<Option<String>> {
    // Implementation
}

// Use async_trait for trait methods
#[async_trait]
pub trait AgentRunner: Send + Sync {
    async fn run(&self, input: Input) -> Result<Output>;
}
```

### Naming Conventions

- **Modules**: `snake_case` (e.g., `agent_runner`, `task_scheduler`)
- **Types**: `PascalCase` (e.g., `ScheduledTask`, `ContainerInput`)
- **Functions/Variables**: `snake_case` (e.g., `run_container`, `max_output_size`)
- **Constants**: `SCREAMING_SNAKE_CASE` (e.g., `DEFAULT_TIMEOUT_MS`)
- **Type Parameters**: `PascalCase` (e.g., `T`, `Result<T, E>`)

### Types and Structs

```rust
// Simple structs with derive
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerInput {
    pub prompt: String,
    pub session_id: Option<String>,
    pub group_folder: String,
}

// Use Option for optional fields
// Use Result for fallible operations
// Use HashMap for dynamic key-value collections
```

### Registry Pattern (Two-Step Registration)

```rust
// Step 1: Define spec in PROVIDERS constant
pub const PROVIDERS: &[ProviderSpec] = &[
    ProviderSpec::new("anthropic", "ANTHROPIC_API_KEY", ...),
];

// Step 2: Use environment variables for configuration
// No code changes needed - auto-configured!
```

### Trait Design

```rust
// Keep traits small and focused
#[async_trait]
pub trait Channel: Send + Sync {
    fn name(&self) -> &str;
    async fn send(&self, jid: &str, message: &str) -> Result<()>;
    async fn start(&self) -> Result<()>;
}

// Use Send + Sync for thread-safe shared state
```

### Testing Requirements

```rust
// All new modules must have 100% test coverage
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_function_behavior() {
        // Test normal path
    }

    #[test]
    fn test_edge_cases() {
        // Test boundaries
    }

    #[test]
    fn test_error_handling() {
        // Test error conditions
    }
}
```

## Critical Paths

1. **Message Processing**: `whatsapp.rs` / `telegram.rs` → `agent_runner.rs` → response
2. **Task Scheduling**: `task_scheduler.rs` → polls cron → executes agent
3. **Container Execution**: `container_runner.rs` → spawns Docker → parses output
4. **Agent Runner**: `agent_runner.rs` switches between Container/API modes

## Environment Variables

| Variable | Description |
|----------|-------------|
| `AGENT_RUNNER_MODE` | `container` (default) or `api` |
| `ANTHROPIC_API_KEY` | Anthropic API key for API mode |
| `ANTHROPIC_BASE_URL` | Custom Anthropic API endpoint |
| `NUCLAW_HOME` | NuClaw config directory (default: `~/.nuclaw`) |
| `CLAUDE_MODEL` | Model name (default: `claude-sonnet-4-20250514`) |

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

### Configuration Functions

```rust
pub fn config_function() -> ReturnType {
    std::env::var("ENV_VAR")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_VALUE)
}
```

## Forbidden Patterns

- **NEVER** use `as any`, `@ts-ignore`, `@ts-expect-error` (not applicable to Rust)
- **NEVER** use empty catch blocks: `catch(e) {}`
- **NEVER** suppress clippy warnings without good reason
- **NEVER** commit without running tests first
- **NEVER** leave code in broken state

## Before Submitting

1. Run `cargo check` - no errors
2. Run `cargo clippy` - fix warnings
3. Run `cargo fmt` - ensure formatting
4. Run `cargo test` - all tests pass
5. Review changes - minimal, focused commits

## Additional Patterns

### Rust API Service Patterns

For reference when building API services with Rust:

#### Critical Rules

- Use `thiserror` for library errors, `anyhow` only in binary crates or tests
- No `.unwrap()` or `.expect()` in production code — propagate errors with `?`
- Prefer `&str` over `String` in function parameters; return `String` when ownership transfers
- Use `clippy` with `#![deny(clippy::all, clippy::pedantic)]` — fix all warnings
- Derive `Debug` on all public types; derive `Clone`, `PartialEq` only when needed
- No `unsafe` blocks unless justified with a `// SAFETY:` comment

#### Database Patterns

- All queries use SQLx with `query!` or `query_as!` macros
- Use migrations in `migrations/` using `sqlx migrate` — never alter database directly
- Use `sqlx::Pool<Postgres>` as shared state — never create connections per request
- All queries use parameterized placeholders (`$1`, `$2`) — never string formatting

```rust
// BAD: String interpolation (SQL injection risk)
let q = format!("SELECT * FROM users WHERE id = '{}'", id);

// GOOD: Parameterized query, compile-time checked
let user = sqlx::query_as!(User, "SELECT * FROM users WHERE id = $1", id)
    .fetch_optional(&pool)
    .await?;
```

#### Error Handling

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("Resource not found")]
    NotFound,
    #[error("Validation failed: {0}")]
    Validation(String),
    #[error("Unauthorized")]
    Unauthorized,
    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            Self::NotFound => (StatusCode::NOT_FOUND, self.to_string()),
            Self::Validation(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            Self::Unauthorized => (StatusCode::UNAUTHORIZED, self.to_string()),
            Self::Internal(err) => {
                tracing::error!(?err, "internal error");
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal error".into())
            }
        };
        (status, Json(json!({ "error": message }))).into_response()
    }
}
```

#### Layered Architecture

```
src/
├── main.rs              # Entrypoint, server setup
├── lib.rs               # Re-exports
├── config.rs            # Environment config
├── router.rs            # Axum router
├── middleware/          # Auth, logging, etc.
├── handlers/            # Route handlers (thin)
├── services/            # Business logic
├── repositories/        # Database access
└── domain/              # Domain types, errors
```

#### Testing Strategy

- Unit tests in `#[cfg(test)]` modules
- Integration tests in `tests/` directory
- Use `#[sqlx::test]` for database tests
- Mock external services with `mockall`

#### Code Style

- Max line length: 100 characters (rustfmt)
- Group imports: `std` → external → `crate`
- Modules: one file per module
- Types: PascalCase, functions: snake_case, constants: UPPER_SNAKE_CASE
