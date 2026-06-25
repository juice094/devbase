---
type: AgentInstruction
title: devbase Agent 环境指引
description: devbase 项目入口指引。完整架构、规范与开发约定见 .knowledge/ OKF bundle。
version: 0.20.1
schema_version: 36
mcp_tools: 71
crates: 12
tests: 616+
timestamp: 2026-06-25T11:15:50Z
tags: [agent-instruction, devbase, rust, onboarding]
---

# devbase Agent 环境指引

> **本文件是入口索引**。详细的项目架构、拓扑、Registry 规范、MCP 路径、开发规范见 [`.knowledge/index.md`](./.knowledge/index.md)。

## 项目速览

**devbase**（v0.20.1）是一个本地优先的开发者工作空间数据库与知识库管理器。它把代码仓库、PARA 笔记、Skill 与工作流编译为 AI 可推理的结构化情境。

**一句话定位（适合简历/演讲）**：devbase 是“开发者工作空间的世界模型编译器”——用 Rust 将本地 Git 仓库、Markdown 笔记、Skill 脚本和工作流编译成 AI 可直接推理的结构化上下文，并通过 71 个 MCP 工具与 TUI 仪表盘对外提供服务。

| 指标 | 数值 |
|------|------|
| 版本 | 0.20.1 |
| Rust Edition | 2024（rustc 1.95+） |
| Registry Schema | v36 |
| MCP Tools | 71 |
| Workspace Crates | 12 |
| 测试 | 616+ |
| 主入口 `main.rs` | 836 行 |
| 生产 `unwrap` | 0 |

## 知识包导航

| 想了解 | 去读 |
|--------|------|
| 完整项目地图 | [`.knowledge/index.md`](./.knowledge/index.md) |
| 三层架构与数据流 | [`.knowledge/architecture/three-layer-model.md`](./.knowledge/architecture/three-layer-model.md) |
| 11-Tier 模块依赖拓扑 | [`.knowledge/architecture/dependency-topology.md`](./.knowledge/architecture/dependency-topology.md) |
| 架构红线与不变量 | [`.knowledge/architecture/invariants.md`](./.knowledge/architecture/invariants.md) |
| Registry Schema 与迁移 | [`.knowledge/registry/`](./.knowledge/registry/index.md) |
| MCP Tool 添加路径 | [`.knowledge/mcp/tool-adding-guide.md`](./.knowledge/mcp/tool-adding-guide.md) |
| 构建、测试、风格规范 | [`.knowledge/development/`](./.knowledge/development/index.md) |
| 本 bundle 变更历史 | [`.knowledge/log.md`](./.knowledge/log.md) |

## 模块地图（按三层架构）

本项目采用 `src/` + `crates/` 的经典 Rust workspace 布局。`src/` 聚合所有功能模块，`crates/` 放置零内部耦合的可复用子 crate。

### 交互层（Application / Protocol）
人类与 AI 的入口：

| 模块 | 文件/目录 | 关键文件 | 职责 |
|------|----------|----------|------|
| CLI 入口 | `src/main.rs` | `main.rs` | 命令解析与分发，硬限界 ≤ 1000 行 |
| 模块导出 | `src/lib.rs` | `lib.rs` | 导出 30+ 模块 |
| 子命令 | `src/commands/` | `repo.rs` / `skill.rs` / `workflow.rs` / `knowledge.rs` / `analysis.rs` / `ontology.rs` / `limit.rs` / `system.rs` / `simple.rs` | 9 类 CLI 子命令实现 |
| TUI 仪表盘 | `src/tui/` | `mod.rs` / `event.rs` / `layout.rs` / `theme.rs` / `render/` / `state/` | ratatui 终端界面：事件、布局、主题、渲染、状态 |
| MCP Server | `src/mcp/` | `mod.rs` / `tools/mod.rs` / `tests.rs` / `tools/*.rs` | stdio 通信，`McpTool` trait，71 个工具实现 |

### 编译层（Semantic / Knowledge）
把原始输入编译为可推理知识：

