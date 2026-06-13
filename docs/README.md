# devbase 文档导航

> **项目状态**：v0.20.1 · Schema v36 · 71 MCP tools · 605 tests · 12 workspace crates
> **权威入口**：[`AGENTS.md`](../AGENTS.md)（Agent 环境指引）· [`CHANGELOG.md`](../CHANGELOG.md)（版本变更）
> **最后整理**：2026-06-13

---

## 实时状态看板

| 指标 | 数值 | 来源 |
|------|------|------|
| 版本 | v0.20.1 | `Cargo.toml` |
| Rust Edition | 2024 | `Cargo.toml` |
| 测试 | 605 passed / 0 failed / 7 ignored | `cargo test --workspace -- --list` |
| Clippy | `-D warnings` 全绿 | CI |
| Schema | v36 | `src/registry/migrate.rs` |
| MCP Tools | **71**（5 Stable / 62 Beta / 4 Experimental） | `src/mcp/mod.rs` |
| Workspace Crates | 12 | `crates/` |
| `main.rs` | 833 行（RF-4 ≤ 1000） | `wc -l` |
| RF-6 | ✅ 生产代码 unwrap/expect/panic 清零 | invariant checks |

---

## 快速跳转

| 你是... | 想了解... | 去这里 |
|---------|-----------|--------|
| 新用户 | 5 分钟上手 | [`guides/quickstart.md`](guides/quickstart.md) |
| 用户 | 完整 CLI 命令参考 | [`guides/cli-reference.md`](guides/cli-reference.md) |
| 用户 | 如何接入 MCP（Kimi / Claude / Cursor） | [`guides/mcp-integration.md`](guides/mcp-integration.md) |
| 用户 | Vault 笔记格式 + PARA 工作流 | [`guides/vault-format.md`](guides/vault-format.md) · [`guides/vault-workflow.md`](guides/vault-workflow.md) |
| 用户 | Embedding Provider 配置 | [`guides/embedding-provider-setup.md`](guides/embedding-provider-setup.md) |
| 开发者 | 数据库 Schema 完整定义 | [`reference/schema-v36.md`](reference/schema-v36.md) |
| 开发者 | 统一实体模型（entities/relations） | [`reference/entities-model.md`](reference/entities-model.md) |
| 开发者 | 71 个 MCP 工具速查 | [`reference/mcp-tools.md`](reference/mcp-tools.md) |
| Agent | 项目架构定义 | [`architecture/context-compiler.md`](architecture/context-compiler.md) |
| Agent | 架构红线与不变量 | [`architecture/invariants.md`](architecture/invariants.md) |
| 维护者 | 功能路线图 | [`ROADMAP.md`](ROADMAP.md) |
| 维护者 | 已知问题与技术债务 | [`../KNOWN_ISSUES.md`](../KNOWN_ISSUES.md) |

---

## 文档目录

### 🏗️ 架构设计（Architecture）

核心架构文档，定义 devbase 是什么、为什么、怎么做。

| 文档 | 说明 |
|------|------|
| [`architecture/overview.md`](architecture/overview.md) | 三层架构：字节 → 语义 → 行动 |
| [`architecture/context-compiler.md`](architecture/context-compiler.md) | **核心定义**：本地情境编译器 — 五层架构、六维信息模型 |
| [`architecture/workflow-dsl.md`](architecture/workflow-dsl.md) | Workflow DSL 规范（YAML 多步骤编排） |
| [`architecture/dependency-topology.md`](architecture/dependency-topology.md) | 模块依赖拓扑（Tier 1–11） |
| [`architecture/invariants.md`](architecture/invariants.md) | **架构红线** RF-1~RF-7 + 分层约束 G/T |
| [`architecture/split-plan.md`](architecture/split-plan.md) | Workspace crate 拆分计划 |
| [`architecture/pre-split-evaluation.md`](architecture/pre-split-evaluation.md) | 单 crate vs 多 crate 评估 |
| [`architecture/adr-template.md`](architecture/adr-template.md) | ADR 模板与已完成决策索引 |
| [`architecture/adr-003-tantivy-sqlite-consistency.md`](architecture/adr-003-tantivy-sqlite-consistency.md) | ADR-003：Tantivy/SQLite 一致性 |
| [`architecture/adr-004-mcp-trait-decoupling.md`](architecture/adr-004-mcp-trait-decoupling.md) | ADR-004：MCP trait 解耦 |
| [`architecture/adr-005-appcontext-clone.md`](architecture/adr-005-appcontext-clone.md) | ADR-005：AppContext Clone 边界 |

### 📖 使用指南（Guides）

面向终端用户的操作手册。

| 文档 | 说明 |
|------|------|
| [`guides/quickstart.md`](guides/quickstart.md) | 5 分钟上手指南 |
| [`guides/cli-reference.md`](guides/cli-reference.md) | 完整 CLI 子命令参考 |
| [`guides/mcp-integration.md`](guides/mcp-integration.md) | MCP 集成指南（Kimi / Claude / Cursor） |
| [`guides/vault-format.md`](guides/vault-format.md) | Vault 笔记格式规范 |
| [`guides/vault-workflow.md`](guides/vault-workflow.md) | PARA 目录结构实践 |
| [`guides/embedding-provider-setup.md`](guides/embedding-provider-setup.md) | Embedding Provider 配置 |
| [`guides/ai-instance-handoff.md`](guides/ai-instance-handoff.md) | AI 实例交接指南 |

