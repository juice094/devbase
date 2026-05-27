# 🗄️ devbase

> **World Model Compiler for Developer Workspaces**
>
> One engine for code context, knowledge memories, and agent reasoning.  
> Replacing fragmented repo managers, note-taking apps & AI context windows.

---

<p align="center">
  <img src="https://img.shields.io/badge/version-v0.20.1-blue" alt="Version">
  <img src="https://img.shields.io/badge/tests-494%2B%20passed-brightgreen" alt="Tests">
  <img src="https://img.shields.io/badge/clippy-0%20warnings-green" alt="Clippy">
  <img src="https://img.shields.io/badge/license-AGPL--3.0%20%2F%20Commercial-orange" alt="License">
  <img src="https://img.shields.io/badge/rust-1.95%2B-9cf" alt="Rust">
</p>

---

## 📋 项目简介

devbase 是开发者的**世界模型编译器**。它将代码库、笔记、工作流等原始数字资产编译为 AI 可推理的结构化情境——不是存储数据，是构建环境的心智模型。

| 你是谁 | devbase 为你做什么 |
|:---|:---|
| **人类开发者** | `devbase tui` 打开终端仪表盘，一眼看清 N 个仓库的 Git 状态，按 `s` 批量安全同步 |
| **AI Agent** | 通过 MCP 调用 `devkit_skill_run`，AI 能发现、执行、编排 Skill——不再重复造轮子 |
| **项目维护者** | `devbase skill discover .` 一键将项目封装为 Skill，让 AI 用户能够发现和调用 |

---

## 🎯 核心能力

### Human Layer — TUI 仪表盘

