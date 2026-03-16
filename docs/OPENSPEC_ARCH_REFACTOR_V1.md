# OpenSpec: Architecture Optimization (Phase 1) - Event Router & Runtime Abstraction

## 1. Executive Summary
This proposal aims to refactor the core interaction between communication channels (Telegram, WhatsApp), task scheduling, and the container execution engine. Currently, chat clients directly invoke the container runtime (`run_container`), creating tight coupling and making unit testing difficult without a real Docker daemon. 

We will introduce a `Runtime` trait (Strategy Pattern) and an `EventRouter` (Mediator Pattern) to decouple these components. This adheres to KISS (Keep It Simple, Stupid) and SOLID principles, specifically Dependency Inversion Principle (DIP).

## 2. Architectural Audit & Improvements
**Original Proposal Audit:**
*   **Unified Channel Trait**: Good long-term, but might over-engineer the immediate need. We only need the channels to *push* events to a central place, not necessarily share a complex trait yet.
*   **Event Router (Decoupling Message Handling)**: Highly recommended. It acts as a Mediator, taking generic messages from channels and routing them to the correct agent/workflow.
*   **Task Scheduler Consolidation**: This is a large, risky change. We will defer merging `task_scheduler` and `orchestrator` to a later phase to minimize blast radius and ensure stability ("do not affect unrelated features").
*   **Runtime Trait**: Critical. It enables 100% test coverage by allowing Mock runtimes in tests, avoiding expensive and brittle integration tests.

**Actionable Plan (Phase 1):**
1.  **`Runtime` Trait**: Define a trait for container execution (`run`). Implement `DockerRuntime` (wrapping the existing `run_container`) and `MockRuntime` (for tests).
2.  **`EventRouter`**: A simple struct that takes a generic event (e.g., a chat message), determines the target group/agent, and uses a generic `Runtime` to execute the task.

## 3. Detailed Design (KISS & High Cohesion/Low Coupling)

### 3.1. `Runtime` Trait
```rust
use async_trait::async_trait;
use crate::types::{ContainerInput, ContainerOutput};
use crate::error::Result;

#[async_trait]
pub trait Runtime: Send + Sync {
    async fn run(&self, input: ContainerInput) -> Result<ContainerOutput>;
}
```

### 3.2. `EventRouter`
```rust
use crate::runtime::Runtime;
use std::sync::Arc;
use crate::error::Result;
use crate::types::{ContainerInput, ContainerOutput};

pub struct EventRouter {
    runtime: Arc<dyn Runtime>,
}

impl EventRouter {
    pub fn new(runtime: Arc<dyn Runtime>) -> Self {
        Self { runtime }
    }

    pub async fn handle_event(&self, input: ContainerInput) -> Result<ContainerOutput> {
        // Here we can add routing logic, metrics, logging, etc.
        self.runtime.run(input).await
    }
}
```

## 4. Implementation Steps
1.  Create `src/runtime.rs` containing the `Runtime` trait, `DockerRuntime`, and `MockRuntime` (cfg(test)).
2.  Create `src/router.rs` containing the `EventRouter`.
3.  Add tests for `EventRouter` using `MockRuntime` to guarantee 100% test coverage for the new module.
4.  Expose the new modules in `src/lib.rs`.
5.  *(Optional but recommended)* Gradually inject `EventRouter` into `whatsapp.rs` and `telegram/client.rs`. To maintain safety, we will only replace the direct `run_container` call with `EventRouter` where it's safe, or provide it as an alternative.

## 5. Testing Strategy
*   Unit tests for `EventRouter` with `MockRuntime`.
*   Verify that `MockRuntime` accurately records invocations.
*   Ensure 100% statement/branch coverage on `src/router.rs` and `src/runtime.rs`.

## 6. Impact Assessment
*   **Risk**: Low. The actual Docker execution logic remains unchanged (just wrapped).
*   **Benefits**: Dramatically improves testability. Paves the way for multi-agent workflows and alternative runtimes (e.g., local process, WASM) without rewriting core logic.
