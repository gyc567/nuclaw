# NuClaw 使用手册

## 目录

1. [概述](#1-概述)
2. [快速开始](#2-快速开始)
3. [环境要求](#3-环境要求)
4. [安装部署](#4-安装部署)
5. [基础配置](#5-基础配置)
6. [Telegram 配置](#6-telegram-配置)
7. [WhatsApp 配置](#7-whatsapp-配置)
8. [自定义大模型配置](#8-自定义大模型配置)
9. [定时任务配置](#9-定时任务配置)
10. [挂载白名单配置](#10-挂载白名单配置)
11. [运行模式](#11-运行模式)
12. [目录结构](#12-目录结构)
13. [常见问题](#13-常见问题)

---

## 1. 概述

NuClaw 是一个基于 Rust 实现的个人 AI 助手，它可以通过 Telegram 或 WhatsApp 与用户交互，并在大模型容器中执行任务。

### 核心特性

- **多平台支持**：支持 Telegram 和 WhatsApp
- **容器隔离**：每个群组有独立的文件系统和运行环境
- **定时任务**：支持 Cron 表达式定时执行任务
- **自定义模型**：支持配置自定义大模型 API

### 工作流程

```
用户发送消息 → Telegram/WhatsApp → NuClaw → 容器(Claude) → 返回结果
```

---

## 2. 快速开始

### 2.1 一键部署

```bash
curl -sSL https://raw.githubusercontent.com/gyc567/nuclaw/main/deploy.sh | bash
```

### 2.2 手动部署

```bash
# 克隆项目
git clone https://github.com/gyc567/nuclaw.git
cd nuclaw

# 创建必要目录
mkdir -p store data groups logs

# 编译
cargo build --release

# 运行
./target/release/nuclaw
```

---

## 3. 环境要求

| 依赖 | 版本要求 | 说明 |
|------|----------|------|
| Rust | 1.70+ | 编译环境 |
| Docker | 最新版 | 容器运行时（Linux） |
| SQLite | 3.x | 数据库 |
| Claude Code | 最新版 | 容器内 AI 代理 |

---

## 4. 安装部署

### 4.1 克隆项目

```bash
git clone https://github.com/gyc567/nuclaw.git
cd nuclaw
```

### 4.2 编译项目

```bash
# Debug 模式
cargo build

# Release 模式（推荐）
cargo build --release
```

### 4.3 创建必要目录

```bash
mkdir -p store data groups logs
```

### 4.4 验证安装

```bash
./target/release/nuclaw --help
```

输出：
```
nuclaw 1.0.0

USAGE:
    nuclaw [FLAGS]

FLAGS:
        --auth         
    -h, --help         Prints help information
        --scheduler    
        --telegram     
    -V, --version      Prints version information
        --whatsapp     
```

---

## 5. 基础配置

### 5.1 环境变量

在运行 NuClaw 前，需要设置相关环境变量。可以在终端中执行：

```bash
# 临时设置（当前终端有效）
export VARIABLE_NAME=value

# 永久设置（添加到 ~/.bashrc 或 ~/.zshrc）
echo 'export VARIABLE_NAME=value' >> ~/.bashrc
source ~/.bashrc
```

### 5.2 核心配置变量

| 变量名 | 默认值 | 必填 | 说明 |
|--------|--------|------|------|
| `ASSISTANT_NAME` | Andy | 否 | 触发词，消息中包含此词才会触发 AI |
| `CONTAINER_TIMEOUT` | 300000 | 否 | 容器执行超时时间（毫秒） |
| `TZ` | UTC | 否 | 时区，用于定时任务 |
| `CONTAINER_IMAGE` | anthropic/claude-code:latest | 否 | Docker 镜像 |
| `LOG_LEVEL` | info | 否 | 日志级别：debug/info/warn/error |

### 5.3 基础运行命令

```bash
# 基本运行（所有功能）
./target/release/nuclaw

# 仅运行调度器
./target/release/nuclaw --scheduler

# 带调试日志运行
LOG_LEVEL=debug ./target/release/nuclaw
```

---

## 6. Telegram 配置

### 6.1 创建 Telegram Bot

1. 打开 Telegram，搜索 **@BotFather**
2. 发送 `/newbot` 命令
3. 按照提示输入 Bot 名称和用户名
4. 获取 Bot Token（格式：`123456:ABC-DEF1234ghIkl-zyx57W2v1u123ew11`）

### 6.2 配置环境变量

```bash
# 必填：Bot Token
export TELEGRAM_BOT_TOKEN=你的BotToken

# 可选：Webhook URL（需要公网域名）
export TELEGRAM_WEBHOOK_URL=https://your-domain.com

# 可选：Webhook 路径（默认：telegram-webhook）
export TELEGRAM_WEBHOOK_PATH=my-bot

# 可选：DM 策略
export TELEGRAM_DM_POLICY=pairing  # pairing/allowlist/open/disabled

# 可选：群组策略
export TELEGRAM_GROUP_POLICY=allowlist  # open/allowlist/disabled
```

### 6.3 DM 策略说明

| 策略 | 说明 |
|------|------|
| `pairing` | 用户需要配对码才能使用（默认） |
| `allowlist` | 只有白名单用户可以使用 |
| `open` | 所有人都可以使用 |
| `disabled` | 禁用 DM |

### 6.4 群组策略说明

| 策略 | 说明 |
|------|------|
| `open` | 所有群组都可以使用 |
| `allowlist` | 只有白名单群组可以使用（默认） |
| `disabled` | 禁用群组功能 |

### 6.5 启动 Telegram 模式

```bash
./target/release/nuclaw --telegram
```

---

## 7. WhatsApp 配置

### 7.1 配置 WhatsApp MCP

NuClaw 通过 MCP（Model Context Protocol）连接 WhatsApp。

```bash
# 必填：WhatsApp MCP 服务地址
export WHATSAPP_MCP_URL=http://localhost:3000
```

### 7.2 启动 WhatsApp 认证

```bash
./target/release/nuclaw --auth
```

这会显示一个 QR 码，用 WhatsApp 扫描认证。

### 7.3 启动 WhatsApp 模式

```bash
./target/release/nuclaw --whatsapp
```

---

## 8. 自定义大模型配置

### 8.1 支持的模型

NuClaw 支持连接任意兼容 Anthropic API 的大模型服务。

### 8.2 配置变量

```bash
# 必填：API Key
export ANTHROPIC_API_KEY=your-api-key

# 可选：自定义 API 端点
export ANTHROPIC_BASE_URL=https://api.anthropic.com

# 可选：自定义模型名称
export CLAUDE_MODEL=claude-3-5-sonnet-20241022
```

### 8.3 使用 MiniMax 模型示例

```bash
# MiniMax 配置
export ANTHROPIC_BASE_URL=https://api.minimaxi.com/anthropic
export ANTHROPIC_API_KEY=sk-cp-xxxxxxxx
export CLAUDE_MODEL=MiniMax-M2.5
```

### 8.4 使用 OpenAI 兼容接口示例

```bash
# OpenAI 兼容接口
export ANTHROPIC_BASE_URL=https://api.openai.com/v1
export ANTHROPIC_API_KEY=sk-xxxxxxxx
export CLAUDE_MODEL=gpt-4o
```

---

## 9.

### 9.1 创建 定时任务配置定时任务

通过数据库直接插入任务，或通过 API（如果有）：

```sql
INSERT INTO tasks (
    id,
    group_folder,
    chat_jid,
    prompt,
    schedule_type,
    schedule_value,
    context_mode,
    status,
    created_at
) VALUES (
    'task_001',
    'default',
    'telegram:user:123456',
    '今天的天气怎么样？',
    'cron',
    '0 8 * * *',
    'append',
    'active',
    datetime('now')
);
```

### 9.2 Cron 表达式格式

```
┌───────────── 分钟 (0 - 59)
│ ┌───────────── 小时 (0 - 23)
│ │ ┌───────────── 日期 (1 - 31)
│ │ │ ┌───────────── 月份 (1 - 12)
│ │ │ │ ┌───────────── 星期 (0 - 6)
│ │ │ │ │
* * * * *
```

### 9.3 示例

| Cron 表达式 | 说明 |
|-------------|------|
| `0 8 * * *` | 每天早上 8 点 |
| `0 */2 * * *` | 每隔 2 小时 |
| `0 9 * * 1-5` | 工作日早上 9 点 |
| `*/15 * * * *` | 每隔 15 分钟 |

### 9.4 启动调度器

```bash
./target/release/nuclaw --scheduler
```

---

## 10. 挂载白名单配置

### 10.1 配置文件位置

`~/.config/nuclaw/mount-allowlist.json`

### 10.2 配置格式

```json
{
  "allowedRoots": [
    {
      "path": "~/projects",
      "allowReadWrite": true,
      "description": "开发项目目录"
    },
    {
      "path": "/mnt/data",
      "allowReadWrite": false,
      "description": "只读数据目录"
    }
  ],
  "blockedPatterns": ["password", "secret", "*.key"],
  "nonMainReadOnly": true
}
```

### 10.3 配置说明

| 字段 | 类型 | 说明 |
|------|------|------|
| `allowedRoots` | 数组 | 允许挂载的目录列表 |
| `path` | 字符串 | 目录路径 |
| `allowReadWrite` | 布尔 | 是否允许读写 |
| `description` | 字符串 | 目录描述 |
| `blockedPatterns` | 数组 | 禁止访问的文件模式 |
| `nonMainReadOnly` | 布尔 | 非主目录是否只读 |

---

## 11. 运行模式

### 11.1 常用运行命令

```bash
# 1. 基本模式（同时启动 Telegram 和 WhatsApp）
./target/release/nuclaw

# 2. Telegram 模式
./target/release/nuclaw --telegram

# 3. WhatsApp 模式
./target/release/nuclaw --whatsapp

# 4. 调度器模式（仅运行定时任务）
./target/release/nuclaw --scheduler

# 5. 认证模式（用于 WhatsApp 认证）
./target/release/nuclaw --auth
```

### 11.2 后台运行

```bash
# 使用 nohup 后台运行
nohup ./target/release/nuclaw > nuclaw.log 2>&1 &

# 使用 systemd 服务（推荐）
sudo cp nuclaw.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable nuclaw
sudo systemctl start nuclaw
```

### 11.3 Docker 模式运行

如果需要指定 Docker 镜像：

```bash
export CONTAINER_IMAGE=anthropic/claude-code:latest
./target/release/nuclaw
```

---

## 12. 目录结构

```
nuclaw/
├── src/                    # 源代码
│   ├── main.rs            # 入口文件
│   ├── config.rs          # 配置管理
│   ├── db.rs              # 数据库操作
│   ├── whatsapp.rs       # WhatsApp 连接
│   ├── telegram.rs        # Telegram 连接
│   ├── container_runner.rs # 容器管理
│   └── task_scheduler.rs # 任务调度
├── store/                 # 数据库存储
│   └── nuclaw.db
├── data/                  # 应用数据
│   └── ipc/              # 进程间通信
├── groups/                # 群组上下文
│   └── {group}/          # 每个群组的独立目录
├── logs/                  # 日志文件
└── target/                # 编译输出
    └── release/
        └── nuclaw         # 可执行文件
```

---

## 13. 常见问题

### Q1: 如何查看运行日志？

```bash
# 实时查看日志
tail -f nuclaw.log

# 查看最新 100 行
tail -n 100 nuclaw.log
```

### Q2: 如何更改触发词？

```bash
export ASSISTANT_NAME=Andy
```

### Q3: 容器启动失败怎么办？

1. 检查 Docker 是否运行：`docker ps`
2. 检查镜像是否拉取：`docker images`
3. 查看详细日志：`LOG_LEVEL=debug ./target/release/nuclaw`

### Q4: 如何完全重启 NuClaw？

```bash
# 查找进程
ps aux | grep nuclaw

# 停止进程
kill <PID>

# 重新启动
./target/release/nuclaw
```

### Q5: 如何配置多个群组？

每个群组会自动创建独立目录在 `groups/` 下。群组的 JID 就是目录名。

### Q6: 测试模式怎么运行？

```bash
# 运行所有测试
cargo test

# 运行特定模块测试
cargo test telegram
cargo test whatsapp
```

---

## 附录：完整环境变量一览

### 核心配置

```bash
export ASSISTANT_NAME=Andy              # 触发词
export CONTAINER_TIMEOUT=300000          # 超时时间（毫秒）
export TZ=UTC                            # 时区
export CONTAINER_IMAGE=anthropic/claude-code:latest  # Docker 镜像
export LOG_LEVEL=info                    # 日志级别
```

### Telegram 配置

```bash
export TELEGRAM_BOT_TOKEN=xxx            # Bot Token（必填）
export TELEGRAM_WEBHOOK_URL=             # Webhook URL
export TELEGRAM_WEBHOOK_PATH=telegram-webhook
export TELEGRAM_DM_POLICY=pairing        # DM 策略
export TELEGRAM_GROUP_POLICY=allowlist   # 群组策略
export TELEGRAM_TEXT_CHUNK_LIMIT=4000    # 文本分块大小
export TELEGRAM_WHITELIST_GROUPS=        # 群组白名单
```

### WhatsApp 配置

```bash
export WHATSAPP_MCP_URL=http://localhost:3000  # MCP 地址（必填）
```

### 自定义大模型配置

```bash
export ANTHROPIC_API_KEY=                # API Key
export ANTHROPIC_BASE_URL=              # 自定义 API 端点
export CLAUDE_MODEL=                    # 模型名称
```

---

*文档最后更新：2026-02-16*
