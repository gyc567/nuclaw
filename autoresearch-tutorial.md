# 🔬 Autoresearch Skill 完全教程

## 什么是 Autoresearch？

Autoresearch 是基于 Andrej Karpathy 的自主实验方法论，改造用于优化 Claude Code 的 SKILL.md 文件。它的核心思想：

> **不要重写整个 skill，而是让它跑几十次、评分每次输出、逐步收紧提示词，直到那 30% 的失败率消失。**

---

## 核心流程图

```
┌─────────────────────────────────────────────────────────────┐
│                    Autoresearch 循环                         │
│                                                             │
│  ┌──────────┐    ┌──────────┐    ┌──────────┐    ┌───────┐ │
│  │ 运行 Skill │ → │  评分输出 │ → │  修改Prompt│ → │  决定  │ │
│  │ N 次      │    │  (Binary) │    │  一次只改1点 │    │保/弃? │ │
│  └──────────┘    └──────────┘    └──────────┘    └───┬───┘ │
│                                                        │     │
│         ┌──────────────────────────────────────────────┘     │
│         │ (如果改善 → 保留，否则回滚)                          │
│         ↓                                                    │
│  ┌──────────────────┐                                        │
│  │  重复直到达到天花板 │ ← 用户停止 / 预算耗尽 / 95%+ 通过率    │
│  └──────────────────┘                                        │
└─────────────────────────────────────────────────────────────┘
```

---

## 📋 使用前的准备工作

**重要：开始前必须与用户确认以下 6 个字段**

| 字段 | 说明 | 示例 |
|------|------|------|
| **1. Target skill** | 要优化的 skill 路径 | `~/.claude/skills/my-skill/SKILL.md` |
| **2. Test inputs** | 3-5 个不同的测试场景 | "OAuth流程图", "CI/CD流水线", "微服务架构" |
| **3. Eval criteria** | 3-6 个二元判断标准 | "文本是否可读?" "是否只用柔和颜色?" |
| **4. Runs per experiment** | 每次实验运行几次 | 默认 5 次 |
| **5. Run interval** | 实验间隔时间 | 默认 2 分钟 |
| **6. Budget cap** | 最大实验次数上限 | 可选，无限制则一直跑 |

---

## 🚀 完整使用步骤

### Step 1: 理解目标 Skill

```
1. 阅读完整的 SKILL.md
2. 阅读 references/ 目录下的文件
3. 理解 skill 的核心任务、流程、输出格式
4. 记录已有的质量检查和反模式
```

### Step 2: 构建 Eval 套件

**每个 Eval 格式：**
```markdown
EVAL [N]: [简短名称]
Question: [关于输出的Yes/No问题]
Pass condition: ["yes" 的样子 — 具体]
Fail condition: [触发 "no" 的条件 — 具体]
```

**Eval 黄金法则：必须二元判断！**
- ❌ 坏："评分 1-7" 
- ❌ 坏："写得怎么样？"
- ✅ 好："是否包含零个禁用词？"
- ✅ 好："文本是否完整无截断？"

**最大分数计算：**
```
max_score = Eval数量 × 每次实验运行次数
例：4个Eval × 5次运行 = 最大20分
```

### Step 3: 创建实时 Dashboard

```bash
# 创建工作目录
mkdir -p autoresearch-[skill-name]/
```

Dashboard 必须：
- 每 10 秒自动刷新
- 显示分数进度折线图
- 彩色条显示：🟢保留 / 🔴丢弃 / 🔵基准
- 展示所有实验的表格
- 显示每个 Eval 的通过率
- 实时状态："Running experiment [N]..." 或 "Idle"

### Step 4: 建立基准线（Experiment #0）

```bash
# 备份原始 Skill
cp SKILL.md SKILL.md.baseline

# 运行 Skill N 次并评分
# 记录基准分数
```

**results.tsv 格式：**
```
experiment	score	max_score	pass_rate	status	description
0	14	20	70.0%	baseline	original skill — no changes
```

**⚠️ 重要：基准线建立后，必须与用户确认是否继续。如果已经是 90%+，可能不需要优化。**

### Step 5: 实验循环

**每次实验只改一件事！**

**好的改动：**
- ✅ 添加针对最常见失败的特定指令
- ✅ 重写模糊指令使其更明确
- ✅ 添加反模式（"禁止做 X"）
- ✅ 把埋藏的指令移到前面（位置=优先级）
- ✅ 改进展示正确行为的示例
- ✅ 删除导致过度优化的指令

**坏的改动：**
- ❌ 从头重写整个 skill
- ❌ 一次加 10 条规则
- ❌ 没有具体原因就加长 skill
- ❌ 添加模糊指令如"做得更好"

**决策逻辑：**
- 分数提升 → **保留**（新的基准线）
- 分数不变 → **丢弃**（增加复杂度没收益）
- 分数下降 → **丢弃**（回滚到上一版本）

### Step 6: 记录 Changelog

