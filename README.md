# devbase

**Bimodal Developer Workspace OS** — 双模态开发者工作区操作系统

> AI 无法识别你的 GUI，devbase 是它的眼镜。  
> One dashboard for humans. One API for agents.

devbase 是一个**本地优先的双模态工作区操作系统**：它既是为人类开发者设计的**多仓库 TUI 仪表盘**，也是为 AI Agent 提供的**结构化代码库知识入口**。在 AI 无法识别你的 GUI、无法感知你的本地文件系统的今天，devbase 是 AI 理解你本地代码库的**唯一入口**。

---

## 一句话介绍

| 你是谁 | devbase 为你做什么 |
|--------|------------------|
| **人类开发者** | `devbase tui` 打开终端仪表盘，一眼看清 50 个仓库的 Git 状态，按 `s` 批量安全同步 |
| **AI Agent** | 通过 MCP 调用 `devkit_vault_search`，AI 能搜索/读写你的笔记和仓库——不再是黑箱 |

---

## 双模态架构 (Bimodal Architecture)

```
┌─────────────────────────────────────────────────────────────┐
│                        devbase                              │
│              Bimodal Developer Workspace OS                 │
├─────────────────────────────┬───────────────────────────────┤
│       Human Layer           │         AI Layer              │
│     (人类交互层)             │       (智能体接口层)           │
│                             │                               │
│  ┌─────────────────────┐    │    ┌─────────────────────┐    │
│  │   TUI Dashboard     │    │    │   MCP Server        │    │
│  │   终端交互仪表盘     │    │    │   34 Tools          │    │
│  │                     │    │    │   stdio only         │    │
│  │ • 多仓库健康总览     │    │    │                     │    │
│  │ • 跨仓库代码搜索 /   │    │    │ • devkit_scan       │    │
│  │ • Stars 趋势图       │    │    │ • devkit_health     │    │
│  │ • AI 洞察面板        │    │    │ • devkit_sync       │    │
│  │ • 智能同步建议       │    │    │ • devkit_query_repos│    │
│  │ • gitui/lazygit 启动 │    │    │ • devkit_code_metrics│   │
│  │                      │    │    │ • devkit_module_graph│   │
│  └─────────────────────┘    │    │ • devkit_natural... │    │
│                             │    └─────────────────────┘    │
│  一眼看遍所有仓库状态        │    让 AI 拥有本地代码库的       │
│  批量操作 + 深度集成         │    结构化世界观                 │
├─────────────────────────────┴───────────────────────────────┤
│                      Data Layer                             │
│                    (数据与引擎层)                            │
│                                                             │
│   Filesystem (Source of Truth) │ SQLite (Lightweight Index) │ Tantivy (Search)
   ─────────────────────────────────────────────────────────────
   vault/  • repos.toml  • assets/      registry.db        search-index/│
│                                                             │
│   • Git 仓库 + 标记目录的自动发现与持久化                     │
│   • Vault 笔记系统（PARA 结构，Obsidian 兼容）                │
│   • Stars 历史缓存 (趋势图数据源)                            │
│   • 代码统计 (tokei) + 模块图 (cargo metadata)               │
│   • 多语言 AST 符号提取 + Call Graph + 依赖图               │
│   • 外置大脑 Embedding 存储协议 (SQLite BLOB)               │
│   • 安全同步策略 (Mirror / Conservative / Rebase / Merge)    │
│   • 操作审计日志与 schema 迁移快照                           │
└─────────────────────────────────────────────────────────────┘
```

---

## TUI 功能一览 (Human Mode)

