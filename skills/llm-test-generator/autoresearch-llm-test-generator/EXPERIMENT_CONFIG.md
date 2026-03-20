# Autoresearch Configuration - llm-test-generator

## Target Skill
`/Users/eric/dreame/code/nuclaw/skills/llm-test-generator/SKILL.md`

## Test Inputs (5 Scenarios)

1. **Provider Basic Chat** - Generate tests for a simple chat() call
2. **System Prompt Chat** - Generate tests for chat_with_system()
3. **Security Tests** - Generate prompt injection and security tests
4. **Edge Cases** - Generate empty/max/unicode input tests
5. **Error Handling** - Generate rate limit/timeout/auth tests

## Eval Criteria (6 Binary Evals)

```
EVAL 1: Coverage Completeness
Question: Does the generated test suite cover all 4 test categories (API, Security, Edge Cases, Error Handling)?
Pass: Tests exist for at least 3 of 4 categories
Fail: Tests missing for 2+ categories

EVAL 2: mockall Correctness
Question: Does the generated code use mockall correctly with async_trait?
Pass: Uses mock! macro with async fn and correct parameter matching
Fail: Missing mockall setup, wrong macro syntax, or async issues

EVAL 3: Test Quality
Question: Are test functions named descriptively and follow Arrange/Act/Assert pattern?
Pass: Names like test_chat_success, test_prompt_injection_blocked
Fail: Vague names like test1, test_case, or no clear structure

EVAL 4: Security Tests
Question: Are prompt injection and data security tests included?
Pass: At least 2 security tests present
Fail: No security tests or only superficial checks

EVAL 5: Error Coverage
Question: Are all major error types tested (RateLimit, Timeout, Auth, Api, Validation)?
Pass: At least 4 different error types tested
Fail: Only 1-2 error types, or no error tests

EVAL 6: Compilation Success
Question: Would the generated test code compile successfully?
Pass: Uses correct Rust syntax, proper imports, valid mockall predicates
Fail: Syntax errors, wrong imports, invalid predicates
```

## Experiment Settings

| Parameter | Value |
|-----------|-------|
| Runs per experiment | 5 |
| Max score | 30 (6 evals × 5 runs) |
| Target pass rate | 90%+ (27/30) |
| Experiment cap | 5 |

## Test Prompt Template

Generate tests for a NuClaw LLM Provider with the following requirements:

```
[SCENARIO DESCRIPTION FROM LIST ABOVE]

Target trait signature:
async fn chat(&self, message: &str, model: &str, temperature: f64) -> Result<String>

Error type: NuClawError with variants Api, Auth, Validation, Timeout, Security, RateLimit
```

## Dashboard

Open `dashboard.html` in browser to see live results.
