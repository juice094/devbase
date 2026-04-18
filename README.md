# devbase

**Bimodal Developer Workspace OS** — 双模态开发者工作区操作系统

> 人类用 TUI 一览众山小，AI 用 MCP 通览本地库。
> One dashboard for humans. One API for agents.

devbase 是一个**本地优先的双模态工作区操作系统**：它既是为人类开发者设计的**多仓库 TUI 仪表盘**，也是为 AI Agent 提供的**结构化代码库知识入口**。在 AI 无法识别你的 GUI、无法感知你的本地文件系统的今天，devbase 是 AI 理解你本地代码库的**唯一入口**。

---

## Memory Sovereignty

你的知识主权，由你掌控。devbase 不仅追踪 Git 仓库，也追踪任何被你标记为知识的工作区。数据默认留在本地；你决定哪些内容——如果有的话——离开你的机器。

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
│  │   终端交互仪表盘     │    │    │   stdio / SSE       │    │
│  │                     │    │    │                     │    │
│  │ • 多仓库健康总览     │    │    │ • devkit_scan       │    │
│  │ • 标签聚类排序       │    │    │ • devkit_health     │    │
│  │ • 本地/远程对比      │    │    │ • devkit_sync       │    │
│  │ • 知识日报           │    │    │ • devkit_query      │    │
│  └─────────────────────┘    │    └─────────────────────┘    │
│                             │                               │
│  一眼看遍所有仓库状态        │    让 AI 拥有本地代码库的       │
│  人类的「多仓库入口」        │    结构化世界观                 │
├─────────────────────────────┴───────────────────────────────┤
│                      Data Layer                             │
│                    (数据与引擎层)                            │
│                                                             │
│   SQLite Registry │ Git2 / blake3 │ OpLog │ Config │ Backups│
│                                                             │
│   • Git 仓库 + 标记目录的自动发现与持久化                     │
│   • 安全同步策略 (Mirror / Conservative / Rebase / Merge)    │
│   • 数据分级 (public / cooperative / private)                │
│   • 操作审计日志与 schema 迁移快照                           │
└─────────────────────────────────────────────────────────────┘
```

### Human Mode — 人类模式

基于 [ratatui](https://github.com/ratatui/ratatui) 的终端交互界面，专为多仓库场景设计：

- **多仓库健康总览**：一眼看清所有仓库的 dirty / ahead / behind 状态
- **标签聚类排序**：按技术栈、优先级或自定义标签分组浏览
- **本地/远程 Commit 对比**：快速查看尚未推送的变更
- **知识日报**：基于健康状态和摘要生成每日工作简报

### AI Mode — 智能体模式

基于 [Model Context Protocol](https://modelcontextprotocol.io) 的标准化接口，让 AI Agent 能够：

- **发现本地代码库**：`devkit_scan` 暴露所有注册工作区的元数据
- **查询健康状态**：`devkit_health` 获取仓库 dirty/ahead/behind 的结构化数据
- **执行安全同步**：`devkit_sync` 在预检规则下批量 fetch/pull
- **知识检索**：`devkit_query` 按标签、类型、状态过滤工作区

支持 **stdio**（本地进程通信）与 **SSE**（HTTP 流式传输）双模式，适配从本地 Claude Desktop 到远程 AI 服务的全场景。

---

## 为什么 devbase？

### 不是替代，是连接

| 工具 | 定位 | devbase 的角色 |
|------|------|---------------|
| **lazygit** | 单仓库 TUI，人类逐仓操作 | devbase 是**多仓库入口**——在 lazygit 之前，先告诉你「哪些仓库需要关注」 |
| **5ire / Claude Code** | AI 助手，对话式编程 | devbase 是**代码库知识源**——让 AI 拥有本地工作区的结构化上下文，不再「盲人摸象」 |
| **GitHub / GitLab** | 远程代码托管 | devbase 是**本地镜像管家**——批量管理远程同步，dirty/diverged 自动保护 |

### AI 无法识别你的 GUI

你的 IDE、文件管理器、甚至 lazygit 的界面，对 AI 来说都是不可见的黑箱。devbase 通过 MCP Server 将本地代码库的状态、结构、健康度翻译成 AI 能理解的结构化数据——这是 AI 介入本地开发流程的**基础设施**。

---

## 安全同步策略 (Safe Sync)

devbase 的同步不是粗暴的 `git pull --all`，而是分级的安全策略：

| 策略 | 行为 | 适用场景 |
|------|------|---------|
| **Mirror** | 强制与远程一致，丢弃本地未跟踪变更 | CI/CD 镜像仓库 |
| **Conservative** | 仅 fast-forward，遇到 diverged 自动跳过 | 日常开发，安全第一 |
| **Rebase** | 自动 rebase 本地提交到远程分支之上 | 个人分支，保持线性历史 |
| **Merge** | 自动 merge 远程变更 | 协作分支，接受合并历史 |

同步前自动预检：dirty 工作区、diverged 分支、protected 分支均会被跳过并记录到 OpLog，绝不擅自破坏你的工作成果。

---

## 功能

- **工作区扫描与注册**：自动发现 Git 仓库 **以及** `SOUL.md` / `MEMORY.md` / `.devbase` 标记的非 Git 工作区，持久化到 SQLite
- **GitHub 同步**：批量 fetch/pull 注册仓库的远程更新，支持并发、超时控制、Safe Sync 预检（dirty/diverged/protected 自动跳过）
- **健康检查**：追踪 Git 仓库的 dirty / ahead / behind，以及非 Git 工作区的 blake3 哈希快照变更检测
- **知识日报**：基于仓库健康状态和摘要生成每日简报
- **TUI 交互界面**：基于 ratatui 的终端交互；支持本地/远程 commit 对比、按标签聚类排序
- **MCP Server**：stdio 与 SSE 双传输模式，暴露 `devkit_scan`/`devkit_health`/`devkit_sync`/`devkit_query` 等工具
- **Registry 备份**：`export`/`import`/`backups`/`clean`，schema 迁移前自动快照
- **操作日志 (OpLog)**：`scan`/`sync`/`health` 自动记录，可追溯审计
- **数据分级**：`public` / `cooperative` / `private` 三级，控制同步边界

---

## 快速开始

```bash
# 扫描并注册当前目录下的所有工作区（Git + 非 Git）
cargo run -- scan . --register

# 批量同步全部仓库（fetch-only）
cargo run -- sync

# 查看工作区健康状态
cargo run -- health --detail

# 生成知识日报
cargo run -- digest

# 启动 TUI
cargo run -- tui

# 查看操作日志
cargo run -- oplog --limit 20

# 导出 registry 备份
cargo run -- registry export --format json

# 启动 MCP SSE Server
cargo run -- mcp --transport sse --port 3001
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

## 依赖

- Rust 2024 edition
- SQLite (bundled via `rusqlite`)
- 可选：`clarity-core`（用于 LLM 驱动的仓库摘要生成）

---

## 配置

配置文件位于：
- Windows: `%LOCALAPPDATA%\devbase\config.toml`

---

## 许可证

MIT
