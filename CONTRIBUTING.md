# Contributing to NuClaw

Thank you for your interest in contributing to NuClaw! This document provides guidelines and instructions for contributing to the project.

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Getting Started](#getting-started)
- [Development Setup](#development-setup)
- [Coding Standards](#coding-standards)
- [Testing](#testing)
- [Submitting Changes](#submitting-changes)
- [Commit Message Guidelines](#commit-message-guidelines)
- [Release Process](#release-process)

## Code of Conduct

This project and everyone participating in it is governed by our commitment to:
- Be respectful and inclusive
- Welcome newcomers
- Focus on constructive feedback
- Prioritize user safety and privacy

## Getting Started

1. **Fork the repository** on GitHub
2. **Clone your fork** locally:
   ```bash
   git clone https://github.com/your-username/nuclaw.git
   cd nuclaw
   ```
3. **Create a branch** for your changes:
   ```bash
   git checkout -b feature/your-feature-name
   ```

## Development Setup

### Prerequisites

- Rust 1.70 or higher
- SQLite3 development libraries
- Docker or Apple Container (for testing container features)

### Building

```bash
# Create required directories
mkdir -p store data groups logs

# Build in debug mode for development
cargo build

# Build in release mode
cargo build --release
```

### Running Tests

```bash
# Run all tests
cargo test

# Run tests with output
cargo test -- --nocapture

# Run specific test
cargo test test_parse_telegram_update

# Run tests for specific module
cargo test telegram
```

## Coding Standards

### Rust Style Guide

- Follow the [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- Use `cargo fmt` to format code
- Use `cargo clippy` to catch common mistakes

```bash
# Format code
cargo fmt

# Check formatting
cargo fmt -- --check

# Run clippy
cargo clippy -- -D warnings
```

### Documentation

- Document all public APIs with doc comments (`///`)
- Include examples in documentation where appropriate
- Keep README.md updated with new features

```rust
/// Brief description of the function
///
/// # Arguments
///
/// * `arg` - Description of the argument
///
/// # Returns
///
/// Description of the return value
///
/// # Examples
///
/// ```
/// let result = my_function("test");
/// assert_eq!(result, expected);
/// ```
pub fn my_function(arg: &str) -> String {
    // implementation
}
```

### Error Handling

- Use the crate's `Result` type alias and `NuClawError` enum
- Provide descriptive error messages
- Use `thiserror` for error definitions

```rust
use crate::error::{NuClawError, Result};

pub fn risky_operation() -> Result<()> {
    some_fallible_op().map_err(|e| NuClawError::Container {
        message: format!("Failed to execute: {}", e),
    })
}
```

## Testing

### Test Organization

Tests are organized in the same file as the code they test, using `#[cfg(test)]` modules:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_feature() {
        // Test implementation
    }
}
```

### Writing Tests

- Test both success and failure cases
- Use descriptive test names
- Keep tests independent and isolated

```rust
#[test]
fn test_container_timeout_respected() {
    let start = Instant::now();
    let result = run_container_with_timeout(input, Duration::from_millis(100));
    let elapsed = start.elapsed();
    
    assert!(result.is_err());
    assert!(elapsed < Duration::from_millis(200));
}
```

### Integration Testing

For integration tests that require external services:

1. Mark them with `#[ignore]` attribute
2. Document required environment variables
3. Provide setup instructions

```rust
#[test]
#[ignore = "Requires WhatsApp MCP server"]
fn test_whatsapp_integration() {
    // Test implementation
}
```

## Submitting Changes

### Pull Request Process

1. **Update documentation** for any changed functionality
2. **Add tests** for new features
3. **Ensure all tests pass**:
   ```bash
   cargo test
   cargo clippy
   cargo fmt -- --check
   ```
4. **Update CHANGELOG.md** if applicable
5. **Submit pull request** with clear description

### Pull Request Template

```markdown
## Description
Brief description of changes

## Type of Change
- [ ] Bug fix
- [ ] New feature
- [ ] Breaking change
- [ ] Documentation update

## Testing
- [ ] Unit tests pass
- [ ] Integration tests pass
- [ ] Manual testing performed

## Checklist
- [ ] Code follows style guidelines
- [ ] Self-review completed
- [ ] Documentation updated
- [ ] Tests added/updated
```

## Commit Message Guidelines

We follow the [Conventional Commits](https://www.conventionalcommits.org/) specification:

### Format

```
<type>(<scope>): <description>

[optional body]

[optional footer]
```

### Types

- **feat**: New feature
- **fix**: Bug fix
- **docs**: Documentation changes
- **style**: Code style changes (formatting, etc.)
- **refactor**: Code refactoring
- **test**: Test changes
- **chore**: Build/tooling changes

### Examples

```
feat(telegram): add webhook retry mechanism

fix(container): handle timeout edge case

docs(readme): update installation instructions

test(scheduler): add cron parsing tests
```

## Release Process

1. Update version in `Cargo.toml`
2. Update `CHANGELOG.md` with release notes
3. Create a git tag:
   ```bash
   git tag -a v1.0.0 -m "Release version 1.0.0"
   git push origin v1.0.0
   ```
4. GitHub Actions will build and publish the release

## Questions?

- Open an [issue](https://github.com/gyc567/nuclaw/issues) for bugs or feature requests
- Start a [discussion](https://github.com/gyc567/nuclaw/discussions) for questions

Thank you for contributing to NuClaw!
