# OpenSpec: Architecture Phase 3 - User Stories (TDD Workflow)

## Epic
Refactor `WhatsApp` to use the unified `AppEvent` routing mechanism established in Phase 2, and create a scalable `Discord` client skeleton using the same pattern. Implement Dependency Injection (DI) for the `EventRouter` to improve performance and testability.

## User Story 1: Enhance AppEvent and Router Intelligence
**As an** architect,
**I want** to add a semantic `is_group` flag to `AppEvent::ChatMessage` and use it in `EventRouter`,
**So that** the router can autonomously decide if a container execution is the 'main' process (e.g., DMs are main, groups are not) without leaking container concepts to the chat clients.

### Acceptance Criteria
- [ ] `AppEvent::ChatMessage` includes a boolean `is_group` field.
- [ ] `EventRouter::map_event_to_input` sets `ContainerInput.is_main` to `!is_group`.
- [ ] Unit tests for `EventRouter` verify that `is_group = true` results in `is_main = false`, and vice versa.

---

## User Story 2: Dependency Injection for WhatsApp Client
**As a** system initializer,
**I want** to pass an `Arc<EventRouter>` into the `WhatsAppClient` constructor,
**So that** the client doesn't need to instantiate a new router for every incoming message, improving performance and enabling isolated mock testing.

### Acceptance Criteria
- [ ] `WhatsAppClient` struct holds an `Arc<EventRouter>`.
- [ ] `WhatsAppClient::new` accepts `router: Arc<EventRouter>`.
- [ ] Unit/Integration tests compile and pass with the new constructor signature.

---

## User Story 3: Event-Driven WhatsApp Message Handling
**As a** WhatsApp client,
**I want** to convert incoming WhatsApp messages into `AppEvent::ChatMessage` and dispatch them via the injected `EventRouter`,
**So that** I am completely decoupled from `ContainerInput` and `run_container` logic.

### Acceptance Criteria
- [ ] `src/whatsapp.rs` no longer imports or uses `ContainerInput` or `run_container`.
- [ ] `handle_message` constructs an `AppEvent::ChatMessage` and calls `self.router.dispatch(event)`.
- [ ] The `is_group` flag is accurately calculated based on the WhatsApp JID (`!msg.chat_jid.ends_with("@s.whatsapp.net")`).

---

## User Story 4: Discord Client Skeleton
**As a** future feature developer,
**I want** a foundational `DiscordClient` skeleton that accepts an `EventRouter`,
**So that** I have a clear, pre-architected pattern to follow when implementing the actual Discord API.

### Acceptance Criteria
- [ ] `src/discord.rs` exists and is exported in `src/lib.rs`.
- [ ] Contains a `DiscordClient` struct with an `Arc<EventRouter>`.
- [ ] Contains a mock or skeleton `handle_message` function demonstrating `AppEvent` dispatch.
- [ ] Unit tests verify the `DiscordClient` can be instantiated and dispatch a test event to a `MockRuntime`.
