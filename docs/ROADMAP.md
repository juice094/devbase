# devbase Roadmap

> **当前阶段**：阶段三 — v0.10.0 发布闭环 / v0.11.0 规划
> 
> **最后更新**：2026-04-26
> 
> **版本状态**：`0.10.0`（L0-L4 知识模型 MVP 已交付）→ 下一里程碑 `0.11.0`（待定）

---

## 阶段一：产品化闭环（v0.3.0）— ✅ 已完成

**核心原则：功能冻结。不修 bug、不补文档之外的一切代码。**

### 验收标准

| # | 标准 | 状态 |
|---|------|------|
| 1 | 34 MCP tools 全量通过 MCP Inspector | ✅ `cargo test --lib mcp` 14 passed |
| 2 | README Quick Start 三步内跑通 | ✅ 已走查，移除虚假声明 |
| 3 | 无"计划中"残留文档 | ✅ roadmap-2026.md 已归档 |
| 4 | CONTRIBUTING.md + ARCHITECTURE.md + AGENTS.md 闭环 | ✅ |
| 5 | Tests 全绿 + Clippy 零警告 | ✅ 239 passed / 0 failed / 3 ignored |
| 6 | GitHub Release 预编译二进制 | ✅ `devbase.exe` 22.6 MB 已上传 |

**Release**: https://github.com/juice094/devbase/releases/tag/v0.3.0

### 明确不做（Deferred）

| 功能 | 原因 | 预计阶段 |
|------|------|---------|
| SSE transport | 未实现，无 ETA | 阶段二或更晚 |
| 跨仓库搜索 (`/`) | TUI grep，新功能 | 阶段二 |
| Stars 趋势可视化 | 新功能，非阻塞 | 阶段二 |
| 自然语言查询 | 新功能，非阻塞 | 阶段二 |
| 智能同步建议 | 新功能，非阻塞 | 阶段二 |
| Skill 市场 / Registry 服务 | 需社区规模支撑 | 阶段二 |
| 跨设备注册表同步 | 依赖 syncthing-rust | 阶段二 |
| 架构拆分为多 crate | 22.7 KLOC 单 crate 仍最优 | 50+ tools 或编译 > 60s 时 |

### 明确不做（Deferred）

| 功能 | 原因 | 预计阶段 |
|------|------|---------|
| SSE transport | 未实现，无 ETA，阻塞发布 | 阶段二或更晚 |
| 跨仓库搜索 (`/`) | TUI grep，新功能 | 阶段二 |
| Stars 趋势可视化 | 新功能，非阻塞 | 阶段二 |
| 自然语言查询 | 新功能，非阻塞 | 阶段二 |
| 智能同步建议 | 新功能，非阻塞 | 阶段二 |
| Skill 市场 / Registry 服务 | 需社区规模支撑 | 阶段二 |
| 跨设备注册表同步 | 依赖 syncthing-rust | 阶段二 |
| 架构拆分为多 crate | 22.7 KLOC 单 crate 仍最优 | 50+ tools 或编译 > 60s 时 |

---

## 阶段二：AI Skill 编排基础设施（v0.4.0）— 进行中

**方向调整**：devbase 不是"个人外置大脑"，而是**将 GitHub 项目转换为 AI 可执行 Skill 的编排基础设施**。

> 50 个仓库不是给人浏览的参考库，而是 AI Skill 的原材料。devbase 的职责是：分析项目 CLI/API 表面 → 自动生成 SKILL.md → 注册到 Skill Registry → 让弱 AI 子代理能够发现、组合、执行。

### 核心文档

- [`docs/architecture/workflow-dsl.md`](architecture/workflow-dsl.md) — Workflow DSL 规范（v0.4.0-reserved）
- [`docs/architecture/workspace-as-schema.md`](architecture/workspace-as-schema.md) — 统一实体模型设计

### Phase 1 已完成 ✅

| 任务 | 状态 | 说明 |
|------|------|------|
| Schema v16 | ✅ | `entity_types` + `entities` + `relations` 统一模型，渐进双写 |
| Skill 自动封装 | ✅ | `devbase skill discover <path>` — Rust/Node/Python/Go/Docker/Generic 检测 |
| 分类体系 | ✅ | `ai`/`dev`/`data`/`infra`/`communication` 二级分类自动推断 |
| Workflow DSL 规范 | ✅ | YAML Schema 冻结，Engine 延后至 v0.5.0 |
| 评分预留 | ✅ | `success_rate`/`usage_count`/`rating` 字段入库，算法 v0.6.0 实现 |

### Phase 2 已完成 ✅（v0.4.0 发布）

| 任务 | 交付物 | 状态 |
|------|--------|------|
| discover 非 dry-run | `devbase skill discover <path>` 默认注册， `--dry-run` 可选预览 | ✅ |
| Git URL discover | `devbase skill discover https://github.com/...` 克隆+分析+注册 | ✅ |
| Skill 执行验证 | devbase/zeroclaw 封装后 `skill run` 验证 entry_script | ✅ |
| MCP discover tool | `devkit_skill_discover` 暴露给 AI Agent（35 tools） | ✅ |
| executor 接口修复 | JSON via stdin 传参，与 discover wrapper 兼容 | ✅ |

### Phase 3 已完成 ✅（v0.4.1–v0.4.2）

| 任务 | 交付物 | 版本 |
|------|--------|------|
| Repo → entities 同步 | `save_repo`/`update_repo_*` 原子双写 entities 表 | v0.4.1 |
| TUI category 显示 | Skill 列表/详情面板显示 `[category]` 标签 | v0.4.1 |
| Skill marketplace 过滤 | `skill list --category <cat>` + `skill search --category <cat>` | v0.4.2 |

