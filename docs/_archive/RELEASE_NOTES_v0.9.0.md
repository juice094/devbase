# devbase v0.9.0 — Workflow Engine 完整闭环

**Full Changelog**: https://github.com/juice094/devbase/compare/v0.8.0...v0.9.0

---

## What's New

### Workflow Loop Step 完整执行

Workflow Engine 的 5 种 step 类型（skill / subworkflow / parallel / condition / **loop**）现已**全部可执行**。

- **`StepType::Loop { for_each, body }`** — 遍历集合并对每项执行 body 子步骤
  - `for_each` 支持 JSON 数组或逗号分隔列表
  - `body` 为 `Vec<StepDefinition>`，内部步骤串行执行
- **Loop 变量插值** — `${loop.item}`（当前项）、`${loop.index}`（零-based 索引）
- **结果聚合** — 每次迭代的 stdout 按 `[index]` 标记聚合；outputs 合并到父 StepResult
- **失败处理** — 单迭代失败按 body step 的 `on_error` 策略（Fail/Continue）处理
- **向后兼容** — 旧 YAML 无 `body` 字段时自动解析为空 `Vec`

### 测试覆盖

- 新增 **12 个单元测试**（279 passed / 0 failed / 3 ignored）
  - `model.rs`：Loop serde 正反序列化 + 向后兼容
  - `interpolate.rs`：`${loop.item}` / `${loop.index}` / 缺失错误
  - `validator.rs`：body ID 重复检测、依赖缺失检测、合法 body 通过
  - `executor.rs`：空集合、单迭代、多迭代聚合、失败处理

---

## Existing Capabilities (v0.8.x)

### Workflow Engine

- **5 种 step 类型**：skill / subworkflow / parallel / condition / loop
- **拓扑调度**：Kahn 算法分批调度，batch 内并行（`std::thread::scope`）
- **变量插值**：`${inputs.x}` / `${steps.y.outputs.z}` / `${env.NAME}`
- **错误策略**：Fail / Continue / Retry(n) / Fallback

### NLQ 自然语言查询

- TUI `[:]` 触发 embedding 语义搜索，失败自动降级为文本搜索
- 搜索结果按 Enter 直接运行 skill

### Mind Market 评分

- `success_rate` / `usage_count` / `rating`（0-5 分，含速度奖励）
- CLI：`skill recalc-scores` / `skill top` / `skill recommend`

### Skill Runtime 全生命周期

- `discover` → `install` → `run` → `score` → `publish`
- 依赖管理：Schema v15 `dependencies`，Kahn 拓扑排序 + DFS 环检测
- 35 个 MCP tools

---

## Migration Notes

- **无 Schema 变更** — Workflow 定义以 YAML 文本存储，`#[serde(default)]` 自动兼容旧数据
- **无 MCP tool 变更** — 现有 `devbase workflow run` CLI/TUI 路径直接受益
- **无配置变更** — 无需修改 `config.toml`

---

## Stats

- **Tests**: 279 passed / 0 failed / 3 ignored
- **Clippy**: 0 warnings (`-D warnings`)
- **Lines of Code**: ~22.7 KLOC
- **MCP Tools**: 35
