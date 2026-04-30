# devbase 文档导航

> **项目状态**：v0.13.0 — 情境编译器闭环构建中  
> **主入口**：[`AGENTS.md`](../AGENTS.md)（Agent 环境指引）· [`ROADMAP.md`](ROADMAP.md)（功能路线图）  
> **最后整理**：2026-04-30

---

## 实时状态看板

| 指标 | 数值 |
|------|------|
| 版本 | v0.13.0 |
| 测试 | 389 passed / 0 failed / 4 ignored |
| Clippy | 0 warnings |
| Schema | v23 |
| MCP Tools | 38 个（Stable 5 / Beta 28 / Experimental 5） |
| 代码行数 | ~30 KLOC |

---

## 快速跳转

| 你是... | 想了解... | 去这里 |
|---------|-----------|--------|
| 新用户 | 5 分钟上手 | [`guides/quickstart.md`](guides/quickstart.md) |
| 用户 | 完整 CLI 命令参考 | [`guides/cli-reference.md`](guides/cli-reference.md) |
| 用户 | 如何接入 MCP（Kimi / Claude / Cursor） | [`guides/mcp-integration.md`](guides/mcp-integration.md) |
| 用户 | Vault 笔记格式 + PARA 工作流 | [`guides/vault-format.md`](guides/vault-format.md) · [`guides/vault-workflow.md`](guides/vault-workflow.md) |
| 开发者 | 数据库 Schema 完整定义 | [`reference/schema-v23.md`](reference/schema-v23.md) |
| 开发者 | 统一实体模型（entities/relations） | [`reference/entities-model.md`](reference/entities-model.md) |
| 开发者 | 38 个 MCP 工具速查 | [`reference/mcp-tools.md`](reference/mcp-tools.md) |
| Agent | 项目架构定义 | [`architecture/context-compiler.md`](architecture/context-compiler.md) |
| 所有人 | 功能路线图 | [`ROADMAP.md`](ROADMAP.md) |

---

## 文档目录

### 🏗️ 架构设计（Architecture）

核心架构文档，定义 devbase 是什么、为什么、怎么做。

| 文档 | 说明 |
|------|------|
| [`architecture/context-compiler.md`](architecture/context-compiler.md) | **v0.13.0 核心定义**：本地情境编译器 — 五层架构、六维信息模型、与 AI Agent 的契约 |
| [`architecture/workflow-dsl.md`](architecture/workflow-dsl.md) | Workflow DSL v0.4.0 规范（YAML 多步骤编排） |
| [`architecture/dependency-topology.md`](architecture/dependency-topology.md) | 模块依赖拓扑（Tier 1–11 自底向上进化顺序） |
| [`architecture/pre-split-evaluation.md`](architecture/pre-split-evaluation.md) | 单 crate vs 多 crate 评估结论 |

### 📖 使用指南（Guides）

面向终端用户的操作手册。

| 文档 | 说明 |
|------|------|
| [`guides/quickstart.md`](guides/quickstart.md) | 5 分钟上手指南：安装 → 扫描 → 索引 → MCP 配置 |
| [`guides/cli-reference.md`](guides/cli-reference.md) | 完整 CLI 子命令参考（scan/health/sync/index/vault/...） |
| [`guides/mcp-integration.md`](guides/mcp-integration.md) | MCP 集成指南：Kimi CLI / Claude Code / Cursor 配置 |
| [`guides/vault-format.md`](guides/vault-format.md) | Vault 笔记格式规范（YAML frontmatter + Markdown） |
| [`guides/vault-workflow.md`](guides/vault-workflow.md) | PARA 目录结构实践（Inbox → Projects → Areas → Resources → Archives） |

### 📚 技术参考（Reference）

面向 AI Agent 和开发者的速查手册。

| 文档 | 说明 |
|------|------|
| [`reference/mcp-tools.md`](reference/mcp-tools.md) | 38 个 MCP 工具完整清单（名称、tier、描述、参数、destructive gate） |
| [`reference/schema-v23.md`](reference/schema-v23.md) | 数据库 Schema v23：全部表结构、列定义、索引、迁移历史 |
| [`reference/entities-model.md`](reference/entities-model.md) | 统一实体模型详解：查询模式、双轨制过渡、自定义扩展 |

### 🗺️ 路线与规划（Roadmaps & Plans）

| 文档 | 说明 |
|------|------|
| [`ROADMAP.md`](ROADMAP.md) | 唯一活跃主路线图。Phase 1–9 全记录 |
| [`plans/docs-reorganization-plan-v0.13.0.md`](plans/docs-reorganization-plan-v0.13.0.md) | **本文档重构计划**（2026-04-30） |

### 🔬 研究分析（Research）

保留有长期价值的深度研究，精简自此前的 10 份文档。

| 文档 | 说明 |
|------|------|
| [`research/ai-tool-context.md`](research/ai-tool-context.md) | AI 开发工具上下文管理机制深度研究 |
| [`research/competitive-analysis.md`](research/competitive-analysis.md) | 竞争格局分析（合并版） |
| [`research/memory-infrastructure.md`](research/memory-infrastructure.md) | 记忆基础设施设计：从 Git repo 到 Knowledge Workspace |
| [`research/ai-infrastructure-analysis.md`](research/ai-infrastructure-analysis.md) | AI 赛道基础设施分析 |
| [`research/competitive-roadmap-table-a.md`](research/competitive-roadmap-table-a.md) | 五战蚕食战略路线 |

### 🗄️ 归档（_archive/）

> 历史文档，保留只读价值，不再维护。

| 文档 | 归档理由 |
|------|----------|
| [`_archive/mcp-contract-v0.1.md`](_archive/mcp-contract-v0.1.md) | v0.1 草案仅 4 个 tool，已实现 38 个 |
| [`_archive/roadmap-2026.md`](_archive/roadmap-2026.md) | 自标"严重过时"（v0.2.3） |
| [`_archive/skill-runtime.md`](_archive/skill-runtime.md) | 已完全实现 |
| [`_archive/tui-skill-integration.md`](_archive/tui-skill-integration.md) | 已完全实现 |
| [`_archive/sprint_2_plan.md`](_archive/sprint_2_plan.md) | Sprint 2 全部完成 |
| [`_archive/smoke_test_report_20260418.md`](_archive/smoke_test_report_20260418.md) | 旧版本测试报告 |
| `_archive/*` | 其余见目录内文件 |

### 📊 运维与进度（Ops & Progress）

| 文档 | 说明 |
|------|------|
| [`ops/code-review-and-ops-plan.md`](ops/code-review-and-ops-plan.md) | v0.10.0 代码审计与运维计划 |
| [`progress/progress-20260430.md`](progress/progress-20260430.md) | v0.13.0 日进度记录 |

---

## 文档维护原则

1. **活跃文档**必须与代码状态同步；出现矛盾时优先修正文档。
2. **`_archive/` 文档**禁止修改内容，仅可添加顶部归档声明。
3. **新增文档**必须在本文档注册，否则视为孤立文档。
4. **每个 Markdown 文档顶部**应包含 `> **状态**：...` 标注。
