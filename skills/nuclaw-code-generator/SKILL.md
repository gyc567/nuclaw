---
name: nuclaw-code-generator
description: "Generate NuClaw-compliant Rust code. Use when: create a new module, add a new provider, implement a channel handler, write database operations, add configuration, write tests. Triggers: write rust code for nuclaw, implement this feature in nuclaw, add a new channel/provider, generate tests for nuclaw module. Output: complete, compilable Rust code following NuClaw conventions."
---

# NuClaw Code Generator

Generate production-quality Rust code for the NuClaw personal assistant project.

---

## NuClaw Architecture Overview

NuClaw is a Rust-based personal AI assistant with these core components:

- **Providers**: LLM API integrations (Anthropic, OpenAI, OpenRouter)
- **Channels**: Messaging integrations (WhatsApp, Telegram, Discord)
- **Database**: SQLite persistence with r2d2 connection pooling
- **Container Runner**: Docker/Apple Container management
- **Task Scheduler**: Cron-based scheduled task execution
- **Skills System**: Extensible skill registry

---

## Import Organization (MUST FOLLOW)

```rust
// Standard ordering: std → external → crate
use std::collections::HashMap;
use std::sync::RwLock;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::error::{NuClawError, Result};
```

**Rule**: 
- `std` imports first
- Then external crates (alphabetically within version groups)
- Then `crate` imports last
- Separate groups with blank lines

---

## Error Handling Patterns

### Error Enum with thiserror

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum YourErrorType {
    #[error("Description: {variable}")]
    TypeName { variable: String },
    
    #[error("Another error: {0}")]
    Simple(String),
}

pub type Result<T> = std::result::Result<T, YourErrorType>;
```

### From Implementations

```rust
impl From<std::io::Error> for YourErrorType {
    fn from(e: std::io::Error) -> Self {
        YourErrorType::TypeName {
            variable: e.to_string(),
        }
    }
}
```

### Error Mapping

```rust
// Map external errors to NuClawError
.map_err(|e| NuClawError::Database {
    message: format!("Failed to do thing: {}", e),
})?;

// Chain errors with context
.ok_or_else(|| NuClawError::NotFound {
    message: format!("Item {} not found", id),
})?;
```

**Anti-patterns (NEVER do these):**
- ❌ `.unwrap()` in production code
- ❌ `.expect("should never fail")` 
- ❌ `panic!()` except in truly unrecoverable situations
- ❌ Empty catch blocks

---

## Trait Design

### Basic Trait Pattern

```rust
#[async_trait]
pub trait YourTrait: Send + Sync {
    fn name(&self) -> &str;
    async fn method(&self, param: &str) -> Result<String>;
    fn helper(&self) -> bool {
        true // Default implementation allowed
    }
}
```

### Trait with Builder Pattern

```rust
#[derive(Debug, Clone)]
pub struct TraitConfig {
    pub field1: String,
    pub field2: u32,
}

impl Default for TraitConfig {
    fn default() -> Self {
        Self {
            field1: "default".to_string(),
            field2: 10,
        }
    }
}

pub trait WithConfig {
    fn with_field1(mut self, val: String) -> Self;
    fn with_field2(mut self, val: u32) -> Self;
}

impl WithConfig for TraitConfig {
    fn with_field1(mut self, val: String) -> Self {
        self.field1 = val;
        self
    }
    fn with_field2(mut self, val: u32) -> Self {
        self.field2 = val;
        self
    }
}
```

---

## Async Functions

```rust
// All async functions return Result
pub async fn async_operation(&self, input: &str) -> Result<Output> {
    // Implementation
}

// Use async_trait for trait methods
#[async_trait]
impl YourTrait for YourStruct {
    async fn method(&self, param: &str) -> Result<String> {
        let result = self.client.get(param).await
            .map_err(|e| NuClawError::Api {
                message: format!("Request failed: {}", e),
            })?;
        Ok(result)
    }
}
```

---

## Struct Patterns

### Simple Structs with Derives

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YourStruct {
    pub field1: String,
    pub field2: Option<u32>,
    #[serde(rename = "customName")]
    pub field3: Vec<String>,
}

#[derive(Default)]
struct PrivateStruct {
    field: String,
}
```