| 模块 | 文件/目录 | 关键文件 | 职责 |
|------|----------|----------|------|
| Registry | `src/registry/` | `migrate.rs` / `test_helpers.rs` / `entity.rs` / `knowledge.rs` / `knowledge_meta.rs` / `call_graph.rs` / `dead_code.rs` / `health.rs` / `import_ontology.rs` / `agent_context.rs` | SQLite schema、迁移、实体、关系、调用图、死代码检测 |
| 搜索 | `src/search/` | `hybrid.rs` / `symbol_index.rs` | Tantivy BM25 + 向量混合检索 |
| Vault / PARA 笔记 | `src/vault/` | `mod.rs` / `scanner.rs` / `indexer.rs` / `frontmatter.rs` / `wikilink.rs` / `backlinks.rs` / `fs_io.rs` / `history.rs` / `export.rs` | Markdown + YAML frontmatter、wikilink、backlinks、BFS 图遍历 |
| Skill 运行时 | `src/skill_runtime/` | `mod.rs` / `discover.rs` / `executor.rs` / `parser.rs` / `scoring.rs` / `publish.rs` / `registry.rs` / `sources.rs` / `dependency.rs` / `clarity_sync.rs` | 发现 → 安装 → 执行 → 评分 → 发布 |
| 工作流引擎 | `src/workflow/` | `mod.rs` / `model.rs` / `parser.rs` / `validator.rs` / `scheduler.rs` / `executor.rs` / `interpolate.rs` / `state.rs` | YAML DAG 编排：5 种 step 类型 + 插值 + 调度 + 验证 |
| 知识引擎 | `src/knowledge_engine/` | 各提取器模块 | 代码符号提取、README 摘要与关键词生成 |
| 同步 | `src/sync/` | `orchestrator.rs` / `policy.rs` / `tasks.rs` / `tests.rs` | 仓库同步编排、策略与任务 |

### 可靠层（Physical / Storage）
本地优先的数据与索引底座：

| 组件 | 技术/位置 | 说明 |
|------|----------|------|
| 关系存储 | SQLite WAL (`registry.db`) | `PRAGMA user_version` 驱动迁移 |
| 全文索引 | Tantivy | 代码符号与 Vault 笔记的 BM25 |
| 语义向量 | SQLite BLOB + `cosine_similarity` UDF | 零 ML 运行时依赖 |
| 代码解析 | tree-sitter | Rust / Python / TypeScript / Go |
| 版本控制 | git2 | Git 仓库状态与操作 |

### 独立 Workspace Crates

`crates/` 目录包含 12 个零内部耦合的子 crate：

```
devbase-core-types          # 最底层：Node / Edge / NodeType 等核心类型
devbase-registry            # Registry 核心逻辑
devbase-embedding           # 本地文本嵌入：Candle + Ollama 后端
devbase-vault-wikilink      # WikiLink 解析器
devbase-vault-frontmatter   # Vault Frontmatter 解析
devbase-skill-runtime-parser# Skill 运行时解析器
devbase-skill-runtime-types # Skill 运行时类型
devbase-symbol-links        # 符号链接
devbase-sync-protocol       # 同步协议
devbase-syncthing-client    # Syncthing 集成客户端
...                         # 其余未在此穷举
```

## Crate 依赖方向

```
devbase-core-types （零内部耦合）
    ↓
{ devbase-registry, devbase-embedding, devbase-vault-*, devbase-skill-runtime-* }
    ↓
src/ 各模块（commands, tui, mcp, registry, search, vault, skill_runtime, workflow, ...）
```

- `devbase-core-types` 为最底层基础 crate，禁止依赖任何 devbase 内部 crate。
- `crates/` 内各 crate 禁止直接调用主 crate (`src/`) 的 `crate::` 路径。
- `src/` 各模块可聚合所有 crate 与内部模块能力，`main.rs` 为唯一二进制入口。

## 核心红线（违反任意一条 → HALT）

红线 `G1–G7` 与 `CLAUDE.md` 中的 `RF-1–RF-7` 一一对应：

1. **G1 依赖注入**（RF-1）：禁止新增 `dirs::data_local_dir()` / `std::env::var_os` 硬编码路径，所有 IO 边界通过 `StorageBackend` / `AppContext` 注入。
2. **G2 测试密封性**（RF-2）：测试禁止修改全局进程状态，文件系统测试用 `tempfile` + `StorageBackend`。
3. **G3 Schema 单一事实来源**（RF-3）：`SCHEMA_DDL` 与 `migrate.rs` 必须原子同步。
4. **G4 二进制入口限界**（RF-4）：`main.rs` 不得超过 1000 行。
5. **G5 生产代码无 panic**（RF-6）：禁止 `unwrap()` / `expect()` / `panic!()`。
6. **G6 无循环依赖**（RF-5）：禁止模块间双向 `use crate::` 引用。
7. **G7 Workspace 拆分约束**（RF-7）：新增模块若对 devbase 内部 `crate::` 引用超过 5 个，禁止提取为 workspace crate。

