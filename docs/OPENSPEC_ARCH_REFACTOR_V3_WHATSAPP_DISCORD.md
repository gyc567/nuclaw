# OpenSpec: Architecture Optimization (Phase 3) - Universal Event Routing (WhatsApp & Discord)

## 1. Executive Summary
Building on Phase 2, which successfully decoupled the Telegram client via the `AppEvent` and `EventRouter` patterns, Phase 3 extends this clean architecture to the `WhatsApp` client and designs a clear contract for the future `Discord` client. 

During our architectural audit, we identified two areas for optimization:
1.  **Abstraction Leaks**: WhatsApp computes an `is_main` flag for the underlying container execution. This couples the chat client to container logic. We will replace this by passing a semantic `is_group` flag in `AppEvent`, allowing the Router to make execution decisions.
2.  **Lifecycle Management**: Chat clients currently instantiate the `EventRouter` per message. For better performance and testing, the `EventRouter` should be injected into the clients at startup.

## 2. Goals
-   **KISS**: Ensure WhatsApp and Discord clients do absolutely zero container manipulation. They just parse JSON/WebSockets and emit `AppEvent`.
-   **High Cohesion**: Centralize the translation of platform-specific chat contexts (e.g., WhatsApp DMs vs. Groups) into unified Container execution requests.
-   **Low Coupling**: Inject `EventRouter` as a dependency into clients to achieve 100% unit-testability for the clients themselves.

## 3. Detailed Design & Refinements

### 3.1. Enhancing `AppEvent::ChatMessage`
We must restore the semantic context of the chat type (Group vs. Direct Message) without leaking the internal `is_main` container flag.

```rust
// In src/types.rs
pub enum AppEvent {
    ChatMessage {
        platform: String,     // "whatsapp", "telegram", "discord"
        chat_id: String,
        user_id: String,
        message_id: String,
        message_text: String,
        group_folder: String,
        is_group: bool,       // <-- NEW: Semantic flag for Router decision making
    },
    // ...
}
```

### 3.2. Router Intelligence Enhancement
The `EventRouter` (`src/router.rs`) will use the `is_group` flag to determine the `is_main` property for the `ContainerInput`.

```rust
// In src/router.rs
async fn map_event_to_input(&self, event: AppEvent) -> Result<ContainerInput> {
    match event {
        AppEvent::ChatMessage { platform, chat_id, message_id, message_text, group_folder, is_group, .. } => {
            Ok(ContainerInput {
                prompt: message_text,
                session_id: Some(format!("{}_{}", platform, message_id)),
                group_folder,
                chat_jid: chat_id,
                is_main: !is_group, // Example translation: DMs are 'main'
                is_scheduled_task: false,
            })
        }
        // ...
    }
}
```

### 3.3. WhatsApp Refactoring (`src/whatsapp.rs`)
The WhatsApp client will be refactored to:
1.  Accept `Arc<EventRouter>` in its constructor.
2.  Calculate `is_group` (`!msg.chat_jid.ends_with("@s.whatsapp.net")`).
3.  Dispatch `AppEvent::ChatMessage`.
4.  Remove all direct references to `run_container` and `ContainerInput`.

### 3.4. Discord Client Skeleton (`src/discord.rs`)
To prove the architecture's extensibility, we will draft a foundational `src/discord.rs` module. It will define the client structure and its dependency on `EventRouter`. When actual Discord APIs are connected, they will seamlessly hook into the `AppEvent` flow.

## 4. Implementation & Testing Strategy
1.  **Modify `types.rs`**: Add `is_group` back to `AppEvent::ChatMessage`.
2.  **Update `router.rs`**: Adjust `map_event_to_input` to utilize `is_group`, and update unit tests (`test_dispatch_chat_message`).
3.  **Refactor `whatsapp.rs`**: Replace container execution with event dispatching.
4.  **Create `discord.rs`**: Scaffold the Discord client to validate the universal pattern.
5.  **Test Coverage**: Run `cargo tarpaulin` and `cargo test --test integration_tests` to ensure we maintain 100% test coverage and 0 regressions.
