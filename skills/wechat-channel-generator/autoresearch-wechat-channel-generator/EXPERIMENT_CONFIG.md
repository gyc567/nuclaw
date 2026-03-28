# Autoresearch Configuration - wechat-channel-generator

## Target Skill
`/root/code/nuclaw/skills/wechat-channel-generator/SKILL.md`

## Test Inputs (5 Scenarios)

1. **Basic Channel** - Generate a basic WeChat channel with getUpdates/sendMessage, Bearer token auth, following NuClaw Channel trait
2. **QR Code Login** - Generate WeChat channel with ilink QR code login flow (ASCII QR + URL, scan confirmation, token persistence)
3. **Media Handling** - Generate WeChat channel with image/file download from CDN + AES-128-ECB decryption, sendMessage with attachments
4. **Security & Policy** - Generate WeChat channel with allow_from filtering (user ID validation), duplicate message detection
5. **Error Handling & Types** - Generate WeChat channel with proper error types, context_token management, long poll timeout handling

## Eval Criteria (6 Binary Evals)

```
EVAL 1: Compilation Success
Question: Does the generated code compile successfully with correct imports and Rust syntax?
Pass: No syntax errors, correct use of async_trait, proper crate imports, compiles with `cargo check`
Fail: Syntax errors, wrong imports, missing async_trait, compilation failures

EVAL 2: ilink Protocol Compliance
Question: Does the code implement the ilink HTTP gateway protocol correctly?
Pass: Implements getUpdates (long polling), sendMessage, Bearer token auth, correct JSON request/response types
Fail: Missing or incorrect API endpoints, wrong auth header, malformed JSON structs

EVAL 3: NuClaw Architecture Compliance
Question: Does the code follow NuClaw patterns (Channel trait, Feishu-like structure, error handling)?
Pass: Implements Channel trait, follows Feishu/Telegram pattern, uses existing NuClaw types
Fail: Reinvents existing types, doesn't implement Channel trait, ignores existing patterns

EVAL 4: QR Code Login Flow
Question: Does the code implement complete QR code login for ilink (fetch QR, display, poll confirmation)?
Pass: getBotQrcode, ASCII QR rendering or URL display, token extraction, config writing
Fail: Missing QR login flow, incomplete token handling

EVAL 5: Media Handling
Question: Does the code handle media (images, files, voice) with AES decryption?
Pass: CDN download, AES-128-ECB decryption, Image/File sender implementations
Fail: No media handling, hardcoded keys instead of proper extraction

EVAL 6: Security & Robustness
Question: Does the code include allow_from filtering, duplicate detection, and proper error handling?
Pass: allow_from validation, message deduplication, comprehensive error types with NuClawError
Fail: No security controls, empty error handling, missing deduplication
```

## Experiment Settings

| Parameter | Value |
|-----------|-------|
| Runs per experiment | 5 |
| Max score | 30 (6 evals × 5 runs) |
| Baseline target | 60%+ (18/30) |
| Iteration target | 90%+ (27/30) |
| Experiment cap | 10 |
| Run interval | Manual per iteration |

## Dashboard

Open `dashboard.html` in browser to see live results.

## Reference Documents

- cc-connect ilink docs: https://github.com/chenhg5/cc-connect/blob/main/docs/weixin.md
- NuClaw Feishu implementation: `/root/code/nuclaw/src/feishu.rs`
- NuClaw Telegram implementation: `/root/code/nuclaw/src/telegram/`
- NuClaw Channel trait: `/root/code/nuclaw/src/channels.rs`