## 常用命令

```powershell
# 构建
cargo build --release

# 本地体验
cargo run -- scan . --register
cargo run -- tui
cargo run -- mcp

# 测试与检查（提交前必须）
cargo test --all-targets
cargo clippy --all-targets -D warnings
cargo fmt --check

# 架构不变量检查
scripts/invariant-checks/run-checks.ps1
```

## 给 Agent 的默认指令

1. **修改前先读知识包**：复杂任务先浏览 [`.knowledge/architecture/dependency-topology.md`](./.knowledge/architecture/dependency-topology.md) 定位模块层级。
2. **改 Schema 先读迁移策略**：任何 `registry.db` 表结构变更遵循 [`.knowledge/registry/migration-policy.md`](./.knowledge/registry/migration-policy.md)。
3. **添加 MCP Tool 走标准路径**：见 [`.knowledge/mcp/tool-adding-guide.md`](./.knowledge/mcp/tool-adding-guide.md)。
4. **优先使用项目内工具**：读文件用 `Read`，搜索用 `Grep`/`Glob`，小修改用 `Edit`，新建/整文件用 `Write`，构建测试用 `Bash`。
5. **不要修改 `.gitignore` 覆盖范围之外的敏感文件**：禁止在源码/注释/测试数据中硬编码真实 token、api_key 或密码。
6. **保持门面文件一致**：若修改了 MCP 工具数、Schema 版本、测试数、红线规则，同步更新 `README.md`、`AGENTS.md`、`CLAUDE.md`、`.knowledge/index.md` 与 `.knowledge/architecture/project-worktree.md` 中的对应数字。

---

## 完整项目结构参考

> 本节按实际文件系统整理，便于新 Agent 快速建立全局认知。OKF 归档版本见 [`.knowledge/architecture/project-worktree.md`](./.knowledge/architecture/project-worktree.md)。

### 顶层布局

```text
devbase/
├── .cargo/                  # 本地 Cargo 配置
├── .github/                 # CI / Release workflows
├── .knowledge/              # OKF Knowledge Bundle
├── benches/                 # Criterion 基准测试
├── crates/                  # 12 个独立 workspace crate
├── docs/                    # 人类可读文档导航
├── examples/                # 可运行示例
├── scripts/                 # 安装脚本与 CI 辅助
├── skills/                  # 示例 Skill
├── src/                     # 主应用程序（30+ 模块）
├── tests/                   # 集成测试
├── AGENTS.md                # Agent 入口指引
├── CLAUDE.md                # Claude Code 专用指引
├── Cargo.toml               # Workspace 配置
└── README.md                # 项目首页
```

### `src/` 主应用程序

#### 交互层

```text
src/
├── main.rs                  # CLI 入口（RF-4 ≤ 1000 行）
├── lib.rs                   # 导出 30+ 模块
├── commands/                # 9 类 CLI 子命令
│   ├── mod.rs / repo.rs / skill.rs / workflow.rs / knowledge.rs
│   ├── analysis.rs / ontology.rs / limit.rs / system.rs / simple.rs
├── tui/                     # ratatui 终端仪表盘
│   ├── mod.rs / event.rs / layout.rs / theme.rs
│   ├── render/              # 渲染组件
│   └── state/               # 状态机
└── mcp/                     # MCP Server（stdio，71 个工具）
    ├── mod.rs               # McpTool trait、路由
    ├── clients.rs / tests.rs
    └── tools/               # 71 个工具实现
```

#### 编译层