基于 [ratatui](https://github.com/ratatui/ratatui) 的终端界面，专为**多仓库场景**设计：

| 按键 | 功能 |
|:---|:---|
| `↑/↓` `PgUp/PgDn` | 导航列表（仓库 / Vault / Session） |
| `Tab` | 切换主视图（RepoList → VaultList → Session） |
| `/` | 跨仓库代码搜索（Tantivy / ripgrep） |
| `Enter` | 一键启动 gitui / lazygit |
| `s` / `S` | 预览 / 执行安全同步 |
| `k` / `w` | Skill 列表 / Workflow 列表 |
| `[:]` | 自然语言查询 Skills |
| `d` | 发现 Skill（自动封装当前项目） |

### AI Layer — 69 个 MCP Tools

基于 [Model Context Protocol](https://modelcontextprotocol.io) 标准化接口，stdio 本地进程通信。

| 域 | 工具数 | 代表能力 |
|:---|:---:|:---|
| 仓库管理 | 4 | `scan`, `health`, `sync`, `query_repos` |
| 代码分析 | 6 | `code_metrics`, `module_graph`, `call_graph`, `dead_code` |
| 知识检索 | 7 | `semantic_search`, `hybrid_search`, `cross_repo_search` |
| Skill 运行时 | 4 | `skill_list`, `skill_search`, `skill_run`, `skill_discover` |
| Workflow 编排 | 3 | `workflow_list`, `workflow_run`, `workflow_status` |
| 知识图谱 | 3 | `relation_store`, `relation_query`, `relation_delete` |
| Agent 记忆 | 4 | `session_recall`, `session_index`, `session_export`, `session_import` |
| ClaudeCode 集成 | 2 | `project_brief`, `impact_analysis` |
| Vault / 笔记 | 7 | `vault_search`, `vault_read`, `vault_write`, `vault_graph` |
| 可观测性 | 3 | `search_quality`, `index_health`, `oplog_query` |

> 完整 69 个 Tool 矩阵及示例查询见 [MCP 集成指南](docs/guides/mcp-integration-guide.md)。

### Storage & Reliability Layer — 生产级本地知识基础设施

| 组件 | 技术 | 生产级特性 |
|:---|:---|:---|
| 关系存储 | SQLite (WAL mode) | 并发安全、增量备份、Schema 迁移前自动快照 |
| 全文检索 | Tantivy | BM25 评分、索引健康检测、损坏自动重建 |
| 语义检索 | SQLite BLOB + `cosine_similarity` UDF | 外置 Embedding 存储、纯 SQL 向量比对、零 ML 运行时依赖 |
| AST 感知 | tree-sitter | Rust / Python / TS / Go 多语言符号提取 + 调用图构建 |
| 可观测性 | SQLite `oplog` + `HybridSearchMetrics` | 全操作审计追踪、混合检索质量指标、查询延迟回归测试 |

**可靠性红线**：所有对 Registry 的写入操作必须留下不可变审计痕迹（OpLog）；Schema 迁移前自动生成 `backup-YYYYMMDD-HHMMSS.db`。

---

## 📁 项目结构

```
devbase/
├── src/
│   ├── main.rs                 # CLI 入口
│   ├── tui/                    # 终端仪表盘 (ratatui)
│   ├── mcp/                    # MCP Server (69 tools, stdio)
│   ├── registry/               # 仓库注册、Git 状态、健康检查
│   ├── index/                  # Tantivy 全文 + SQLite 向量索引
│   ├── vault/                  # PARA 笔记系统、双向链接、BFS 图遍历
│   ├── skill/                  # Skill 发现、执行、评分、依赖拓扑
│   ├── workflow/               # YAML 编排引擎 (5 step 类型)
│   └── session/                # Agent 会话生命周期 + 向量记忆
├── docs/
│   ├── architecture/
│   └── guides/
├── scripts/
│   ├── install.ps1             # Windows 一键安装
│   ├── install.sh              # Linux/macOS 一键安装
│   └── devbase-claude.ps1      # Claude Code 一键启动器
└── README.md
```

---

## 🚀 快速开始

### 一键安装

```powershell
# Windows
irm https://raw.githubusercontent.com/juice094/devbase/main/scripts/install.ps1 | iex

# Linux / macOS
curl -fsSL https://raw.githubusercontent.com/juice094/devbase/main/scripts/install.sh | bash
```

### 预编译二进制

| 平台 | 下载 | 大小 |
|:---|:---|:---|
| Windows x86_64 | [`devbase-v0.20.1-windows-x64.zip`](https://github.com/juice094/devbase/releases/download/v0.20.1/devbase-v0.20.1-windows-x64.zip) | ~8.7 MB |
| Linux x86_64 | [`devbase-v0.20.1-linux-x64.tar.gz`](https://github.com/juice094/devbase/releases/download/v0.20.1/devbase-v0.20.1-linux-x64.tar.gz) | ~8.8 MB |

### 从源码

```bash
git clone https://github.com/juice094/devbase.git
cd devbase && cargo install --path .
```

### 基础工作流

```bash
# 1. 扫描并注册工作区
devbase scan . --register

# 2. 检查索引状态
devbase status --json

# 3. 启动 TUI 仪表盘
devbase tui

# 4. 启动 MCP Server（供 AI 助手调用）
devbase mcp
```

### AI 助手配置

**Claude Desktop**（`claude_desktop_config.json`）：
```json
{
  "mcpServers": {
    "devbase": {
      "command": "devbase",
      "args": ["mcp"]
    }
  }
}
```

**Kimi CLI**（`~/.kimi/mcp.json`）：
```json
{
  "mcpServers": {
    "devbase": {
      "command": "devbase",
      "args": ["mcp"]
    }
  }
}
```

---

## 🏗️ 架构说明

```
┌─────────────────────────────────────────────────────────────────┐
│  Interaction Layer  (人类与 AI 的接口)                          │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────────┐  │
│  │  TUI 仪表盘   │  │ MCP Server   │  │ Workflow Engine      │  │
│  │  (ratatui)    │  │ 69 Tools     │  │ YAML + 拓扑调度      │  │
│  └──────────────┘  └──────────────┘  └──────────────────────┘  │
├─────────────────────────────────────────────────────────────────┤
│  Compilation Layer  (World Model Compiler Core)                 │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────────┐  │
│  │  Perception  │  │  Knowledge   │  │  Policy / Action     │  │
│  │  · tree-sitter│  │  · Graph DB  │  │  · Sync Strategy     │  │
│  │  · Tantivy    │  │  · Vector UDF│  │  · Workflow Rules    │  │
│  │  · Git 状态   │  │  · Relation  │  │  · Health Guardrails │  │
│  └──────────────┘  └──────────────┘  └──────────────────────┘  │
├─────────────────────────────────────────────────────────────────┤
│  Reliability Layer  (生产级底线)                                │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────────┐  │
│  │  SQLite WAL  │  │  Index Health│  │  Observability       │  │
│  │  并发安全     │  │  · 损坏检测  │  │  · OpLog 审计        │  │
│  │  · 增量备份   │  │  · 自动重建  │  │  · 查询延迟指标        │  │
│  │  · 迁移回滚   │  │  · 性能基线  │  │  · 数据质量评分        │  │
│  └──────────────┘  └──────────────┘  └──────────────────────┘  │
├─────────────────────────────────────────────────────────────────┤
│  Source of Truth  (持久化真相源)                                │
│  Git 代码库 · Vault PARA 笔记 · 外部论文 · 二进制资源              │
└─────────────────────────────────────────────────────────────────┘
```

---

## 📊 功能深度

### 安全同步 (Safe Sync)

| 策略 | 行为 | 场景 |
|:---|:---|:---|
| **Mirror** | 仅 fetch，永不修改 | 参考仓库、第三方依赖 |
| **Conservative** | 仅 fast-forward，diverged 跳过 | 日常开发，安全第一 |
| **Rebase** | 自动 rebase 本地提交 | 个人分支，线性历史 |
| **Merge** | 自动 merge | 协作分支 |

同步前预检：dirty / diverged / protected 分支自动跳过并记录到 OpLog。

### Skill 全生命周期

```
discover → install → run → score → publish
    ↑_________________________________|
```

- **发现**：`devbase skill discover <path>` — 自动分析项目 CLI/API，生成 `SKILL.md`
- **执行**：Process-based，自动 interpreter 探测，timeout，stdout/stderr 捕获
- **评分**：Mind Market 算法自动计算 `success_rate` / `usage_count` / `rating`（0-5）
- **依赖**：Schema v15 `dependencies`，Kahn 拓扑排序 + DFS 环检测

### Workflow 引擎 (v0.8.0)

- 5 种 step 类型：`skill` / `subworkflow` / `parallel` / `condition` / `loop`
- 拓扑调度 + batch 并行执行
- 变量插值：`${inputs.x}` / `${steps.y.outputs.z}`
- 错误策略：Fail / Continue / Retry / Fallback

### L3-L4 知识模型 (v0.10.0)

- **L3 风险层** (`known_limits`)：记录 hard vetoes、已知缺陷、外部依赖风险
- **L4 元认知层** (`knowledge_meta`)：记录人类对 L1-L3 的纠正
- **运行时守卫**：Skill 执行前自动检查未解决 hard veto，警告注入 stderr

---

## 🔧 开发规范

```bash
# 快速开始
cargo build --release
cargo test --all-targets

# 代码规范
cargo clippy --all-targets -D warnings
cargo fmt --check

# 构建加速（可选）
# sccache 配置见 CONTRIBUTING.md — tree-sitter 重复编译从 20s → <1s
```

| 构建模式 | 命令 | 说明 |
|:---|:---|:---|
| 最小化 CLI | `cargo build --no-default-features` | 纯 CLI，无 TUI/MCP |
| 纯 TUI | `cargo build --features tui` | 含 TUI，无 MCP |
| 完整功能 | `cargo build --features mcp` | 含 MCP Server |

- **架构文档**: [`docs/architecture/overview.md`](docs/architecture/overview.md)
- **Agent 约定**: [`AGENTS.md`](AGENTS.md)
- **贡献指南**: [`CONTRIBUTING.md`](CONTRIBUTING.md)

---

## 📚 文档索引

| 文档 | 受众 | 用途 |
|:---|:---|:---|
| [`CONTRIBUTING.md`](CONTRIBUTING.md) | Contributors | 如何添加 MCP Tool / Skill、Schema 迁移规范 |
| [`AGENTS.md`](AGENTS.md) | AI agents | 环境指南、已知问题、耦合说明 |
| [`docs/architecture/overview.md`](docs/architecture/overview.md) | Developers | 架构总览 |
| [`docs/guides/mcp-integration-guide.md`](docs/guides/mcp-integration-guide.md) | Users | Claude Code / 5ire / Kimi CLI 集成 |
| [`CHANGELOG.md`](CHANGELOG.md) | Users | 版本历史与迁移说明 |

---

## 🔒 隐私与安全

**本地优先（Local-First）**：

- 代码不会离开本地机器 — Registry、索引、日志全部存储在用户目录的 SQLite 中
- MCP Server 仅通过 stdio 本地进程通信，不监听网络端口
- GitHub Token / LLM API Key 存储在用户配置目录的 `config.toml` 中，不会进入 git 仓库

```toml
# %LOCALAPPDATA%\devbase\config.toml (Windows)
# ~/.config/devbase/config.toml (Linux/macOS)
[github]
token = "<YOUR_GITHUB_PAT>"
```

---

## 📄 License

本软件采用 **双许可证 (Dual License)** 模式：

- **开源使用**: [GNU Affero General Public License v3.0 or later (AGPL-3.0+)](./LICENSE)
  - 适用于个人、学术、及遵守 AGPL-3.0 义务的开源项目
  - 核心约束：若将修改版部署为网络服务（SaaS、托管 MCP Server 等），必须向用户公开完整源代码

- **商业使用**: 如需在闭源产品、专有 SaaS 或无法遵守 AGPL-3.0 的场景中使用，可联系作者获取商业授权
  - 详见 [`LICENSE-COMMERCIAL.md`](./LICENSE-COMMERCIAL.md)
  - 联系方式: `juice094@protonmail.com`

---

<p align="center">
  <sub>Built with Rust · 494+ tests · 0 warnings · Local-First by Design</sub>
</p>