### Phase 4 已完成 ✅（v0.5.0 — Workflow Engine）

| 任务 | 交付物 | 优先级 | 状态 |
|------|--------|--------|------|
| Workflow YAML Parser | `workflow::parser` — 无 `type` 标签的 `untagged` 反序列化 | P0 | ✅ |
| Topological Scheduler | `workflow::scheduler` — Kahn 算法分批调度 | P0 | ✅ |
| Parallel Executor | `workflow::executor` — SkillRuntime 集成 + 错误策略 | P0 | ✅ |
| State Persistence | Schema v17 + `workflow::state` CRUD | P0 | ✅ |
| CLI Integration | `devbase workflow {list,show,register,run,delete}` | P0 | ✅ |
| TUI Workflow Panel | `[w]` 键 workflow 列表/详情弹窗 | P1 | ✅ |

### Phase 5 已完成 ✅（v0.6.0 — Mind Market）

| 任务 | 交付物 | 状态 |
|------|--------|------|
| 评分算法 | `skill_runtime::scoring` — success_rate + usage_count + rating (0-5) | ✅ |
| 自动评分更新 | `skill run` 执行后自动重新计算并写入 skills 表 | ✅ |
| CLI `skill recalc-scores` | 批量重新计算所有 skill 评分 | ✅ |
| CLI `skill top` | 按 rating 排序展示 Top-N skills | ✅ |
| CLI `skill recommend` | 按 category 过滤 + 推荐理由 | ✅ |
| TUI Workflow 执行 | `[w]` 详情页 `r/Enter` 运行 + 结果弹窗 | ✅ |

### Phase 6 已完成 ✅（v0.7.0 — 自然语言查询 + 智能同步建议）

| 任务 | 交付物 | 状态 |
|------|--------|------|
| Embedding 查询生成 | `embedding::generate_query_embedding` 迁移到 lib | ✅ |
| TUI NLQ 输入 | `[:]` 键触发自然语言输入行 | ✅ |
| 语义搜索 Skill | 后台线程生成 embedding + `search_skills_semantic` | ✅ |
| NLQ 结果展示 | 弹窗列表展示语义搜索到的 skills | ✅ |
| 智能同步建议 | `sync/policy.rs::recommend_sync_action` — 基于 safety/ahead/behind 生成建议 | ✅ |

### Phase 7 已完成 ✅（v0.8.0 — Workflow 子类型执行）

| 任务 | 交付物 | 状态 |
|------|--------|------|
| Subworkflow 执行 | `execute_subworkflow_step` — 递归调用 `execute_workflow` | ✅ |
| Parallel 执行 | `execute_parallel_step` — 子步骤串行执行 + 结果聚合 | ✅ |
| Condition 执行 | `execute_condition_step` — 字符串插值后 true/false 评估 | ✅ |
| 并行 batch 执行 | `std::thread::scope` 替换串行 loop（#7 风险点修复） | ✅ |

### Phase 8 已完成 ✅（v0.9.0 — Workflow Loop Step 硬化 + 发布闭环）

| 任务 | 交付物 | 状态 |
|------|--------|------|
| Loop Step 结构补全 | `StepType::Loop { for_each, body }` | ✅ |
| Loop Step 执行 | `execute_loop_step` — 集合解析 + 迭代执行 + 结果聚合 | ✅ |
| Loop 变量插值 | `${loop.item}` / `${loop.index}` | ✅ |
| Loop body 验证 | validator 检查 body ID 唯一性 + 依赖有效性 | ✅ |
| 发布闭环 | 版本号、CHANGELOG、AGENTS、ROADMAP 对齐 | ✅ |

### 不做（明确排除）

- ❌ SSE transport（stdio 已足够）
- ❌ `.devbase` 目录规范（无外部采纳者）
- ❌ MCP 协议扩展提案（Star = 0，不会被采纳）
- ❌ 商业化 / 付费版
- ❌ 拆分 crate（22.7 KLOC 单 crate 仍最优）

---

## Phase 9（v0.10.0 — L0-L4 知识模型 Schema 设计）— 规划中

**目标**：将 devbase 从"代码索引"升级为**自指知识库**，支持 L0-L4 五层知识索引。

| 方向 | 状态 | 阻塞因素 |
|:---|:---:|:---|
| L0 对象层 Schema | 📝 草案 | 需定义 `knowledge_objects` 表 + 版本冻结机制 |
| L1 方法层 Schema | 📝 草案 | 需定义检索/分块/向量化方法的元数据表 |
| L2 哲学层 Schema | 📝 草案 | 需与 `vault/` PARA 结构集成 |
| L3 风险层 Schema | 📝 草案 | 需定义 `known_limits` / `boundary_map` 表 |
| L4 元认知层 Schema | 📝 草案 | 需人类纠正信号的存储与一致性校验机制 |
| 生长信号与遗忘机制 | 📝 草案 | 需设计 `frequency` / `confidence` / `expiration` 字段规则 |

---

## Future / Icebox

- ~~自然语言查询~~ ✅ v0.8.1 已完成
- ~~智能同步建议~~ ✅ v0.7.0 已完成
- 跨设备注册表同步（syncthing-rust 集成，REST API 待就绪）
- 形式化验证 / TEE 集成（长期，无排期）
- Workflow 引擎细化（Loop body Retry/Fallback、TUI 执行进度条，无排期）

---

*本 Roadmap 替代 `plans/roadmap-2026.md` 成为唯一活跃主路线图。*
*历史计划见 `docs/archive/`。*