```text
src/
├── registry/                # SQLite Registry：schema、迁移、实体、关系
│   ├── migrate.rs / test_helpers.rs
│   ├── entity.rs / relation.rs / repo.rs / workspace.rs
│   ├── knowledge.rs / knowledge_meta.rs / vault.rs
│   ├── code_symbols.rs / call_graph.rs / dead_code.rs / links.rs
│   ├── metrics.rs / known_limits.rs / import_ontology.rs
│   ├── agent_context.rs / health.rs / tests.rs
├── repository/              # 仓库业务抽象
│   ├── mod.rs / repo.rs / workspace.rs / dependency.rs
│   ├── health.rs / knowledge.rs / search.rs / symbol.rs
├── search/                  # Tantivy BM25 + 向量混合检索
│   ├── mod.rs / hybrid.rs / symbol_index.rs
├── semantic_index/          # tree-sitter 代码符号提取
│   ├── mod.rs / symbol.rs / call_graph.rs / git_diff.rs / persist.rs
├── vault/                   # PARA 笔记系统
│   ├── mod.rs / scanner.rs / indexer.rs
│   ├── frontmatter.rs / wikilink.rs / backlinks.rs
│   ├── fs_io.rs / history.rs / export.rs
├── skill_runtime/           # Skill 生命周期
│   ├── mod.rs / parser.rs / registry.rs / discover.rs
│   ├── dependency.rs / executor.rs / scoring.rs / publish.rs
│   ├── sources.rs / clarity_sync.rs / sync_adapter.rs
├── skill_sync.rs            # Vault → Skill 桥接
├── workflow/                # YAML 工作流引擎
│   ├── mod.rs / model.rs / parser.rs / validator.rs
│   ├── scheduler.rs / executor.rs / interpolate.rs / state.rs
├── knowledge_engine/        # README 摘要、关键词、模块探测
│   ├── mod.rs / readme.rs / module.rs / index.rs
│   ├── index_state.rs / llm.rs / fallback.rs
└── sync/                    # 仓库同步编排
    ├── mod.rs / orchestrator.rs / policy.rs / tasks.rs / tests.rs
```

#### 可靠层与基础能力

```text
src/
├── storage.rs               # StorageBackend + AppContext（依赖注入容器）
├── config.rs                # 配置结构体
├── i18n/                    # 国际化（en / zh_cn）
├── core/                    # 原子类型（Node / Edge / NodeType）
├── asyncgit.rs              # 异步 Git 通知通道
├── scan.rs                  # 仓库扫描入口
├── query.rs                 # 结构化查询表达式
├── health.rs / health/env_cache.rs
├── oplog_analytics.rs       # 操作日志与覆盖率分析
├── backup.rs                # Schema 迁移前自动快照
├── watch.rs                 # 目录监控
├── dependency_graph.rs      # 跨仓库依赖图
├── symbol_links.rs          # 符号链接（RE-EXPORT ONLY）
├── discovery_engine.rs      # 跨仓库发现
├── embedding.rs             # 向量嵌入封装
├── digest.rs                # 摘要/哈希工具
├── arxiv.rs                 # arXiv 元数据抓取
├── greptime.rs              # GreptimeDB 可选集成
├── syncthing_client.rs      # Syncthing 客户端
├── sync_protocol.rs         # 同步协议基础类型
├── clients.rs               # 通用客户端
├── daemon.rs                # 守护进程入口
└── test_utils.rs            # 测试辅助
```

### `crates/` — 12 个 Workspace Crate

```text
crates/
├── devbase-core-types            # Node / Edge / NodeType 核心类型
├── devbase-registry              # Registry 核心逻辑
├── devbase-embedding             # 本地文本嵌入（Candle + Ollama）
├── devbase-vault-wikilink        # WikiLink 解析器
├── devbase-vault-frontmatter     # Vault Frontmatter 解析
├── devbase-skill-runtime-parser  # Skill 运行时解析器
├── devbase-skill-runtime-types   # Skill 运行时类型
├── devbase-symbol-links          # 符号链接
├── devbase-sync-protocol         # 同步协议
├── devbase-syncthing-client      # Syncthing 客户端
├── devbase-workflow-model        # Workflow 数据模型
└── devbase-workflow-interpolate  # Workflow 变量插值
```

### Crate 依赖方向

```text
devbase-core-types （零内部耦合）
    ↓
{ devbase-registry, devbase-embedding, devbase-vault-*, devbase-skill-runtime-*,
  devbase-symbol-links, devbase-sync-protocol, devbase-syncthing-client,
  devbase-workflow-* }
    ↓
src/ 各模块（commands, tui, mcp, registry, search, vault, skill_runtime, workflow, ...）
```
