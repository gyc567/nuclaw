# Per-Session Workspace Integration Specification

## Background

NuClaw Phase 3 implemented Per-Session Workspace Isolation in `src/workspace.rs`, but it's NOT integrated with the Agent Runner. Currently all agent components share group-level workspace.

## Goal

Integrate Per-Session Workspace into Agent Runner to achieve true session isolation.

## Current State

- `src/workspace.rs`: ✅ Implemented (Workspace struct, create, activate, deactivate, metadata)
- `src/agent_runner.rs`: ❌ Not using workspace.rs
- Session isolation: ❌ Currently uses group_folder only

## User Stories

### User Story 1: Agent Uses Session Workspace

**As a** system architect  
**I want** each agent execution to use its own session workspace  
**So that** different sessions are completely isolated from each other

**Acceptance Criteria:**
- [ ] When a new session starts, a new workspace is created
- [ ] Workspace is bound to session_id
- [ ] Agent executes within its session workspace
- [ ] Session workspace is cleaned up when session ends

### User Story 2: Workspace Lifecycle Management

**As a** system  
**I want** workspaces to be managed throughout their lifecycle  
**So that** resources are properly allocated and cleaned up

**Acceptance Criteria:**
- [ ] Workspace is created when session starts
- [ ] Workspace is activated for the session
- [ ] Workspace is deactivated when session ends
- [ ] Workspace is cleaned up after retention period

### User Story 3: Fallback to Group Workspace

**As a** system  
**I want** to fall back to group workspace when session workspace is unavailable  
**So that** system remains functional

**Acceptance Criteria:**
- [ ] When workspace creation fails, fallback to group workspace
- [ ] Clear logging when fallback occurs
- [ ] No disruption to user experience

## Technical Design

### Integration Points

1. **ContainerInput Enhancement**
   ```rust
   pub struct ContainerInput {
       // ... existing fields ...
       pub session_workspace_id: Option<String>,  // NEW: session workspace ID
   }
   ```

2. **AgentRunner Enhancement**
   ```rust
   pub struct ContainerRunnerAdapter {
       // ... existing fields ...
       workspace_manager: Option<WorkspaceManager>,  // NEW
   }
   ```

3. **WorkspaceManager** (new component)
   - Creates/activates/deactivates workspaces
   - Handles lifecycle
   - Provides fallback

### Workspace Resolution Order

1. Session workspace (if session_id provided)
2. Group workspace (fallback)
3. Default workspace (last resort)

## Implementation Plan (TDD)

### Phase 1: Workspace Manager
1. Create `workspace_manager.rs`
2. Write tests for workspace creation/activation
3. Implement workspace lifecycle

### Phase 2: Agent Runner Integration
1. Modify `ContainerInput` to include session_workspace_id
2. Update `agent_runner.rs` to use workspace
3. Add fallback logic

### Phase 3: Testing & Documentation
1. Integration tests
2. End-to-end tests
3. Update documentation

## Files to Modify

- `src/agent_runner.rs` - Integrate workspace
- `src/container_runner.rs` - Accept workspace path
- `src/types.rs` - Add session_workspace_id to ContainerInput
- `src/workspace.rs` - Add WorkspaceManager (if needed)

## Test Coverage Requirements

- All new code: 100% coverage
- Preserve existing tests
- Add integration tests