### 📚 技术参考（Reference）

面向 AI Agent 和开发者的速查手册。

| 文档 | 说明 |
|------|------|
| [`reference/mcp-tools.md`](reference/mcp-tools.md) | 71 个 MCP 工具完整清单 |
| [`reference/schema-v36.md`](reference/schema-v36.md) | 数据库 Schema v36（表、列、索引、迁移历史） |
| [`reference/entities-model.md`](reference/entities-model.md) | 统一实体模型详解 |
| [`reference/stable-tools/README.md`](reference/stable-tools/README.md) | 5 个 Stable 工具独立文档 |

### 🗺️ 路线与规划（Roadmaps & Plans）

| 文档 | 说明 |
|------|------|
| [`ROADMAP.md`](ROADMAP.md) | 唯一活跃主路线图 |
| [`plans/v0.21.0-architecture-hardening.md`](plans/v0.21.0-architecture-hardening.md) | v0.21.0 架构硬化计划 |
| [`plans/greptimedb-integration.md`](plans/greptimedb-integration.md) | GreptimeDB 可选集成计划 |
| [`ops/roadmap-v0.14-v0.16.md`](ops/roadmap-v0.14-v0.16.md) | 历史路线图归档 |

### 🔬 研究分析（Research）

保留有长期价值的深度研究。

| 文档 | 说明 |
|------|------|
| [`theory/AI_TOOL_CONTEXT_RESEARCH.md`](theory/AI_TOOL_CONTEXT_RESEARCH.md) | AI 开发工具上下文管理机制 |
| [`research/competitive-analysis.md`](research/competitive-analysis.md) | 竞争格局分析 |
| [`research/memory-infrastructure.md`](research/memory-infrastructure.md) | 记忆基础设施设计 |
| [`research/ai-infrastructure-analysis.md`](research/ai-infrastructure-analysis.md) | AI 赛道基础设施分析 |
| [`research/competitive-roadmap-table-a.md`](research/competitive-roadmap-table-a.md) | 五战蚕食战略路线 |

### 📐 RFC（Request for Comments）

| 文档 | 说明 |
|------|------|
| [`RFC/agent-memory-vector-storage.md`](RFC/agent-memory-vector-storage.md) | Agent Memory 向量存储 RFC |
| [`RFC/claudecode-workflow-integration.md`](RFC/claudecode-workflow-integration.md) | ClaudeCode 工作流集成 RFC |

### 🖥️ 客户端适配（Clients）

客户端无关原则下的适配示例。

| 文档 | 说明 |
|------|------|
| [`clients/claude/scenarios.md`](clients/claude/scenarios.md) | Claude Code 使用场景 |

### 🗄️ 归档（_archive/）

> 历史文档，保留只读价值，不再维护。新增归档文件需在本文档注册。

| 文档 | 归档理由 |
|------|----------|
| [`_archive/mcp-contract-v0.1.md`](_archive/mcp-contract-v0.1.md) | v0.1 草案，已实现 71 个 tool |
| [`_archive/roadmap-2026.md`](_archive/roadmap-2026.md) | 严重过时（v0.2.3） |
| [`_archive/skill-runtime.md`](_archive/skill-runtime.md) | 已完全实现 |
| [`_archive/tui-skill-integration.md`](_archive/tui-skill-integration.md) | 已完全实现 |
| `_archive/*` | 其余见目录内文件 |

### 📊 运维与进度（Ops & Progress）

| 文档 | 说明 |
|------|------|
| [`ops/code-review-and-ops-plan.md`](ops/code-review-and-ops-plan.md) | 代码审计与运维计划 |
| [`progress/progress-20260430.md`](progress/progress-20260430.md) | v0.13.0 日进度记录 |

### 📋 其他门面文件

| 文件 | 说明 |
|------|------|
| [`../AGENTS.md`](../AGENTS.md) | Agent 环境指引（权威） |
| [`../CONTRIBUTING.md`](../CONTRIBUTING.md) | 贡献指南 |
| [`../SECURITY.md`](../SECURITY.md) | 安全策略 |
| [`../CHANGELOG.md`](../CHANGELOG.md) | 版本变更日志 |
| [`../KNOWN_ISSUES.md`](../KNOWN_ISSUES.md) | 已知问题与技术债务 |

---

## 文档维护原则

1. **活跃文档**必须与代码状态同步；出现矛盾时优先修正文档。
2. **`_archive/` 文档**禁止修改内容，仅可添加顶部归档声明。
3. **新增文档**必须在本文档注册，否则视为孤立文档。
4. **每个 Markdown 文档顶部**应包含 `> **状态**：...` 标注。
5. **关键数字指标**（版本、Schema、Tools、Tests）必须从代码/CI 实测，禁止复制粘贴旧值。

---

*本文件是文档目录的唯一入口。修改文档结构时请同步更新本文件。*
