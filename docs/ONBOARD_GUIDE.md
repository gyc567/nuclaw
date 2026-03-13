# NuClaw Onboard 使用指南

**NuClaw** 提供交互式配置向导,帮助新用户快速完成初始设置。

---

## 目录

1. [快速开始](#快速开始)
2. [功能介绍](#功能介绍)
3. [详细配置步骤](#详细配置步骤)
4. [环境变量说明](#环境变量说明)
5. [常见问题](#常见问题)

---

## 快速开始

### 方式一: 一键安装 (推荐)

```bash
# 克隆项目并运行安装脚本
git clone https://github.com/gyc567/nuclaw.git
cd nuclaw
chmod +x install.sh
./install.sh
```

安装脚本会自动:
- 检测系统环境
- 安装 Rust 和依赖
- 编译项目
- 引导配置

### 方式二: 手动配置

```bash
# 1. 克隆项目
git clone https://github.com/gyc567/nuclaw.git
cd nuclaw

# 2. 编译
cargo build --release

# 3. 运行配置向导
./target/release/nuclaw --onboard

# 4. 启动服务
./target/release/nuclaw
```

---

## 功能介绍

`--onboard` 命令提供交互式配置,包括:

| 功能 | 说明 |
|------|------|
| LLM Provider | 选择大模型服务商 |
| API Key | 配置大模型访问密钥 |
| Base URL | 设置 API 端点 (可选) |
| Telegram Bot | 配置 Telegram 机器人 |

### 支持的 LLM Provider

| Provider | API Key 环境变量 | 默认 Base URL |
|----------|------------------|---------------|
| Anthropic | `ANTHROPIC_API_KEY` | https://api.anthropic.com |
| OpenAI | `OPENAI_API_KEY` | https://api.openai.com/v1 |
| OpenRouter | `OPENROUTER_API_KEY` | https://openrouter.ai/api/v1 |
| Custom | `CUSTOM_API_KEY` | 用户自定义 |

---

## 详细配置步骤

### 步骤 1: 选择 LLM Provider

```
=== NuClaw Onboard Wizard ===

This wizard will help you configure NuClaw.
Configuration will be saved to: ~/.nuclaw/.env

Available LLM Providers:
------------------------
  1. anthropic - Anthropic Claude API
     API Key: ANTHROPIC_API_KEY, Base URL: ANTHROPIC_BASE_URL
  2. openai - OpenAI GPT API
     API Key: OPENAI_API_KEY, Base URL: OPENAI_BASE_URL
  3. openrouter - OpenRouter - Unified LLM Gateway
     API Key: OPENROUTER_API_KEY, Base URL: OPENROUTER_BASE_URL
  4. custom - Custom OpenAI-compatible endpoint
     API Key: CUSTOM_API_KEY, Base URL: CUSTOM_BASE_URL

Select provider number [1]: 
```

输入数字选择 Provider (默认 1 = Anthropic)

### 步骤 2: 输入 API Key

```
Enter anthropic API Key (ANTHROPIC_API_KEY):
```

输入你的 API Key (输入时不显示,安全)

### 步骤 3: 配置 Base URL (可选)

```
Enter Base URL (optional, default: https://api.anthropic.com):
```

- 直接回车使用默认 URL
- 输入自定义 URL 使用自定义端点

### 步骤 4: 配置 Telegram (可选)

```
Configure Telegram bot? [Y/n]:
```

- 输入 `Y` 配置 Telegram
- 输入 `n` 跳过

如果选择配置,输入 Bot Token:

```
Enter Telegram Bot Token (from @BotFather):
```

### 步骤 5: 完成

```
✓ Configuration saved to /root/.nuclaw/.env

=== Onboard Complete ===

To use NuClaw, either:
 1. Source the config: source /root/.nuclaw/.env
 2. Or set environment variables manually
```

---

## 环境变量说明

配置完成后,`.env` 文件包含以下变量:

### LLM 配置

```bash
# Anthropic (默认)
ANTHROPIC_API_KEY=your-api-key-here
ANTHROPIC_BASE_URL=https://api.anthropic.com
ANTHROPIC_MODEL=claude-sonnet-4-20250514

# 或 OpenAI
OPENAI_API_KEY=your-openai-key
OPENAI_BASE_URL=https://api.openai.com/v1

# 或 OpenRouter
OPENROUTER_API_KEY=your-openrouter-key

# 或自定义端点
CUSTOM_API_KEY=your-custom-key
CUSTOM_BASE_URL=https://your-endpoint.com/v1
```

### Telegram 配置

```bash
TELEGRAM_BOT_TOKEN=your-bot-token
```

---

## 使用配置

### 方式一: 每次运行前加载

```bash
# 加载环境变量
source ~/.nuclaw/.env

# 运行 NuClaw
./nuclaw
```

### 方式二: 添加到 shell 配置

```bash
# 添加到 ~/.bashrc 或 ~/.zshrc
echo 'source ~/.nuclaw/.env' >> ~/.bashrc
```

### 方式三: 系统级配置

```bash
# 复制到系统环境
sudo cp ~/.nuclaw/.env /etc/environment
```

---

## 常见问题

### Q: 如何获取 Anthropic API Key?

1. 访问 https://www.anthropic.com/
2. 注册账户
3. 在 Console > API Keys 创建新密钥

### Q: 如何获取 Telegram Bot Token?

1. 在 Telegram 搜索 @BotFather
2. 发送 `/newbot` 创建新机器人
3. 按照指示获取 Token

### Q: 可以同时配置多个 LLM Provider 吗?

可以,最后配置的 Provider 会生效。

### Q: 如何更新配置?

重新运行:

```bash
./nuclaw --onboard
```

系统会询问是否覆盖现有配置。

### Q: 配置丢失怎么办?

`.env` 文件在 `~/.nuclaw/.env`,可以:
- 重新运行 `--onboard`
- 手动编辑文件

### Q: 如何验证配置是否正确?

```bash
# 加载配置
source ~/.nuclaw/.env

# 检查变量
echo $ANTHROPIC_API_KEY
echo $TELEGRAM_BOT_TOKEN
```

---

## 测试报告

所有配置功能已通过 22 个自动化测试:

```
test_onboard_llm_only_anthropic ... ok
test_onboard_llm_only_openai ... ok
test_onboard_llm_only_openrouter ... ok
test_onboard_telegram_only_test ... ok
test_onboard_full_llm_plus_telegram ... ok
test_onboard_env_file_sourceable ... ok
test_onboard_env_file_multiline_structure ... ok
test_onboard_provider_model_config ... ok
test_onboard_env_file_no_duplicate_keys ... ok
... (共 22 个测试)
```

---

## 下一步

配置完成后,可以:

- [ ] 运行 WhatsApp: `./nuclaw --whatsapp`
- [ ] 运行 Telegram: `./nuclaw --telegram`
- [ ] 配置定时任务: 查看 [任务调度](docs/TASK_SCHEDULER.md)

---

*文档版本: 1.0.0*  
*更新时间: 2026-03-13*