```markdown
## Experiment [N] — [keep/discard]

**Score:** [X]/[max] ([percent]%)
**Change:** [一句话描述改动]
**Reasoning:** [为什么这个改动应该有帮助]
**Result:** [实际发生了什么]
**Failing outputs:** [仍然失败的输出描述]
```

### Step 7: 输出结果

1. **分数摘要：** 基准线 → 最终分数（提升百分比）
2. **实验总数：** 尝试了多少次突变
3. **保留率：** 保留 vs 丢弃的比例
4. **Top 3 改动：** 最有帮助的变更
5. **剩余问题：** Skill 仍然出错的地方
6. **优化后的 SKILL.md：** 已保存到原位置
7. **文件位置：** results.tsv 和 changelog.md

---

## 📁 输出文件结构

```
autoresearch-[skill-name]/
├── dashboard.html       # 实时浏览器 Dashboard（自动刷新）
├── results.json         # Dashboard 的数据文件
├── results.tsv          # 每次实验的分数日志
├── changelog.md         # 详细的变更日志
└── SKILL.md.baseline    # 优化前的原始 Skill
```

---

## 💡 实战例子：优化图表生成 Skill

### 上下文收集
- **目标：** `~/.claude/skills/diagram-generator/SKILL.md`
- **测试输入：** "OAuth流程图", "CI/CD流水线", "微服务架构", "用户引导漏斗", "数据库关系"
- **Eval：**
  1. 文本是否可读且拼写正确？
  2. 是否只用柔和/ pastel 颜色？
  3. 线性布局（左到右或上到下）？
  4. 无数字、序号和排序？
- **每次实验运行：** 10 次
- **最大分数：** 40 分

### 实验记录

| 实验 | 分数 | 状态 | 改动 |
|------|------|------|------|
| 0 (基准) | 32/40 (80%) | baseline | 原始 skill |
| 1 | 35/40 (87.5%) | **keep** | 添加"禁止数字序号"指令 |
| 2 | 34/40 (85%) | discard | 添加最小字号要求 |
| 3 | 37/40 (92.5%) | **keep** | 具体十六进制颜色代码替换模糊描述 |
| 4 | 37/40 (92.5%) | discard | 添加霓虹色反模式（无改进） |
| 5 | 39/40 (97.5%) | **keep** | 添加正确格式的示例 |

### 最终结果
- **提升：** 80% → 97.5%
- **Top 改动：** 具体颜色代码、明确反数字规则、工作示例
- **剩余问题：** 复杂图表偶尔标签重叠（1/40 失败率）

---

## 📝 Eval 写作指南

### 好 vs 坏的 Eval

| 类型 | ❌ 坏 Eval | ✅ 好 Eval |
|------|-----------|-----------|
| 文本 | "写得怎么样？" | "是否包含零个禁用词？" |
| 视觉 | "看起来专业吗？" | "文本是否无截断/重叠？" |
| 代码 | "代码干净吗？" | "是否无 TODO 或占位符注释？" |
| 文档 | "是否全面？" | "是否包含所有必需章节？" |

### 常见错误

1. **Eval 太多** → 超过 6 个，Skill 开始"应试"而不是真正提升
2. **太窄/死板** → "必须正好 3 个要点" → 输出变得奇怪
3. **Eval 重叠** → "语法正确" + "无拼写错误" → 重复计数
4. **无法评估** → "人类会觉得有趣吗？" → Agent 无法可靠回答

### 3 问题测试

写完每个 Eval 后问自己：
1. **两个 Agent 给同一输出评分会一致吗？** → 不一致 = 太主观
2. **Skill 能"作弊"通过而不真正改进吗？** → 能 = 太窄
3. **这个 Eval 测试的是用户真正在乎的吗？** → 不是 = 删除

---

## 🎯 何时使用

**触发词：**
- "optimize this skill"
- "improve this skill"
- "run autoresearch on"
- "make this skill better"
- "self-improve skill"
- "benchmark skill"
- "eval my skill"
- "run evals on"

---

## ⚡ 快速开始命令

```bash
# 在 OpenCode 中激活 skill
/skill autoresearch

# 然后说：
# "Optimize my diagram-generator skill"
# 并提供：
# - Skill 路径
# - 3-5 个测试场景
# - 3-6 个二元判断标准
```

---

## 📦 Skill 安装信息

- **Skill 文件位置：** `~/.config/opencode/skills/autoresearch.md`
- **来源：** https://github.com/olelehmann100kMRR/autoresearch-skill
- **Stars：** 229 ⭐
- **Forks：** 30

---

## 附录：Eval 模板

```
EVAL [N]: [Short name]
Question: [Yes/no question about the output]
Pass: [What "yes" looks like — one sentence, specific]
Fail: [What triggers "no" — one sentence, specific]
```

示例：

```
EVAL 1: Text legibility
Question: Is all text in the output fully legible with no truncated, overlapping, or cut-off words?
Pass: Every word is complete and readable without squinting or guessing
Fail: Any word is partially hidden, overlapping another element, or cut off at the edge
```
