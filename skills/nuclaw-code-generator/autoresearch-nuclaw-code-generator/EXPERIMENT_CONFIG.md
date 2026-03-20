# Autoresearch Experiment for nuclaw-code-generator

## Experiment Configuration

- **Target Skill**: `skills/nuclaw-code-generator/SKILL.md`
- **Runs per Experiment**: 5
- **Max Score**: 25 (5 evals × 5 runs)
- **Budget Cap**: 10 experiments

---

## Eval Criteria (Binary Yes/No)

### EVAL 1: Compilation Success
**Question**: Does the generated code compile without errors?
**Pass**: The generated code contains no syntax errors, type errors, or import errors that would prevent `cargo check` from passing
**Fail**: The code has syntax errors, missing imports, type mismatches, or other compilation failures

### EVAL 2: Import Organization
**Question**: Does the code follow NuClaw's import order convention (std → external → crate)?
**Pass**: All imports are organized in correct order: std imports first, then external crates (async_trait, serde, thiserror, etc.), then crate imports
**Fail**: Imports are disorganized, missing, or in wrong order

### EVAL 3: Error Handling
**Question**: Does the code handle errors properly without unwrap/expect in production?
**Pass**: All fallible operations use `?` operator or `map_err`, with no `.unwrap()` or `.expect()` in non-test code
**Fail**: Contains `.unwrap()`, `.expect()`, or bare `panic!()` in production code paths

### EVAL 4: Documentation
**Question**: Does public API have appropriate documentation?
**Pass**: All public structs, enums, and functions have `///` doc comments explaining their purpose
**Fail**: Public API lacks documentation or has placeholder comments

### EVAL 5: Pattern Compliance
**Question**: Does the code follow NuClaw's established patterns (traits, registries, configs)?
**Pass**: Code uses correct patterns from NuClaw: `#[async_trait]`, `thiserror` enums, `RwLock<HashMap>` registries, `Result<T>` returns, proper naming conventions
**Fail**: Code deviates from NuClaw patterns or uses non-idiomatic Rust

---

## Test Scenarios

### Scenario 1: New Provider Implementation
**Prompt**: Create a new LLM provider implementation for "Google Gemini" following the NuClaw provider pattern. Include the Provider trait implementation, API client setup, and error handling.

### Scenario 2: Channel Handler
**Prompt**: Implement a new Discord channel handler with the Channel trait, including send, start, and is_enabled methods.

### Scenario 3: Database Operations
**Prompt**: Add a new database table for storing user preferences with CRUD operations, connection pooling, and proper error handling.

### Scenario 4: Configuration Module
**Prompt**: Create a configuration module for a new feature that loads settings from environment variables with defaults and validation.

### Scenario 5: Test Module
**Prompt**: Write a comprehensive test module for an existing service that includes unit tests, async tests, and error handling tests.

---

## Expected Baseline

Based on the skill content, expected baseline score: ~15-18/25 (60-72%)

Common expected failures:
- Import organization may not be perfectly followed
- Some generated code may use `.unwrap()` instead of proper error handling
- Documentation may be incomplete
