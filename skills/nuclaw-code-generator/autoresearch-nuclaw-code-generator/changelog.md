# Changelog

## Experiment 2 — KEEP ✅ (Manual Verification)

**Score:** Skill now includes Performance + Security patterns

**Change:** Added two new sections to SKILL.md:

### Performance Patterns
- Avoid unnecessary clones (use references when possible)
- Preallocate collections with `Vec::with_capacity()`
- Async performance (Arc for shared state, JoinHandle for batching)
- `#[inline]` for small/hot functions

### Security Patterns
- **NEVER expose secrets in error messages** (generic messages only)
- **Always validate input** (email format, password strength, length limits)
- **SQL injection prevention** (parameterized queries with `?1`)
- **Rate limiting pattern** (AtomicU64 counter with window)
- **Secure random generation** (rand::Rng)

**Reasoning:** Generated code should be performant AND secure out of the box.

**Result:** Skill now guides generation of:
- Fast code (no unnecessary clones, proper async patterns)
- Safe code (no secret leakage, input validation, SQL injection safe)

---

## Experiment 1 — KEEP ✅

**Score:** 25/25 (100.0%)

**Change:** Added Anti-Patterns section with "NEVER duplicate entire files" rule

**Reasoning:** Baseline showed 23/25 (92%) with minor import issues.

**Result:** 
- All 5 scenarios now pass with 100% score
- No file duplication - only adding new code with `use crate::xxx` imports

**Improvement:** +8% (from 92% to 100%)

---

## Experiment 0 — baseline

**Score:** 23/25 (92.0%)

**Change:** Original skill — no changes

**Result:** 
- Compilation: 5/5 passed
- Error Handling: 5/5 passed
- Documentation: 5/5 passed
- Pattern Compliance: 5/5 passed
- Import Organization: 4/5 passed (minor issues)
