# OpenSpec 提案 - Skill 热加载框架

## 执行摘要

本提案在现有 Skill 系统基础上，实现热加载 + Rig Tool 集成，参考 Extism + Rig 架构设计。

## 一、用户故事 (User Stories)

### Story 1: Skill 热加载

**作为** NuClaw 用户  
**我希望** 在不重启服务的情况下添加/修改/删除 Skill  
**以便** 实时扩展 Agent 能力

#### 验收标准

- [x] 新增 Skill 文件后自动加载，无需重启
- [x] 修改现有 Skill 后自动更新，无需重启
- [x] 删除 Skill 后自动卸载，无需重启
- [x] 热加载失败时不影响现有功能
- [x] 支持批量操作（同时增删改多个 Skill）

---

### Story 2: Skill 类型扩展

**作为** Skill 开发者  
**我希望** Skill 支持 Tool 类型（可执行外部工具）  
**以便** 创建更强大的 Agent 能力

#### 验收标准

- [x] Skill 支持 Text 类型（现有：纯文本 prompt）
- [x] Skill 支持 Tool 类型（新增：绑定外部工具）
- [x] 现有 Skill 保持兼容，无需迁移
- [x] Skill 类型可从 YAML frontmatter 识别
- [x] Tool 类型 Skill 可声明所需工具列表

---

### Story 3: ToolRegistry 注册表

**作为** 系统开发者  
**我希望** 统一管理所有可执行工具  
**以便** Rig Agent 可以动态调用

#### 验收标准

- [x] 提供统一的 ToolRegistry trait
- [x] 支持工具注册（register）
- [x] 支持工具查找（get）
- [x] 支持工具列表（list）
- [x] 工具调用返回结构化结果

---

### Story 4: Rig Tool 适配器

**作为** Agent 开发者  
**我希望** 将 Skill 转换为 Rig Tool  
**以便** LLM 可以动态选择使用

#### 验收标准

- [x] Skill 可转换为 Rig Tool
- [x] Tool 定义包含名称、描述、参数schema
- [x] 工具执行返回结构化输出
- [x] 支持异步执行
- [x] 错误处理标准化

---

### Story 5: RigRunner 工具集成

**作为** NuClaw 开发者  
**我希望** RigRunner 支持动态工具  
**以便** Agent 可以调用 Skill 工具

#### 验收标准

- [x] RigRunner 初始化时加载所有可用工具
- [x] Agent 执行时自动注入工具定义
- [x] 工具调用结果正确传递
- [x] 保持与 ContainerRunner/ApiRunner 接口一致

---

### Story 6: Skill 配置管理

**作为** 系统管理员  
**我希望** 通过配置文件管理 Skill 行为  
**以便** 细粒度控制 Skill 执行

#### 验收标准

- [x] 支持 YAML 配置文件
- [x] 可配置热加载开关
- [x] 可配置超时时间
- [x] 可配置并发限制

---

## 二、技术设计

### 2.1 模块结构

```
src/
├── skills.rs              # 现有 Skill 定义（扩展）
├── skill_watcher.rs       # 新增：热加载监控
├── skill_config.rs       # 新增：配置管理
├── skill_registry.rs     # 新增：Registry 统一接口
├── tool_registry.rs      # 新增：Tool 注册表
├── skill_to_rig.rs      # 新增：Rig 适配器
├── skill_hot_reloader.rs # 新增：热加载集成
├── hot_reload_registry.rs # 新增：热加载注册表
├── wasm_executor.rs     # 新增：WASM 执行器
├── agent_runner.rs       # 修改：集成工具
```

### 2.2 数据模型

```rust
// 扩展后的 Skill
pub struct Skill {
    // 现有字段（保留）
    pub name: String,
    pub description: String,
    pub content: String,
    
    // 新增字段
    pub skill_type: SkillType,     // Text | Tool | Wasm
    pub tools: Vec<String>,         // Tool 类型所需工具
    pub config: HashMap<String, Value>,
}

pub enum SkillType {
    Text,   // 纯文本 prompt
    Tool,   // 可执行工具
    Wasm,   // WASM 模块（Phase 3）
}
```

### 2.3 热加载流程

```
SkillWatcher (notify crate)
    │
    ├── CREATE → SkillRegistry.register()
    ├── MODIFY → SkillRegistry.reload()
    └── REMOVE → SkillRegistry.unregister()
```

---

## 三、实施计划

### Phase 1: 基础设施

| 任务 | 用户故事 | 验收标准 |
|------|----------|----------|
| 扩展 Skill 数据模型 | Story 2 | 5/5 |
| 实现 SkillWatcher | Story 1 | 4/5 |
| 实现 SkillRegistry | Story 1, 3 | 5/5 |

### Phase 2: Rig 集成

| 任务 | 用户故事 | 验收标准 |
|------|----------|----------|
| 定义 ToolRegistry | Story 3 | 4/4 |
| 实现 SkillAsTool | Story 4 | 5/5 |
| 改造 RigRunner | Story 5 | 4/4 |

### Phase 3: 配置管理

| 任务 | 用户故事 | 验收标准 |
|------|----------|----------|
| 实现 SkillConfig | Story 6 | 4/4 |

---

## 四、测试策略

### 单元测试

- Skill 数据模型：100% 覆盖
- SkillWatcher 事件：100% 覆盖
- ToolRegistry：100% 覆盖
- SkillAsTool：100% 覆盖

### 集成测试

- Skill 热加载流程
- Rig Runner 工具调用
- 配置加载/解析

---

## 五、风险评估

| 风险 | 影响 | 缓解 |
|------|------|------|
| 热加载竞态 | 中 | RwLock 保护 |
| 破坏现有功能 | 高 | 保持兼容 |
| Rig API 变更 | 低 | 适配层隔离 |

---

## 六、验收确认

- [ ] 所有用户故事验收标准达成
- [ ] 新增代码 100% 测试覆盖
- [ ] 现有测试全部通过
- [ ] 文档完整更新

---

**提案版本**: v1.0  
**生成日期**: 2026-03-17  
**状态**: 🔄 实施中