### Builder Pattern

```rust
pub struct Builder {
    config: YourConfig,
}

impl Builder {
    pub fn new() -> Self {
        Self {
            config: YourConfig::default(),
        }
    }

    pub fn with_field(mut self, value: impl Into<String>) -> Self {
        self.config.field = value.into();
        self
    }

    pub fn build(self) -> Result<YourType> {
        if self.config.field.is_empty() {
            return Err(NuClawError::Validation {
                message: "field cannot be empty".to_string(),
            });
        }
        Ok(YourType { config: self.config })
    }
}
```

---

## Configuration Patterns

### Env Variable Loading

```rust
pub fn config_function() -> ReturnType {
    std::env::var("ENV_VAR")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_VALUE)
}

pub fn optional_config() -> Option<String> {
    env::var("OPTIONAL_VAR").ok()
}
```

### Config Struct with Defaults

```rust
#[derive(Debug, Clone)]
pub struct Config {
    pub pool_size: u32,
    pub timeout_ms: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            pool_size: std::env::var("POOL_SIZE")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(10),
            timeout_ms: std::env::var("TIMEOUT_MS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(30000),
        }
    }
}
```

---

## Registry Patterns

### Singleton Registry

```rust
use std::sync::RwLock;

pub struct Registry {
    items: RwLock<HashMap<String, Box<dyn ItemTrait>>>,
}

impl Registry {
    pub fn new() -> Self {
        Self {
            items: RwLock::new(HashMap::new()),
        }
    }

    pub fn register<T: ItemTrait + 'static>(&self, item: T) -> &Self {
        if let Ok(mut items) = self.items.write() {
            items.insert(item.name().to_string(), Box::new(item));
        }
        self
    }

    pub fn get(&self, name: &str) -> Option<Box<dyn ItemTrait>> {
        self.items.read().ok()?.get(name).cloned()
    }

    pub fn list(&self) -> Vec<String> {
        self.items.read().ok()?.keys().cloned().collect()
    }
}

impl Default for Registry {
    fn default() -> Self {
        Self::new()
    }
}
```

### Spec-Based Registration

```rust
pub const SPECS: &[Spec] = &[
    Spec::new("name1", "env_var1", "description1"),
    Spec::new("name2", "env_var2", "description2"),
];

#[derive(Debug, Clone)]
pub struct Spec {
    pub name: &'static str,
    pub env_var: &'static str,
    pub description: &'static str,
}

impl Spec {
    pub const fn new(name: &'static str, env_var: &'static str, description: &'static str) -> Self {
        Self { name, env_var, description }
    }
}
```

---

## Database Patterns

### Connection Pool with Config

```rust
use r2d2::{Pool, PooledConnection};
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::Connection;

pub struct Database {
    pool: Pool<SqliteConnectionManager>,
    config: DatabaseConfig,
}

impl Database {
    pub fn new() -> Result<Self> {
        Self::with_config(DatabaseConfig::default())
    }

    pub fn with_config(config: DatabaseConfig) -> Result<Self> {
        let manager = SqliteConnectionManager::file(&config.db_path)
            .with_init(|conn| {
                conn.pragma_update(None, "foreign_keys", "ON")?;
                conn.pragma_update(None, "journal_mode", "WAL")?;
                Ok(())
            });

        let pool = Pool::builder()
            .max_size(config.pool_size)
            .connection_timeout(std::time::Duration::from_millis(
                config.connection_timeout_ms,
            ))
            .build(manager)
            .map_err(|e| NuClawError::Database {
                message: format!("Pool error: {}", e),
            })?;

        Ok(Database { pool, config })
    }

    pub fn get_connection(&self) -> Result<PooledConnection<SqliteConnectionManager>> {
        self.pool.get().map_err(|e| NuClawError::Database {
            message: format!("Connection error: {}", e),
        })
    }
}
```