基于 [ratatui](https://github.com/ratatui/ratatui) 的终端交互界面，专为**多仓库场景**设计：

| 按键 | 功能 |
|:---|:---|
| `↑/↓` | 在仓库列表中导航 |
| `PgUp/PgDn` | 快速翻页 |
| `Home/End` | 跳到列表顶部/底部 |
| `/` | **跨仓库代码搜索** — Tantivy 仓库语义搜索 / ripgrep 代码搜索（`Ctrl+R` 切换模式） |
| `Enter` | 启动 **gitui** / **lazygit** 进行单仓库深度操作（自动检测并挂起/恢复终端） |
| `s` | 生成 Safe Sync Preview（dry-run 预览） |
| `S` | 执行安全同步 |
| `t` | 为选中仓库打标签 |
| `o` | 切换排序模式：Status ↔ Stars |
| `Tab` / `Shift+Tab` | 切换详情面板标签页：Overview ↔ Health ↔ Insights |
| `r` | 刷新仓库列表 |
| `k` | 打开 **Skill 列表** — 浏览、搜索、执行 devbase Skills |
| `h` / `?` / `F1` | 显示快捷键帮助弹窗 |
| `q` / `Esc` | 退出 / 关闭弹窗 |

### TUI 面板

- **左侧 35%**：仓库列表，状态图标（⏳ 加载中 / ● dirty / ◆ diverged / ▼ behind / ▲ ahead / ✓ 正常 / ○ 无远程）
- **右侧 65%**：三标签页详情面板
  - **Overview**：Git 状态、HEAD、SyncPolicy、标签、语言、upstream、last sync
  - **Health**：完整健康报告（dirty / detached / diverged / ahead / behind）
  - **Insights**：AI 智能洞察 + Stars Trend Sparkline（最近 30 次 fetch 的历史）

---

## MCP Tool 矩阵 (AI Mode)

基于 [Model Context Protocol](https://modelcontextprotocol.io) 的标准化接口。当前支持 **stdio**（本地进程通信）；**SSE**（HTTP 流式传输）正在开发中。

| Tool | 功能 | 示例查询 |
|------|------|---------|
| `devkit_scan` | 扫描目录并注册工作区 | "扫描 ~/projects" |
| `devkit_health` | 健康检查（所有仓库状态） | "我本地有哪些项目需要同步？" |
| `devkit_sync` | 批量同步（dry-run 默认） | "预览同步这些仓库会发生什么" |
| `devkit_query_repos` | 结构化查询（语言/标签/状态） | "列出所有 dirty 的 Rust 项目" |
| `devkit_code_metrics` | 代码统计（行数、文件数、语言） | "我最大的项目是什么？" |
| `devkit_module_graph` | Rust 模块/目标结构 | "devbase 有哪些二进制目标？" |
| `devkit_natural_language_query` | **自然语言查询** | "show dirty rust repos with more than 100 stars" |
| `devkit_index` | 索引仓库摘要和模块结构 | "为所有仓库生成知识索引" |
| `devkit_query` | 知识库搜索（tantivy） | "搜索关于 sync policy 的知识" |
| `devkit_note` | 为仓库添加笔记 | "给 devbase 项目添加一条笔记" |
| `devkit_digest` | 生成每日知识简报 | "生成今天的知识日报" |
| `devkit_github_info` | 查询 GitHub 元数据 | "devbase 项目有多少 stars？" |
| `devkit_paper_index` | 索引 PDF 论文 | "索引 ~/papers 目录" |
| `devkit_experiment_log` | 记录实验运行 | "记录这次实验的配置" |
| `devkit_vault_search` | 搜索 Vault 笔记 | "搜索关于 API 设计的笔记" |
| `devkit_vault_read` | 读取 Vault 笔记内容 | "读取 01-Projects/devbase.md" |
| `devkit_vault_write` | 创建/更新 Vault 笔记 | "新建一篇关于重构的笔记" |
| `devkit_vault_backlinks` | 查询笔记反向链接 | "哪些笔记链接到了 devbase？" |
| `devkit_project_context` | **统一项目上下文** | "获取 devbase 项目的 repo + vault + assets 全景" |
| `devkit_code_symbols` | **代码语义索引** | "函数 `build_server` 在哪个文件第几行？" |
| `devkit_call_graph` | **调用关系分析** | "谁调用了 `register_tool`？" |
| `devkit_dependency_graph` | **跨仓库依赖图** | "改了 `shared-lib` 会影响哪些仓库？" |
| `devkit_dead_code` | **死代码检测** | "这个仓库有哪些函数从没被调用过？" |
| `devkit_semantic_search` | **向量语义搜索** | "搜索与错误处理相关的函数（传入 query_embedding）" |
| `devkit_embedding_store` | **Embedding 存储** | "将外部生成的向量存入 devbase" |
| `devkit_embedding_search` | **向量搜索** | "用外部 query 向量搜索相似符号" |
| `devkit_arxiv_fetch` | **arXiv 论文抓取** | "获取 arXiv 2401.12345 的元数据" |
| `devkit_hybrid_search` | **混合检索（推荐）** | "搜索错误处理相关函数，自动融合向量+关键词" |
| `devkit_cross_repo_search` | **跨仓库语义搜索** | "在所有 Rust CLI 项目中搜索配置解析逻辑" |
| `devkit_knowledge_report` | **知识覆盖报告** | "workspace 的索引覆盖度如何？" |
| `devkit_related_symbols` | **概念关联搜索** | "与 `authenticate` 签名相似的其他函数" |
| `devkit_skill_list` | **列出可用 Skills** | "devbase 有哪些内置 skill？" |
| `devkit_skill_search` | **搜索 Skills**（文本 + 语义） | "查找与代码审计相关的 skill" |
| `devkit_skill_run` | **执行 Skill** | "运行 embed-repo skill 为 devbase 生成 embeddings" |

### AI 助手集成指南

- [Claude Code 集成](docs/guides/mcp-integration-guide.md)
- [5ire 集成](docs/guides/mcp-5ire-integration.md)

---

## 为什么 devbase？

### 不是替代，是连接

| 工具 | 定位 | devbase 的角色 |
|------|------|---------------|
| **lazygit** | 单仓库 TUI，人类逐仓操作 | devbase 是**多仓库入口**——在 lazygit 之前，先告诉你「哪些仓库需要关注」，按 `Enter` 一键进入 |
| **gitui** | 轻量 Rust TUI | devbase 的**深度操作伙伴**——批量管理后，单仓库精细操作交给 gitui |
| **5ire / Claude Code** | AI 助手，对话式编程 | devbase 是**代码库知识源**——让 AI 拥有本地工作区的结构化上下文，不再「盲人摸象」 |
| **GitHub Desktop** | GUI Git 客户端 | devbase 是**TUI 替代方案**——轻量 30 倍，SSH 可用，支持批量操作 |
| **GitHub / GitLab** | 远程代码托管 | devbase 是**本地镜像管家**——批量管理远程同步，dirty/diverged 自动保护 |

### AI 无法识别你的 GUI

你的 IDE、文件管理器、甚至 lazygit 的界面，对 AI 来说都是不可见的黑箱。devbase 通过 MCP Server 将本地代码库的状态、结构、健康度翻译成 AI 能理解的结构化数据——这是 AI 介入本地开发流程的**基础设施**。

---

## 安全同步策略 (Safe Sync)

devbase 的同步不是粗暴的 `git pull --all`，而是分级的安全策略：

| 策略 | 行为 | 适用场景 | TUI 颜色 |
|------|------|---------|:-------:|
| **Mirror** | 仅 fetch，永不修改本地分支 | 参考仓库、第三方依赖 | 🔵 Blue |
| **Conservative** | 仅 fast-forward，diverged 自动跳过 | 日常开发，安全第一 | 🟡 Yellow |
| **Rebase** | 自动 rebase 本地提交到远程分支 | 个人分支，保持线性历史 | 🟢 Green |
| **Merge** | 自动 merge 远程变更 | 协作分支，接受合并历史 | 🟣 Magenta |

同步前自动预检：dirty 工作区、diverged 分支、protected 分支均会被跳过并记录到 OpLog，绝不擅自破坏你的工作成果。

**智能同步建议**：在 Sync Preview 弹窗中，每个仓库下方会显示 AI 生成的同步建议，例如：
- `→ Safe to fast-forward 3 commit(s)`
- `→ Working tree dirty — commit or stash before sync`
- `→ Diverged (2 ahead, 3 behind) — switch to Rebase/Merge policy`

---

## 功能清单

- **工作区扫描与注册**：自动发现 Git 仓库 **以及** `SOUL.md` / `MEMORY.md` / `.devbase` 标记的非 Git 工作区，持久化到 SQLite
- **GitHub Stars 追踪**：显示、缓存、TTL 刷新、历史趋势图
- **代码统计**：集成 `tokei`，统计代码行数、文件数、语言分布（扫描时自动计算）
- **Rust 模块图**：通过 `cargo metadata` 提取 bin/lib/test 目标
- **健康检查**：追踪 Git 仓库的 dirty / ahead / behind，以及非 Git 工作区的 blake3 哈希快照变更检测
- **知识日报**：基于仓库健康状态和摘要生成每日简报
- **TUI 交互界面**：
  - 多仓库健康总览、标签聚类排序、Stars 排序
  - 跨仓库代码搜索 `/`
  - AI Insights 面板
  - Stars Trend sparkline
  - 一键启动 gitui/lazygit
  - 智能同步建议
- **MCP Server**：34 个 tools（含 5 个 vault tools + 8 个代码分析工具 + 4 个 embedding/搜索工具 + 3 个 Skill Runtime tools + 1 个报告工具 + 1 个 arXiv 工具），stdio / SSE 双传输
- **代码语义索引**：tree-sitter AST 解析，支持 Rust / Python / JavaScript / TypeScript / Go，提取函数/结构体/枚举/trait/impl/class/接口 定义到 SQLite
- **调用关系分析**：遍历 AST 提取 `call_expression` / `macro_invocation`，构建 intra-repo call graph
- **跨仓库依赖图**：解析 Cargo.toml / package.json / go.mod / pyproject.toml / requirements.txt / CMakeLists.txt，构建 repo 间依赖边
- **死代码检测**：基于 call graph 的 `NOT EXISTS` 查询，识别无 incoming edges 的函数
- **自然语言查询**：AI 可通过自然语言查询仓库（"dirty rust repos with more than 100 stars"）
- **外置大脑 Embedding 架构**：devbase 不内置 embedding 生成引擎（由外部 Skill/MCP Server 提供），只负责向量存储协议（SQLite BLOB）和相似度检索接口
- **Skill Runtime**：安装、发现、执行 AI Skills 的完整生命周期
  - 内置 `embed-repo`、`search-workspace`、`knowledge-report`
  - **语义搜索**：`devbase skill search "audit code" --semantic`（基于 sentence-transformers 384-dim 向量）
  - **执行引擎**：Process-based，支持 Python/Bash/PowerShell/Node.js/二进制，自动 interpreter 解析，timeout，stdout/stderr 捕获
  - **发布**：`devbase skill publish ./my-skill/` — 自动校验 + git tag + push to remote
  - **同步**：`devbase skill sync --target clarity` — 导出为 Clarity plan JSON
  - **MCP 暴露**：`devkit_skill_list` / `devkit_skill_search` / `devkit_skill_run` 共 3 个 tools
- **arXiv 集成**：通过 MCP 抓取论文元数据（标题/作者/摘要/分类）
- **性能基准测试**：Criterion 覆盖索引流水线、向量相似度、AST 提取、CMake 解析
- **Registry 备份**：`export`/`import`/`backups`/`clean`，schema 迁移前自动快照
- **操作日志 (OpLog)**：`scan`/`sync`/`health` 自动记录，可追溯审计
- **i18n**：中文 / 英文双语支持
- **数据分级**：`public` / `cooperative` / `private` 三级，控制同步边界

---

## 快速开始

### 安装

**一键安装（推荐）**

```powershell
# Windows
irm https://raw.githubusercontent.com/juice094/devbase/main/scripts/install.ps1 | iex

# Linux / macOS
curl -fsSL https://raw.githubusercontent.com/juice094/devbase/main/scripts/install.sh | bash
```

**从源码构建**

```bash
cargo install --path .
# 或未来从 crates.io
# cargo install devbase
```

### 初始化

```bash
# 扫描并注册当前目录下的所有工作区（Git + 非 Git）
devbase scan . --register

# 查看工作区健康状态
devbase health --detail

# 批量同步全部仓库（fetch-only）
devbase sync

# 生成知识日报
devbase digest
```

### TUI

```bash
# 启动 TUI 仪表盘
devbase tui
```

常用按键：
- `↑/↓` 导航仓库
- `/` 跨仓库搜索代码
- `Enter` 启动 gitui/lazygit（如果已安装）
- `s` 预览同步
- `S` 执行同步
- `t` 打标签
- `o` 切换排序（Status ↔ Stars）
- `r` 刷新
- `q` 退出

### MCP Server

```bash
# stdio 模式（本地 AI 助手，如 Claude Desktop / 5ire / Cursor）
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

**Cursor 配置**（`~/.cursor/mcp.json`）：同上。

> 当前 MCP 仅支持 stdio 传输。SSE 远程模式计划在未来版本中提供。

### 其他 CLI 命令

```bash
# 查看操作日志
devbase oplog --limit 20

# 导出 registry 备份
devbase registry export --format json

# 导入备份
devbase registry import backup-20260101.db
```

---

## 非 Git 工作区支持

devbase 不仅管理 Git 仓库，也支持任意被标记的目录：

| 标记文件 | 工作区类型 | 变更检测 |
|---------|-----------|---------|
| `.git/` | `git` | `git2` ahead/behind/dirty |
| `SOUL.md` / `.claude/` | `openclaw` | blake3 哈希快照 |
| `MEMORY.md` / `.devbase` | `generic` | blake3 哈希快照 |

---

## 竞品分析

devbase 不是 Git 客户端，不是 AI 编码助手，而是**连接两者的基础设施**。

| 竞品 | 赛道 | 关系 |
|------|------|------|
| lazygit | 单仓库 Git TUI | **互补** — devbase 是多仓库入口，lazygit 是单仓库深度操作 |
| gitui | 单仓库 Git TUI | **互补** — 同上 |
| GitHub Desktop | Git GUI | **无关** — 不同用户群体，devbase 是 TUI 方案 |
| 5ire | AI 助手 + 知识库 | **竞合** — 5ire 是 MCP Client，devbase 是 MCP Server |
| Claude Code | AI 编码助手 | **上下游** — Claude 调用 devbase 获取本地上下文 |

完整的 36 项目竞品分析见 [docs/research/competitive-analysis.md](docs/research/competitive-analysis.md)。

---

## 依赖

- Rust 2024 edition
- SQLite (bundled via `rusqlite`)
- `tokei` (代码统计)
- `tree-sitter` + `tree-sitter-rust` + `tree-sitter-python` + `tree-sitter-typescript` + `tree-sitter-go` (多语言 AST 解析)
- `ripgrep`（可选，用于跨仓库搜索）
- `criterion`（性能基准测试，dev-only）
- 可选：`clarity-core`（用于 LLM 驱动的仓库摘要生成）

---

## 隐私与安全

devbase 遵循**本地优先（Local-First）**原则：

- **你的代码不会离开本地机器**。Registry、索引、日志全部存储在用户目录下的 SQLite 中
- **MCP Server** 仅通过 stdio 本地进程通信，不监听任何网络端口
- **GitHub Token / LLM API Key** 存储在本地 `config.toml` 中，该文件位于用户配置目录，不会进入 git 仓库
- `.gitignore` 已覆盖 `*.db`、`.devbase/`、`*.log`、`.env*` 等敏感文件，防止意外提交

### 凭证管理最佳实践

```toml
# ~/.config/devbase/config.toml (Linux/macOS)
# %LOCALAPPDATA%\devbase\config.toml (Windows)
[github]
token = "<YOUR_GITHUB_PAT>"  #  NEVER 将此文件提交到版本控制

[llm]
# api_key = "<YOUR_LLM_API_KEY>"
```

## 配置

配置文件位于：
- Windows: `%LOCALAPPDATA%\devbase\config.toml`
- Linux/macOS: `~/.config/devbase/config.toml`

首次运行会自动生成带注释的默认模板。

```toml
[github]
# token = "<YOUR_GITHUB_PAT>"  # 提高 GitHub API 限流阈值

[sync]
concurrency = 8     # 批量同步并发数
timeout_seconds = 60

cache.ttl_seconds = 3600  # Stars 缓存 TTL
```

---

## 开发者与贡献

> devbase 是单人维护项目（Bus Factor = 1），欢迎任何形式的贡献——代码、文档、Issue、想法均可。

- **快速开始**: `cargo build --release` → `cargo test --all-targets`
- **代码规范**: `cargo clippy --all-targets -D warnings` + `cargo fmt --check`
- **架构文档**: [`ARCHITECTURE.md`](ARCHITECTURE.md) — 三层架构、技术决策记录
- **Agent 约定**: [`AGENTS.md`](AGENTS.md) — 安全原则、上下文机制、Schema 迁移规范
- **详细贡献指南**: [`CONTRIBUTING.md`](CONTRIBUTING.md) — 添加 MCP Tool / Skill、子代理协作安全

---

## 许可证

MIT
