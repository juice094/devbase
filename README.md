# devbase

[![Version](https://img.shields.io/badge/version-v0.14.0-blue)](https://github.com/juice094/devbase/releases)
[![Tests](https://img.shields.io/badge/tests-406%20passed-brightgreen)](./AGENTS.md)
[![Clippy](https://img.shields.io/badge/clippy-0%20warnings-green)](./AGENTS.md)
[![License](https://img.shields.io/badge/license-MIT-orange)](./LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.94%2B-9cf)](https://www.rust-lang.org)

**本地优先的 AI Skill 编排基础设施**

> 把 GitHub 项目变成 AI 能执行的 Skill。  
> One dashboard for humans. One skill registry for agents.

---

## 30 秒了解

devbase 将 GitHub 项目自动封装为**标准化、可发现、可组合的 Skill**，让弱 AI 子代理能够发现、调用、编排这些 Skill 完成复杂任务。

| 你是谁 | devbase 为你做什么 |
|:---|:---|
| **人类开发者** | `devbase tui` 打开终端仪表盘，一眼看清 N 个仓库的 Git 状态，按 `s` 批量安全同步 |
| **AI Agent** | 通过 MCP 调用 `devkit_skill_run`，AI 能发现、执行、编排 Skill——不再重复造轮子 |
| **项目维护者** | `devbase skill discover .` 一键将项目封装为 Skill，让 AI 用户能够发现和调用 |

```
┌─────────────────────────────────────────────────────────────┐
│                        devbase                              │
│              Bimodal Developer Workspace OS                 │
├─────────────────────────────┬───────────────────────────────┤
│       Human Layer           │         AI Layer              │
│  ┌─────────────────────┐    │    ┌─────────────────────┐    │
│  │   TUI Dashboard     │    │    │   MCP Server        │    │
│  │   终端交互仪表盘     │    │    │   37 Tools          │    │
│  │   • 多仓库健康总览   │    │    │   stdio only         │    │
│  │   • 跨仓库代码搜索   │    │    │                     │    │
│  │   • 一键启动 gitui   │    │    │   • devkit_scan     │    │
│  │   • Skill / Workflow │    │    │   • devkit_skill_run│    │
│  └─────────────────────┘    │    │   • devkit_hybrid_search│  │
│                             │    └─────────────────────┘    │
├─────────────────────────────┴───────────────────────────────┤
│                      Data Layer                             │
│   Filesystem (Source of Truth) │ SQLite │ Tantivy (Search)   │
└─────────────────────────────────────────────────────────────┘
```

---

## 安装

**一键安装**

```powershell
# Windows
irm https://raw.githubusercontent.com/juice094/devbase/main/scripts/install.ps1 | iex

# Linux / macOS
curl -fsSL https://raw.githubusercontent.com/juice094/devbase/main/scripts/install.sh | bash
```

**从源码**

```bash
git clone https://github.com/juice094/devbase.git
cd devbase && cargo install --path .
```

---

## 核心能力

### Human Layer — TUI 仪表盘

基于 [ratatui](https://github.com/ratatui/ratatui) 的终端界面，专为**多仓库场景**设计：

| 按键 | 功能 |
|:---|:---|
| `↑/↓` `PgUp/PgDn` | 导航仓库列表 |
| `/` | 跨仓库代码搜索（Tantivy / ripgrep） |
| `Enter` | 一键启动 gitui / lazygit |
| `s` / `S` | 预览 / 执行安全同步 |
| `k` / `w` | Skill 列表 / Workflow 列表 |
| `[:]` | 自然语言查询 Skills |
| `d` | 发现 Skill（自动封装当前项目） |
| `h` / `?` | 快捷键帮助 |

**面板布局**：左侧 35% 仓库列表（状态图标 ● dirty ◆ diverged ▼ behind ✓ 正常），右侧 65% 三标签页详情（Overview / Health / Insights）。

### AI Layer — 38 个 MCP Tools

基于 [Model Context Protocol](https://modelcontextprotocol.io) 标准化接口，stdio 本地进程通信。

| 域 | Tools | 代表能力 |
|:---|:---|:---|
| 仓库管理 | `scan`, `health`, `sync`, `query_repos` | 批量管理 + 安全同步策略 |
| 代码分析 | `code_metrics`, `module_graph`, `code_symbols`, `call_graph`, `dependency_graph`, `dead_code` | AST 符号 + 调用图 + 死代码检测 |
| 知识检索 | `semantic_search`, `hybrid_search`, `cross_repo_search`, `related_symbols`, `knowledge_report` | 向量语义 + RRF 混合检索 |
| Skill 运行时 | `skill_list`, `skill_search`, `skill_run`, `skill_top` | 发现 / 搜索 / 执行 / 评分 |
| Workflow 编排 | `workflow_list`, `workflow_run` | YAML 多步骤自动化 |
| Vault / 其他 | `vault_search`, `vault_read`, `vault_write`, `arxiv_fetch`, ... | PARA 笔记 + 论文抓取 |

> 完整 Tool 矩阵见下文 [MCP Tool 矩阵](#mcp-tool-矩阵)。

### Data Layer — 本地优先知识库

| 组件 | 技术 | 说明 |
|:---|:---|:---|
| 索引 | SQLite + Tantivy | 仓库元数据 + 全文检索 |
| 语义 | SQLite BLOB (768-dim) | 外置 Embedding 存储协议，不绑定特定模型 |
| AST | tree-sitter | Rust / Python / TS / Go 多语言符号提取 |
| 审计 | SQLite `oplog` | 所有 `scan`/`sync`/`health` 自动记录，schema 迁移前自动快照 |

---

## 快速开始

```bash
# 1. 扫描并注册工作区
devbase scan . --register

# 2. 启动 TUI 仪表盘
devbase tui

# 3. 启动 MCP Server（供 AI 助手调用）
devbase mcp
```

**Claude Desktop 配置**（`claude_desktop_config.json`）：

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

**Kimi CLI 配置**（`~/.kimi/mcp.json`）：

```json
{
  "mcpServers": {
    "devbase": {
      "command": "devbase",
      "args": ["mcp"],
      "env": {
        "DEVBASE_MCP_ENABLE_DESTRUCTIVE": "1",
        "DEVBASE_MCP_TOOL_TIERS": "stable,beta"
      }
    }
  }
}
```

---

## 功能深度

### 安全同步 (Safe Sync)

不是粗暴的 `git pull --all`，而是分级策略：

| 策略 | 行为 | 场景 | 颜色 |
|:---|:---|:---|:---:|
| **Mirror** | 仅 fetch，永不修改 | 参考仓库、第三方依赖 | 🔵 |
| **Conservative** | 仅 fast-forward，diverged 跳过 | 日常开发，安全第一 | 🟡 |
| **Rebase** | 自动 rebase 本地提交 | 个人分支，线性历史 | 🟢 |
| **Merge** | 自动 merge | 协作分支 | 🟣 |

同步前预检：dirty / diverged / protected 分支自动跳过并记录到 OpLog。

### Skill 全生命周期

```
discover → install → run → score → publish
    ↑_________________________________|
```

- **发现**：`devbase skill discover <path>` — 自动分析项目 CLI/API，生成 `SKILL.md` + entry_script（支持 Rust/Node/Python/Go/Docker/Generic）
- **执行**：Process-based，自动 interpreter 探测，timeout，stdout/stderr 捕获
- **评分**：Mind Market 算法自动计算 `success_rate` / `usage_count` / `rating`（0-5）
- **依赖**：Schema v15 `dependencies`，Kahn 拓扑排序 + DFS 环检测

### Workflow 引擎 (v0.8.0)

YAML 编排多步骤自动化：

- 5 种 step 类型：`skill` / `subworkflow` / `parallel` / `condition` / `loop`
- 拓扑调度 + batch 并行执行
- 变量插值：`${inputs.x}` / `${steps.y.outputs.z}`
- 错误策略：Fail / Continue / Retry / Fallback

### 自然语言查询 (v0.8.1)

TUI `[:]` 触发 embedding 语义搜索，失败自动降级为文本搜索。AI 可以说：

> "show dirty rust repos with more than 100 stars"

### L3-L4 知识模型 (v0.10.0)

系统具备**自我边界意识**和**认知纠错能力**：

- **L3 风险层 (`known_limits`)**: 记录 hard vetoes、已知缺陷、外部依赖风险
  - `devbase limit list` 查看当前系统约束
  - `devbase limit seed` 从 AGENTS.md 自动填充 hard vetoes
- **L4 元认知层 (`knowledge_meta`)**: 记录人类对 L1-L3 的纠正
  - `devbase limit resolve <id> --reason "..."` 自动创建 L4 纠正记录
- **运行时守卫**: Skill 执行前自动检查未解决 hard veto，警告注入 stderr

---

## MCP Tool 矩阵

| Tool | 功能 | 示例查询 |
|:---|:---|:---|
| `devkit_scan` | 扫描并注册工作区 | "扫描 ~/projects" |
| `devkit_health` | 健康检查 | "哪些项目需要同步？" |
| `devkit_sync` | 批量同步（dry-run 默认） | "预览同步结果" |
| `devkit_query_repos` | 结构化查询 | "列出所有 dirty 的 Rust 项目" |
| `devkit_code_metrics` | 代码统计 | "我最大的项目是什么？" |
| `devkit_module_graph` | 模块结构 | "有哪些二进制目标？" |
| `devkit_natural_language_query` | 自然语言查询 | "dirty rust repos with >100 stars" |
| `devkit_index` | 索引仓库摘要 | "为所有仓库生成索引" |
| `devkit_query` | 知识库搜索 | "搜索 sync policy" |
| `devkit_note` | 添加笔记 | "给 devbase 添加笔记" |
| `devkit_digest` | 知识日报 | "今天的知识日报" |
| `devkit_github_info` | GitHub 元数据 | "devbase 多少 stars？" |
| `devkit_paper_index` | 索引 PDF 论文 | "索引 ~/papers" |
| `devkit_experiment_log` | 记录实验 | "记录这次实验配置" |
| `devkit_vault_search` | 搜索 Vault 笔记 | "搜索 API 设计笔记" |
| `devkit_vault_read` | 读取 Vault 笔记 | "读取 01-Projects/devbase.md" |
| `devkit_vault_write` | 创建/更新 Vault 笔记 | "新建重构笔记" |
| `devkit_vault_backlinks` | 反向链接 | "哪些笔记链接到 devbase？" |
| `devkit_project_context` | 统一项目上下文 | "devbase 的全景视图" |
| `devkit_code_symbols` | 代码语义索引 | "`build_server` 在哪？" |
| `devkit_call_graph` | 调用关系分析 | "谁调用了 `register_tool`？" |
| `devkit_dependency_graph` | 跨仓库依赖图 | "改 `shared-lib` 影响哪些？" |
| `devkit_dead_code` | 死代码检测 | "哪些函数没被调用过？" |
| `devkit_semantic_search` | 向量语义搜索 | "搜索错误处理相关函数" |
| `devkit_embedding_store` | Embedding 存储 | "将向量存入 devbase" |
| `devkit_embedding_search` | 向量搜索 | "用 query 向量搜索符号" |
| `devkit_arxiv_fetch` | arXiv 论文抓取 | "获取 arXiv 2401.12345" |
| `devkit_hybrid_search` | 混合检索（推荐） | "自动融合向量+关键词" |
| `devkit_cross_repo_search` | 跨仓库语义搜索 | "所有 Rust CLI 中搜配置解析" |
| `devkit_knowledge_report` | 知识覆盖报告 | "索引覆盖度如何？" |
| `devkit_related_symbols` | 概念关联搜索 | "与 `authenticate` 相似的函数" |
| `devkit_skill_list` | 列出 Skills | "有哪些内置 skill？" |
| `devkit_skill_search` | 搜索 Skills | "查找代码审计相关 skill" |
| `devkit_skill_run` | 执行 Skill | "运行 embed-repo skill" |
| `devkit_workflow_list` | 列出工作流 | "有哪些工作流？" |
| `devkit_workflow_run` | 执行工作流 | "运行 deploy-staging" |
| `devkit_skill_top` | Top 评分 Skills | "评分最高的 skill？" |
| `devkit_known_limit_store` | 记录 known limit | "记录系统约束" |
| `devkit_known_limit_list` | 列出 known limits | "查看当前风险" |

### AI 助手集成

- [Claude Code 集成](docs/guides/mcp-integration-guide.md)
- [5ire 集成](docs/guides/mcp-5ire-integration.md)

---

## 路线图

| 版本 | 状态 | 核心交付 |
|:---|:---:|:---|
| v0.3.0 | ✅ 已发布 | 产品化闭环：34 MCP tools + TUI + 安全同步 |
| v0.4.0 | ✅ 已发布 | Skill 自动封装 + 统一实体模型 Schema v16 |
| v0.5.0 | ✅ 已发布 | Workflow 引擎：YAML 编排 + 5 step 类型 |
| v0.6.0 | ✅ 已发布 | Mind Market 评分：success_rate / usage_count / rating |
| v0.7.0 | ✅ 已发布 | NLQ 自然语言查询 + 智能同步建议 |
| v0.8.0 | ✅ 已发布 | Workflow 子类型：Subworkflow / Parallel / Condition / Loop |
| v0.9.0 | ✅ 已发布 | Loop Step 硬化 + 发布闭环 |
| **v0.10.0** | **✅ 已发布** | **L3-L4 知识模型 + 工程健康维护（main.rs 拆分 / StorageBackend / feature flags）** |
| **v0.11.0** | **✅ 已发布** | **AppContext Pool 化 + MCP 测试隔离 + CI 多线程** |
| **v0.11.1** | **✅ 已发布** | **Flat ID 命名空间 + entities-first 写入反转** |
| **v0.11.2** | **✅ 已发布** | **读路径全量迁移：所有 SELECT 切到 `entities`** |
| **v0.11.3** | **✅ 已发布** | **`repos` 表删除，`entities` 成为唯一数据源（Phase 1 完成）** |
| **v0.12.0-alpha** | **✅ 已发布** | **Phase 2 完成 (Stage A-E): entities 统一重构 + `.devbase-ignore` + managed-gate fail-safe 同步** |
| **v0.13.0** | **✅ 已发布** | **Registry God Object 拆解：10 子模块提取为 free function；WorkspaceRegistry 退化为纯 facade** |
| **v0.14.0** | **✅ 已发布** | **Workspace 拆分：6 个零耦合 crate 提取；MCP trait 化：`mcp/tools/repo.rs` `crate::` 引用 68→41** |
| **v0.15.0** | **🚧 进行中** | **分发就绪：第二批 crate 提取（registry/health, metrics, workspace...）；MCP `crate::` 引用 <30** |

---

## 为什么 devbase？

### 不是替代，是连接

| 工具 | 定位 | devbase 的角色 |
|:---|:---|:---|
| **lazygit** | 单仓库 TUI | **多仓库入口** — 先告诉你哪些仓库需要关注，再按 `Enter` 进入 |
| **5ire / Claude Code** | AI 助手 | **代码库知识源** — 让 AI 拥有本地工作区的结构化上下文 |
| **GitHub Desktop** | GUI Git 客户端 | **TUI 替代** — 轻量 30 倍，SSH 可用，支持批量操作 |

### AI 无法识别你的 GUI

你的 IDE、文件管理器、甚至 lazygit 的界面对 AI 都是不可见的黑箱。devbase 通过 MCP Server 将本地代码库的状态、结构、健康度翻译成 AI 能理解的结构化数据——这是 AI 介入本地开发流程的**基础设施**。

---

## 隐私与安全

**本地优先（Local-First）**：

- 代码不会离开本地机器 — Registry、索引、日志全部存储在用户目录的 SQLite 中
- MCP Server 仅通过 stdio 本地进程通信，不监听网络端口
- GitHub Token / LLM API Key 存储在用户配置目录的 `config.toml` 中，不会进入 git 仓库
- `.gitignore` 已覆盖 `*.db`、`.devbase/`、`*.log`、`.env*` 等敏感文件

```toml
# %LOCALAPPDATA%\devbase\config.toml (Windows)
# ~/.config/devbase/config.toml (Linux/macOS)
[github]
token = "<YOUR_GITHUB_PAT>"
```

---

## 开发者与贡献

> devbase 当前为单人维护项目（Bus Factor = 1），欢迎任何形式的贡献。

- **快速开始**: `cargo build --release` → `cargo test --all-targets`
- **代码规范**: `cargo clippy --all-targets -D warnings` + `cargo fmt --check`
- **架构文档**: [`ARCHITECTURE.md`](ARCHITECTURE.md)
- **Agent 约定**: [`AGENTS.md`](AGENTS.md)
- **贡献指南**: [`CONTRIBUTING.md`](CONTRIBUTING.md) — 如何添加 MCP Tool / Skill、Schema 迁移规范

---

## 许可证

[MIT](./LICENSE)