### SQL Operations

```rust
pub fn execute_query(&self, sql: &str, params: impl rusqlite::Params) -> Result<()> {
    self.get_connection()?.execute(sql, params)
        .map_err(|e| NuClawError::Database {
            message: format!("Query failed: {}", e),
        })?;
    Ok(())
}

pub fn query_single<T: FromSql>(&self, sql: &str, params: impl rusqlite::Params) -> Result<Option<T>> {
    let mut stmt = self.get_connection()?.prepare(sql)
        .map_err(|e| NuClawError::Database {
            message: format!("Prepare failed: {}", e),
        })?;
    
    let result = stmt.query_row(params, |row| row.get(0))
        .map_err(|e| NuClawError::Database {
            message: format!("Query failed: {}", e),
        })?;
    
    Ok(Some(result))
}
```

---

## Testing Patterns

### Module Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn setup() {
        let _guard = ENV_LOCK.lock().unwrap();
        // Setup test environment
    }

    #[test]
    fn test_basic_functionality() {
        setup();
        let result = function_under_test("input");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), expected_value);
    }

    #[test]
    fn test_error_handling() {
        setup();
        let result = function_under_test("invalid");
        assert!(result.is_err());
        match result.unwrap_err() {
            YourError::TypeName { .. } => {},
            _ => panic!("Expected specific error"),
        }
    }

    #[test]
    fn test_edge_cases() {
        // Empty input
        let result = function_under_test("");
        assert!(result.is_err());
        
        // Max values
        let result = function_under_test("x".repeat(1000));
        assert!(result.is_ok());
    }
}
```

### Async Tests

```rust
#[cfg(test)]
mod async_tests {
    use super::*;

    #[tokio::test]
    async fn test_async_operation() {
        let service = Service::new().await.unwrap();
        let result = service.operation("test").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_async_error() {
        let service = Service::new().await.unwrap();
        let result = service.operation("fail").await;
        assert!(result.is_err());
    }
}
```

---

## Module Structure

### File Header

```rust
//! Module documentation

use std::collections::HashMap;
use std::sync::RwLock;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::error::{NuClawError, Result};
```

### Adding to lib.rs

When adding a new module, include in `src/lib.rs`:
```rust
pub mod your_module;
pub use your_module::{PublicType, public_function};
```

---

## Naming Conventions

| Type | Convention | Example |
|------|------------|---------|
| Modules | `snake_case` | `agent_runner`, `task_scheduler` |
| Types | `PascalCase` | `ContainerConfig`, `ProviderSpec` |
| Functions | `snake_case` | `create_runner`, `load_config` |
| Variables | `snake_case` | `max_size`, `connection_timeout` |
| Constants | `SCREAMING_SNAKE_CASE` | `DEFAULT_TIMEOUT_MS`, `MAX_RETRIES` |
| Type Parameters | `PascalCase` | `T`, `Result<T, E>` |

---

## Output Format

When generating code:

1. **Start with module documentation** (`//! Module description`)
2. **Follow import order** (std → external → crate)
3. **Define error types first** if needed
4. **Implement traits** with `#[async_trait]`
5. **Add inline tests** in `#[cfg(test)]` blocks
6. **Use `Result<T>` not `Option<T>`** for fallible operations
7. **Include doc comments** (`///`) on public API

---

## Common Patterns Quick Reference

### HashMap with RwLock

```rust
use std::collections::HashMap;
use std::sync::RwLock;

pub struct Registry {
    items: RwLock<HashMap<String, Item>>,
}
```

### Option Chaining

```rust
let value = self.items
    .read()
    .ok()?
    .get(name)
    .cloned();
```

### Environment Variable Fallback

```rust
let value = std::env::var("VAR")
    .ok()
    .and_then(|v| v.parse().ok())
    .unwrap_or(default);
```

### Safe Clone

```rust
// For Arc/Rc types
self.inner.clone()

// For simple types  
value.to_string()

// For Option
opt.as_ref().map(|v| v.clone())
```
