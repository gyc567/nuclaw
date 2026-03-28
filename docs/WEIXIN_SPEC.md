# WeChat Personal Account Integration Specification

## Overview

Implement WeChat personal account (微信个人号) integration for NuClaw using the ilink HTTP gateway, following the reference implementation in [cc-connect](https://github.com/chenhg5/cc-connect/blob/main/docs/weixin.md).

## Architecture

### Module Structure

```
src/
├── weixin.rs              # Main module (KISS: single file for MVP)
                          # Can expand to weixin/ if complexity grows
```

### Core Components

1. **WeixinClient** - Main client struct
2. **WeixinMessage** - Message type definitions
3. **WeixinConfig** - Configuration types
4. **Pure functions** - Message parsing, trigger extraction (testable)

## Configuration

### Environment Variables

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `WEIXIN_TOKEN` | Yes | - | ilink Bearer token |
| `WEIXIN_API_URL` | No | `https://ilinkai.weixin.qq.com` | ilink gateway URL |
| `WEIXIN_CDN_URL` | No | - | CDN base URL for media |
| `WEIXIN_ALLOW_FROM` | No | `*` | Allowed user IDs (comma-separated) |
| `WEIXIN_ACCOUNT_ID` | No | `default` | Multi-account identifier |
| `WEIXIN_POLL_TIMEOUT_MS` | No | 35000 | Long-poll timeout |
| `WEIXIN_PROXY` | No | - | HTTP proxy URL |
| `WEIXIN_WEBHOOK_BIND` | No | `0.0.0.0:8789` | Webhook server bind address |

## API Design

### Message Flow

```
WeChat User → ilink Gateway → WeixinClient.getUpdates() 
                                    ↓
                              handle_message()
                                    ↓
                              EventRouter.dispatch()
                                    ↓
                              Container Agent
                                    ↓
                              WeixinClient.send_message()
                                    ↓
                              ilink Gateway → WeChat User
```

### Implementation Phases

#### Phase 1: MVP (Text Only)
- Long-polling getUpdates
- Text message send/receive
- Basic trigger extraction (@name)
- Message deduplication
- User allowlist

#### Phase 2: Media Support (Future)
- Image download from CDN
- File handling
- Voice transcription

## Integration Points

### Existing Code

1. **Channel Trait** (`src/channels.rs`)
   - Implement `Channel` trait for registry integration
   
2. **Error Handling** (`src/error.rs`)
   - Add `Weixin` variant to `NuClawError`

3. **Router** (`src/router.rs`)
   - Dispatch `AppEvent` with platform="weixin"

4. **Types** (`src/types.rs`)
   - Reuse `NewMessage`, `RegisteredGroup`, `RouterState`

## Test Strategy

### Unit Tests (100% coverage target)
- Message parsing
- Trigger extraction
- Configuration parsing
- Deduplication logic
- Pure utility functions

### Integration Tests
- API client (mocked)
- State persistence

### Test Organization
```rust
#[cfg(test)]
mod tests {
    // Inline tests following existing patterns
}
```

## User Stories

### US1: Receive Text Message
**As a** WeChat user  
**I want to** send a text message to my NuClaw bot  
**So that** I can get AI responses

**Acceptance Criteria:**
- [ ] Long-poll receives message from ilink gateway
- [ ] Message parsed into NewMessage struct
- [ ] Trigger (@Andy) extracted correctly
- [ ] Duplicate messages filtered
- [ ] Response sent back via sendMessage API

### US2: Send Text Response
**As a** NuClaw bot  
**I want to** send text responses to WeChat users  
**So that** they receive AI-generated replies

**Acceptance Criteria:**
- [ ] Text message sent via ilink sendMessage API
- [ ] Empty messages skipped
- [ ] API errors handled gracefully

### US3: User Allowlist
**As a** bot owner  
**I want to** restrict who can use my bot  
**So that** only authorized users can interact

**Acceptance Criteria:**
- [ ] allow_from configuration respected
- [ ] Unauthorized messages silently ignored
- [ ] Wildcard "*" allows all users

## Design Principles

1. **KISS** - Single module, simple structure
2. **High Cohesion** - WeChat-specific logic in weixin.rs
3. **Low Coupling** - Minimal dependencies on other modules
4. **Testability** - Pure functions for business logic
5. **Extensibility** - Easy to add media support later

## Error Handling

Add to `NuClawError`:
```rust
#[error("WeChat error: {message}")]
WeChat { message: String },
```

## CLI Commands (Future)

```bash
# Setup WeChat (QR code login)
nuclaw --weixin-setup

# Start WeChat bot
nuclaw --weixin

# Bind existing token
nuclaw --weixin-bind --token <token>
```

## Implementation Order

1. Add `WeChat` error variant
2. Create `src/weixin.rs` with types and pure functions
3. Implement `WeixinClient` struct
4. Implement `Channel` trait
5. Add CLI flag to main.rs
6. Write tests (TDD)
7. Verify no regression

## Reference Links

- [cc-connect weixin.md](https://github.com/chenhg5/cc-connect/blob/main/docs/weixin.md)
- [OpenClaw weixin plugin](https://github.com/chenhg5/openclaw-weixin)
