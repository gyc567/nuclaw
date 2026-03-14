# NuClaw AutoResearch 模块使用指南

**NuClaw AutoResearch** - 自主 LLM 训练实验自动化模块

---

## 1. 概述

AutoResearch 模块赋予 AI Agent 自主进行 LLM 训练实验的能力:

```
修改代码 → 训练 → 评估 → 保留/丢弃 → 重复
```

基于 [AutoSearch](https://github.com/ClawTechForEric/autoresearch) 概念设计。

---

## 2. 快速开始

### 2.1 基本使用

```rust
use nuclaw::autoresearch::{AutoResearchRunner, ExperimentConfig, Program};

let config = ExperimentConfig::default();
let program = Program::load("program.md")?;
let mut runner = AutoResearchRunner::new(config, program);

runner.run_full_loop(|iter, best| {
    // AI Agent 生成新的训练代码
    format!("# iteration {}\ntrain_model()", iter)
})?;
```

### 2.2 命令行使用

```bash
# 配置 LLM
./nuclaw --onboard

# 运行自主研究
./nuclaw --autoresearch
```

---

## 3. 配置

### 3.1 ExperimentConfig

| 参数 | 默认值 | 说明 |
|------|--------|------|
| time_budget_secs | 300 | 时间预算(秒) |
| max_iterations | 100 | 最大迭代次数 |
| metric | val_bpb | 评估指标 |
| early_stop_patience | 5 | 早停耐心值 |
| output_dir | experiments | 输出目录 |

### 3.2 支持的指标

| 指标 | 说明 |
|------|------|
| val_bpb | validation bits per byte (默认) |
| val_loss | validation loss |
| train_loss | training loss |

---

## 4. 程序模板 (program.md)

```yaml
---
name: my-research
description: Optimize LLM training hyperparameters
compatibility: Single GPU, Python 3.10+
---

# Research Goal
找到最优的超参数组合来提升模型性能。

# 修改策略
1. 调整学习率 (1e-5 到 1e-3)
2. 调整 batch size (16, 32, 64)
3. 调整层数和注意力头数

# 评估指标
优化 val_bpb，越低越好
```

---

## 5. 与 NuClaw 集成

### 5.1 自动化工作流

```rust
// 结合 Skills 系统
use nuclaw::skills::SkillRegistry;

let skills = builtin_skills();
let research_skill = skills.get("autoresearch").unwrap();

// 使用 skill 内容作为研究指令
```

### 5.2 Telegram/WhatsApp 通知

```rust
// 实验完成后发送通知
runner.run_full_loop(|iter, best| {
    let script = generate_script(iter, best);
    
    // 发送进度到 Telegram
    telegram_client.send_message(
        &chat_id,
        &format!("实验 {} 完成: {:?}", iter, best.metric_value)
    ).await;
    
    script
})?;
```

---

## 6. 示例程序

### 6.1 超参数优化

```python
# train.py 示例 (由 AI 修改)
import torch

# 可调超参数
LR = 1e-4  # AI 会修改这个值
BATCH_SIZE = 32   # AI 会修改这个值
NUM_LAYERS = 12     # AI 会修改这个值

def train():
    model = build_model(NUM_LAYERS)
    optimizer = torch.optim.AdamW(model.parameters(), lr=LR)
    
    for batch in dataloader(BATCH_SIZE):
        loss = model(batch)
        loss.backward()
        optimizer.step()
    
    return evaluate(model)
```

### 6.2 程序指令

```markdown
# program.md

你是一个 LLM 训练研究员。你的任务是优化 train.py 中的超参数。

目标: 最小化 val_bpb

策略:
1. 尝试不同的学习率 (1e-5 到 1e-3)
2. 尝试不同的 batch size (16, 32, 64)
3. 尝试不同的模型架构

约束:
- 单 GPU
- 每次实验 5 分钟
- 记录所有尝试和结果
```

---

## 7. API 参考

### 7.1 核心类型

```rust
// 实验配置
pub struct ExperimentConfig {
    pub time_budget_secs: u64,
    pub max_iterations: u64,
    pub metric: Metric,
    pub early_stop_patience: u32,
}

// 实验结果
pub struct ExperimentResult {
    pub iteration: u32,
    pub metric_value: f64,
    pub is_improvement: bool,
    pub duration_secs: u64,
}
```

### 7.2 主要函数

```rust
// 创建运行器
let runner = AutoResearchRunner::new(config, program);

// 运行完整研究循环
runner.run_full_loop(|iter, best| {
    // 生成新的训练代码
    format!("# iteration {}\n", iter)
})?;

// 获取最佳结果
let best = runner.best_result();

// 保存实验历史
runner.save_results(&PathBuf::from("results.json"))?;
```

---

## 8. 测试

```bash
# 运行模块测试
cargo test autoresearch:: -- --test-threads=1
```

---

## 9. 常见问题

### Q: 如何选择指标?

A: `val_bpb` 最适合架构比较,不受 vocab size 影响。

### Q: 如何调整实验时间?

A: 修改 `time_budget_secs`,默认 300 秒 (5 分钟)。

### Q: 如何提前停止?

A: 设置 `early_stop_patience`,连续 N 次无改进则停止。

---

*文档版本: 1.0.0*
