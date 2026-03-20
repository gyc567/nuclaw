# OpenSpec: Agent Context File System - User Stories (TDD Workflow)

## Epic
Implement a file-driven context loading system for NuClaw Agent, enabling cross-session memory through a three-layer architecture (Identity → Operation → Knowledge). The system must be secure, high-performance, and achieve 100% test coverage.

---

## User Story 1: Path Validation Security
**As a** security engineer,
**I want** to validate all file paths before loading,
**So that** path traversal attacks are prevented and the system is secure.

### Acceptance Criteria
- [ ] PathValidator can validate paths are within allowed directories
- [ ] PathValidator rejects paths containing ".." sequences
- [ ] PathValidator rejects symlinks pointing outside allowed directories
- [ ] PathValidator rejects paths outside allowed roots
- [ ] PathValidator returns canonicalized path on success
- [ ] Unit tests cover all security scenarios

---

## User Story 2: Content Sanitization
**As a** security engineer,
**I want** to sanitize all loaded file content,
**So that** prompt injection attacks are prevented.

### Acceptance Criteria
- [ ] ContentSanitizer filters "ignore previous instructions" patterns
- [ ] ContentSanitizer filters "system prompt" patterns
- [ ] ContentSanitizer filters "# Instructions" delimiters
- [ ] ContentSanitizer truncates content exceeding max length (100KB)
- [ ] ContentSanitizer handles empty and Unicode content correctly
- [ ] Unit tests verify all dangerous patterns are filtered

---

## User Story 3: Context Cache with LRU + TTL
**As a** performance engineer,
**I want** a caching layer for context files,
**So that** repeated loads are fast (<10ms) and cold starts are <100ms.

### Acceptance Criteria
- [ ] ContextCache supports get_or_load pattern
- [ ] ContextCache evicts entries using LRU when max size reached
- [ ] ContextCache expires entries after TTL (60s)
- [ ] ContextCache detects stale entries via file mtime
- [ ] ContextCache supports manual invalidation
- [ ] Unit tests verify LRU eviction and TTL expiration

---

## User Story 4: Load Identity (SOUL.md)
**As a** agent developer,
**I want** to load SOUL.md to define agent personality,
**So that** the agent knows who it is.

### Acceptance Criteria
- [ ] ContextLoader can load SOUL.md from group context directory
- [ ] Parsed Identity contains name, role, vibe, emoji fields
- [ ] Identity supports YAML frontmatter parsing
- [ ] Graceful degradation when SOUL.md is missing (use defaults)
- [ ] Unit tests verify parsing and default values

---

## User Story 5: Load User Profile (USER.md)
**As a** agent developer,
**I want** to load USER.md to understand the user,
**So that** the agent knows its boss preferences.

### Acceptance Criteria
- [ ] ContextLoader can load USER.md from group context directory
- [ ] Parsed User contains name, timezone, language, preferences
- [ ] User profile supports YAML frontmatter parsing
- [ ] Graceful degradation when USER.md is missing (use defaults)
- [ ] Unit tests verify parsing and default values

---

## User Story 6: Load Agent Rules (AGENTS.md)
**As a** agent developer,
**I want** to load AGENTS.md for working rules,
**So that** the agent knows how to behave.

### Acceptance Criteria
- [ ] ContextLoader can load AGENTS.md from group context directory
- [ ] Parsed AgentRules contains startup_sequence, memory_rules, safety_boundaries
- [ ] AgentRules supports YAML frontmatter parsing
- [ ] Graceful degradation when AGENTS.md is missing
- [ ] Unit tests verify parsing and default values

---

## User Story 7: Load Memory (MEMORY.md)
**As a** agent developer,
**I want** to load MEMORY.md for long-term memory,
**So that** the agent remembers past interactions.

### Acceptance Criteria
- [ ] ContextLoader can load MEMORY.md from group context directory
- [ ] Parsed Memory contains last_updated, preferences, lessons_learned
- [ ] Memory supports YAML frontmatter parsing
- [ ] Graceful degradation when MEMORY.md is missing
- [ ] Unit tests verify parsing and default values

---

## User Story 8: Build System Prompt
**As a** agent developer,
**I want** to build a complete system prompt from loaded context,
**So that** the agent has all necessary information at runtime.

### Acceptance Criteria
- [ ] PromptBuilder combines Identity, User, Rules, Memory into single prompt
- [ ] PromptBuilder handles empty sections gracefully
- [ ] Generated prompt follows defined template structure
- [ ] Unit tests verify prompt structure and content

---

## User Story 9: Full Context Loading Flow
**As a** system integrator,
**I want** to load all context files in one call,
**So that** the agent startup is simplified.

### Acceptance Criteria
- [ ] ContextLoader.load_context() loads all required files
- [ ] Files are loaded in parallel where possible
- [ ] Partial failures don't crash the system (graceful degradation)
- [ ] Security validation runs before any file content is used
- [ ] Integration tests verify full flow

---

## User Story 10: Memory Bridge (TieredMemory ↔ File)
**As a** memory engineer,
**I want** to sync between TieredMemory and file system,
**So that** memories persist across restarts.

### Acceptance Criteria
- [ ] MemoryBridge can write important entries to MEMORY.md
- [ ] MemoryBridge can load MEMORY.md content into TieredMemory
- [ ] MemoryBridge handles sync conflicts gracefully
- [ ] Integration tests verify bidirectional sync

---

## User Story 11: Agent Coordination
**As a** system architect,
**I want** to coordinate multiple agents with dependencies,
**So that** agents run in correct order.

### Acceptance Criteria
- [ ] AgentCoordinator can register agent dependencies
- [ ] AgentCoordinator executes agents in topological order
- [ ] Single-writer rule prevents write conflicts
- [ ] Unit tests verify dependency resolution

---

## User Story 12: Access Frequency Tracking
**As a** performance engineer,
**I want** to track which groups are accessed most,
**So that** I can preload frequently accessed contexts.

### Acceptance Tools
- [ ] AccessTracker records each group access
- [ ] AccessTracker returns top N most accessed groups
- [ ] AccessTracker supports frequency-based preloading
- [ ] Unit tests verify frequency calculation

---

## User Story 13: Integration with AgentRunner
**As a** system integrator,
**I want** to integrate context loading with AgentRunner,
**So that** agents automatically load context on startup.

### Acceptance Criteria
- [ ] AgentRunner loads context before execution
- [ ] Context is injected into system prompt
- [ ] Existing tests still pass
- [ ] No regression in existing functionality

---

## Implementation Priority

### Phase 1: Security & Performance Foundation (Stories 1-3)
- PathValidator
- ContentSanitizer
- ContextCache

### Phase 2: Core Context Loading (Stories 4-9)
- ContextLoader
- PromptBuilder
- Full integration

### Phase 3: Memory & Coordination (Stories 10-13)
- MemoryBridge
- AgentCoordinator
- AccessTracker
- AgentRunner Integration

---

## Test Coverage Requirements

| Module | Target Coverage |
|--------|-----------------|
| PathValidator | 100% |
| ContentSanitizer | 100% |
| ContextCache | 100% |
| ContextLoader | 100% |
| PromptBuilder | 100% |
| MemoryBridge | 100% |
| AgentCoordinator | 100% |
| AccessTracker | 100% |

---

**Document Version**: 1.0
**Created**: 2026-03-19
**Status**: Ready for Implementation
