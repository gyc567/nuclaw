# OpenSpec: Architecture Optimization (Phase 2) - Event-Driven Routing

## 1. Executive Summary
Following the Phase 1 abstraction of `Runtime`, Phase 2 focuses on standardizing the input from various communication channels (Telegram, WhatsApp). We introduce `AppEvent` to decouple platform-specific logic from the execution flow. The `EventRouter` will now be responsible for converting these events into execution requests, keeping the chat clients thin and focused purely on I/O.

## 2. Goals
- **KISS**: Use simple enums and match patterns instead of complex registry systems.
- **High Cohesion**: Centralize "what happens when a message arrives" logic in the Router.
- **Low Coupling**: Chat clients shouldn't know about `ContainerInput` or how to build prompts.
- **100% Testability**: Enable testing of the entire business logic (routing + prompt building) without Docker or real Chat APIs.

## 3. Detailed Design

### 3.1. `AppEvent` Enum
Located in `src/types.rs`.
```rust
pub enum AppEvent {
    ChatMessage {
        platform: String, // "telegram", "whatsapp"
        chat_id: String,
        user_id: String,
        message_text: String,
        is_group: bool,
    },
    ScheduledTask {
        task_id: String,
    }
}
```

### 3.2. `EventRouter` Enhancement
The Router will gain a `dispatch` method:
```rust
impl EventRouter {
    pub async fn dispatch(&self, event: AppEvent) -> Result<ContainerOutput> {
        let input = self.map_event_to_input(event).await?;
        self.runtime.run(input).await
    }

    async fn map_event_to_input(&self, event: AppEvent) -> Result<ContainerInput> {
        // Logic to decide group_folder, prompt wrapping, etc.
    }
}
```

## 4. Implementation Plan
1.  Update `src/types.rs` with `AppEvent`.
2.  Enhance `src/router.rs` with `dispatch` and event mapping logic.
3.  Add unit tests for `dispatch` ensuring different events produce correct `ContainerInput`.
4.  Update `docs/TEST_REPORT_ARCH_PHASE2.md`.
