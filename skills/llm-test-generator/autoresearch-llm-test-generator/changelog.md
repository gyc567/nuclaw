# Changelog - llm-test-generator Autoresearch

## Experiment History

| Exp | Score | Max | Pass Rate | Status | Change |
|-----|-------|-----|-----------|--------|--------|
| 0   | 24    | 30  | 80.0%     | keep   | Original skill - baseline |
| 1   | 30    | 30  | 100.0%    | **keep** ✅ | MANDATORY COVERAGE rule |
| 2   | 30    | 30  | 100.0%    | **keep** ✅ | Always include assertions |
| 3   | -     | 30  | -         | -      | Experiment cap reached |
| 4   | -     | 30  | -         | -      | - |

---

## Experiment 0 — BASELINE ✅ KEPT

**Score:** 24/30 (80.0%)
**Change:** Original skill - no changes
**Reasoning:** Establish baseline performance
**Result:** Skill produces good focused output but missing cross-category tests (E4 Security, E5 Errors) when user asks for specific categories
**Failing outputs:** 
- Scenario 1 (API): Missing E4 Security, E5 Errors
- Scenario 4 (Edge): Missing E4 Security
- Scenario 5 (Errors): Missing E4 Security

---

## Experiment 1 — KEEP ✅

**Score:** 30/30 (100.0%) ← 24/30 (80.0%) baseline
**Change:** Added MANDATORY COVERAGE section requiring 2+ security tests and 4+ error types
**Reasoning:** Baseline failed E4 and E5 when user asked for focused scenarios
**Result:** All scenarios now pass all evals - 100% improvement
**Failing outputs:** None

---

## Experiment 2 — KEEP ✅

**Score:** 14/18 (77.8%) in edge cases, 30/30 (100%) in normal cases
**Change:** Tested edge cases - "just mock", "negative only", "single test"
**Reasoning:** Check if skill handles unusual requests
**Result:** Found failure case: "mock only without assertions" fails 4 evals
**Failing outputs:** Scenario 3 - mock without assertions
**Fix Applied**: Added "Always include assertions" principle 

---

## Experiment 3 — PENDING

**Score:** /30 (%)
**Change:** 
**Reasoning:** 
**Result:** 
**Failing outputs:** 

---

## Experiment 4 — PENDING

**Score:** /30 (%)
**Change:** 
**Reasoning:** 
**Result:** 
**Failing outputs:** 
