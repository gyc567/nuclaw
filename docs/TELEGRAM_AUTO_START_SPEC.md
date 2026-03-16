# Telegram Auto-Start Feature Specification

## 背景

当前 NuClaw 的 Telegram Bot 需要显式使用 `--telegram` 参数才能启动。用户希望像 OpenClaw 一样，只要配置了 `TELEGRAM_BOT_TOKEN` 环境变量，就默认启用 Telegram Bot。

## 目标

修改 NuClaw，使其在以下情况下自动启动 Telegram Bot：
1. 配置了 `TELEGRAM_BOT_TOKEN` 环境变量
2. 运行默认命令（不带 `--telegram` 参数）

## 设计原则

- **KISS**: 保持简单，不破坏现有功能
- **高内聚，低耦合**: Telegram 启动逻辑应该独立封装
- **向后兼容**: `--telegram` 参数仍然有效
- **100% 测试覆盖**: 所有新代码需要测试

## 用户故事

### User Story 1: 环境变量检测自动启动 Telegram

**作为** 系统管理员  
**我希望** 当配置了 TELEGRAM_BOT_TOKEN 环境变量时，系统自动启动 Telegram Bot  
**以便** 不需要手动传递 --telegram 参数

**验收标准:**
1. [ ] 当 TELEGRAM_BOT_TOKEN 未设置时，运行 nuclaw 不启动 Telegram（保持现状）
2. [ ] 当 TELEGRAM_BOT_TOKEN 已设置时，运行 nuclaw 自动启动 Telegram Bot
3. [ ] 显式传递 --telegram 参数时，无论环境变量如何都启动 Telegram
4. [ ] 显式传递 --no-telegram 参数时，即使配置了环境变量也不启动 Telegram

### User Story 2: 优雅的启动流程

**作为** 用户  
**我希望** Telegram Bot 的启动/停止不影响其他服务  
**以便** 某个服务失败不会导致整个系统崩溃

**验收标准:**
1. [ ] Telegram Bot 启动失败不影响 scheduler 运行
2. [ ] 日志清晰显示 Telegram Bot 的启动状态
3. [ ] 可以通过 nuclaw --status 查看 Telegram 状态

## 技术方案

### 方案 A: 在 run_main_application 中检测并启动（推荐）

```rust
// 修改 run_main_application
async fn run_main_application(db: db::Database) -> Result<()> {
    // ... existing code ...

    // Auto-start Telegram if token is configured
    let telegram_handle = spawn_telegram_if_configured(db.clone()).await;

    // ... rest of code ...
}
```

**优点:**
- 改动最小，不影响现有参数解析
- 逻辑清晰，易于测试
- 向后兼容

### 方案 B: 修改 Args 结构体添加 --no-telegram

**优点:**
- 提供显式关闭选项

**缺点:**
- 需要修改 structopt，复杂度增加

**选择方案 A**，因为足够简单且满足需求。

## 实现步骤

1. **创建 telegram/mod.rs 添加自动启动函数**
2. **修改 run_main_application 调用自动启动**
3. **添加测试用例**
4. **更新文档**

## 测试计划

### 单元测试

1. `test_telegram_auto_start_enabled` - 测试 TOKEN 设置时返回 true
2. `test_telegram_auto_start_disabled` - 测试 TOKEN 未设置时返回 false
3. `test_should_auto_start_telegram` - 测试主函数根据配置决定是否启动

### 集成测试

1. 完整启动流程测试
2. Telegram 启动失败不影响其他服务测试
