# devbase 架构讨论与技术决策记录

> 本文件记录了 devbase 项目从概念诞生到 MVP 实现过程中的关键技术讨论与架构决策。
> 
> **当前版本**：2026-04-17（Sprint 1 完成，Sprint 2 规划中）

---

## 一、项目缘起：从桌面 ZIP 灾难到知识库管理

### 1.1 原始问题

用户桌面上有约 20+ 个项目文件夹（`openclaw-main`、`lazygit-master`、`ollama-main` 等），它们全部是 **GitHub ZIP 下载包**（文件夹名带 `-main`/`-master` 后缀）。这导致：

- ❌ 无法 `git pull` 获取上游更新
- ❌ 无法查看提交历史、对比变更
- ❌ 无法追踪自己的本地修改
- ❌ 长期管理沦为"僵尸代码"

### 1.2 核心洞察

> "把源码从桌面移到统一目录"只是表面动作；更深层的价值在于，**开发者的本地环境本身就是一个未被结构化的数据库**。把这个数据库管好，让它可被查询、可被保鲜、可被策略化地同步，这就是一个**面向开发者的知识管理框架**。

---

## 二、三层架构模型：字节 → 语义 → 行动

### 2.1 模型总览

```text
┌─────────────────────────────────────────────────────────────┐
│  应用层（Application / Protocol Layer）                      │
│  ─────────────────────────────────────                      │
│  • MCP 工具（devkit_scan, devkit_sync, devkit_query）       │
│  • Agent Skill（Clarity 的 reasoning + action 接口）        │
│  • CLI / TUI（devbase scan, sync, health）                  │
├─────────────────────────────────────────────────────────────┤
│  抽象层（Semantic / Knowledge Layer）                        │
│  ─────────────────────────────────────                      │
│  • Registry（仓库节点、标签、依赖关系）                     │
│  • 知识图谱（项目 A 参考项目 B，语言 Rust，策略 fetch-only）│
│  • 健康状态抽象（dirty、stale: 30d、behind: 12）            │
│  • 同步策略（what should be synced, when, how）             │
├─────────────────────────────────────────────────────────────┤
│  实体层（Physical / Storage Layer）                          │
│  ─────────────────────────────────────                      │
│  • 文件系统（NTFS / ext4 / APFS）                            │
│    - Git 对象数据库（.git/objects/）                        │
│    - 源码文件树（src/, Cargo.toml）                         │
│  • 本地数据库（SQLite：devbase registry.db）                 │
│  • Syncthing 的块索引与本地版本向量                          │
└─────────────────────────────────────────────────────────────┘
```

### 2.2 转化点 1：实体层 → 抽象层

`devbase scan` 把无结构的文件系统转化为有语义的知识库：

| 实体层原始信号 | 抽象层提取结果 |
|---------------|---------------|
| `lazygit-master/.git/config` | `RepoEntry { upstream: "github.com/jesseduffield/lazygit", tags: "third-party,reference" }` |
| `cargo build` 失败 | `HealthReport { rustc: "1.94.1", status: "编译失败" }` |
| 目录名带 `-main`/`-master` | `RepoEntry { source_type: "zip-snapshot", needs_migration: true }` |
| `.gitmodules` 存在 | `DependencyEdge { from: "openclaw", to: "submodule-x", type: "git-submodule" }` |
| `SOUL.md` / `.devbase` 标记 | `RepoEntry { workspace_type: "openclaw" | "generic", data_tier: "private" }` |

### 2.3 转化点 2：抽象层 → 应用层

通过 MCP（Model Context Protocol）接口，`devbase` 成为 Clarity Agent 的"环境感知器官"。LLM 不再"盲人摸象"，而是能直接查询：

- "用户本地有哪些 Rust 项目？"
- "系统 CMake 版本是多少？"
- "哪些第三方库超过 30 天未同步？"
- "我的农业知识库中水稻病害的最新记录是什么？"

---

## 三、关键架构决策

### 3.1 Git vs Syncthing 的粒度差异

| 维度 | **Git** | **Syncthing** |
|------|---------|---------------|
| **基本单位** | 整个文件（Blob） | 固定大小的块（Block，~128KB） |
| **核心目的** | 保存完整历史与版本树 | 最小化网络同步流量 |
| **主要维度** | **时间**（历史） | **空间**（分布） |
| **核心问题** | "这个文件**过去**长什么样？" | "这个文件在**另一台机器**上长什么样？" |
| **环境假设** | 数据量小、本地磁盘充足、完整性优先 | 数据量大、带宽昂贵、网络不可靠 |
| **去重粒度** | 文件级 | 块级 |
| **历史语义** | 强（Commit → Tree → Blob 的不可变快照） | 弱（只关心当前一致性） |

### 3.2 数据主权（Memory Sovereignty）

Registry Schema v2 引入了三层数据分级：

- `private`：默认状态。原始对话、私有代码、个人笔记。不同步到任何外部节点。
- `cooperative`：经授权后可参与模式聚合。例如去标识化的工具调用序列、农业诊断案例统计。
- `public`：完全开放的知识。例如开源文档、去标识化的通用百科。

CLI 已支持通过 `devbase meta <repo_id> --tier <tier>` 动态调整分级。

---

## 四、与 Clarity / syncthing-rust-rearch 的协同定位

### 4.1 三个项目的独立价值

| 项目 | 核心事务 | 占据的层次 |
|------|---------|-----------|
| **syncthing-rust-rearch** | 解决"数据怎么在机器之间搬运" | 实体层 + 部分抽象层 |
| **devbase** | 解决"搬运的数据有什么语义、值不值得更新" | 抽象层 + 部分应用层 |
| **Clarity** | 解决"Agent 如何基于这些语义做出决策并执行" | 应用层 |

### 4.2 垂直协同关系

```text
Clarity (应用层)
      │
      │ 调用 MCP Tool: devkit_sync / devkit_agri_query
      ▼
devbase (抽象层)
      │
      │ 查询 Registry / 生成 SyncPlan / 农业知识图谱
      ▼
syncthing-rust-rearch (实体层)
      │
      │ 决定哪些目录需要被块级同步到远端
      ▼
Peer Device (另一台机器的实体层)
```

### 4.3 当前阶段声明（2026-04-17）

> **三者已通过 MCP 协议完成初步融合。**
>
> - `devbase`：Sprint 1 完成 CLI/TUI/Registry/MCP SSE/OpLog，Registry 规模 39 个工作区
> - `Clarity`：通过 MCP stdio 调用 devbase 工具，SSE transport 待 Daemon 常驻模式
> - `syncthing-rust-rearch`：BEP 协议栈就绪，`.syncdone` 标记格式已对齐，REST API 集成待 Sprint 2
>
> 成熟期通过明确的层间协议（MCP / REST / 配置契约 / `.syncdone` 文件标记）进行对接，避免过早耦合。

---

## 五、Registry Schema 演进

| 版本 | 日期 | 变更 |
|------|------|------|
| v1 | 初始 | `repos` 表含 `upstream_url`（扁平） |
| v2 | 2026-04-15 | 新增 `workspace_type`、`data_tier`、`last_synced_at`；`repo_tags`/`repo_remotes` 规范化 |
| v3 | 2026-04-15 | 新增 `workspace_snapshots` 表（非 Git 工作区 blake3 快照） |
| v4 | 2026-04-17 | 新增 `oplog` 表（操作日志）；迁移前自动备份 |

---

## 六、三仓库架构视角（横向分层）

> 本文档的"二、三层架构模型"是**纵向分层**（物理 → 语义 → 应用）。
> 与之互补的是**横向分层**：把 AI 基础设施拆分为三个正交的"仓库"——代码仓库、Skill 仓库、MCP 仓库。
>
> 详见独立文档：[`docs/theory/three-repositories.md`](./theory/three-repositories.md)

### 6.1 为什么需要横向分层

纵向分层回答的是"数据在系统中如何流转"；横向分层回答的是"能力在生态中如何被复用"。

| 纵向层 | 横向仓库 | 核心问题 |
|--------|---------|---------|
| 实体层 | **GitHub 仓库** | 代码资产存在哪里？ |
| 抽象层 | **Skill 仓库** | AI 能做什么？怎么做？ |
| 应用层 | **MCP 仓库** | AI 通过什么协议调用能力？ |

### 6.2 devbase 横跨三层

devbase 的当前模块已经天然分布在三仓库中：

- **Layer 1（GitHub 仓库）**：`registry` 模块管理代码资产的元数据、符号、调用图
- **Layer 2（Skill 仓库）**：`vault/` + `skill-sync-prototype` 探索 Vault → Skill 同步
- **Layer 3（MCP 仓库）**：`src/mcp/` 实现 31 个 MCP tools，作为协议适配器

三仓库之间的**正交性**意味着：改变 Skill 的业务逻辑不应影响 MCP 的协议格式，替换 GitHub 数据源不应破坏 Skill 的执行语义。

---

## 七、当前实现状态（2026-04-17）

### 6.1 CLI 功能清单

| 命令 | 状态 | 说明 |
|------|------|------|
| `devbase scan <path> --register` | ✅ 已实现 | Git + 非 Git（`SOUL.md`/`.devbase`）工作区；语言自动检测；ZIP 快照标记 |
| `devbase health --detail` | ✅ 已实现 | Git: dirty/ahead/behind；非 Git: blake3 快照变更检测 |
| `devbase sync --strategy=auto-pull` | ✅ 已实现 | Safe Sync 预检：dirty/diverged/protected 自动跳过；并发编排；可配置超时 |
| `devbase sync --dry-run` | ✅ 已实现 | 只预览不执行 |
| `devbase query <expression>` | ✅ 已实现 | `lang:rust`、`stale:>30`、`behind:>10`、`tag:third-party` |
| `devbase tag <repo_id> <tags>` | ✅ 已实现 | 分类标签；支持 `agri:crop:rice` 分层命名空间 |
| `devbase meta <repo_id> --tier <tier>` | ✅ 已实现 | `workspace_type` / `data_tier` 动态更新 |
| `devbase tui` | ✅ 已实现 | ratatui 异步事件循环；Safe Sync Preview 弹窗；commit 对比；标签聚类 |
| `devbase mcp --transport stdio` | ✅ 已实现 | 10 个 MCP 工具 |
| `devbase mcp --transport sse --port` | ✅ 已实现 | Axum SSE Server；端到端验证通过 |
| `devbase registry export/import/backups` | ✅ 已实现 | SQLite + JSON 双格式；自动保留 10 个快照 |
| `devbase oplog --limit N` | ✅ 已实现 | scan/sync/health 自动记录；按 repo 过滤 |
| `devbase watch <path> --duration` | ✅ 已实现 | 目录监控 + 事件聚合 + 变更调度 |
| `devbase clean` | ✅ 已实现 | 清理备份目录记录 |

### 6.2 测试状态

```
42 passed / 0 failed / 2 ignored
```

### 6.3 Registry 规模

**总计 39 个工作区**：
- **自有项目（4 个）**：`clarity`、`syncthing-rust-rearch`、`devbase`、`agri-paper`
- **第三方参考库（35 个）**：包括 `gws`（Google Workspace CLI）、`5ire`（MCP Client）、`workspace-tools`（changeset 管理）等竞品

---

## 八、后续规划

### Sprint 2（Phase 2，2026-04-18 ~ 2026-05-01）

| 周 | 任务 | 产出 | 阻塞依赖 |
|----|------|------|---------|
| W1 | `McpTool::invoke_stream()` trait 扩展 | 支持 `progress` → `partial` → `done` 三段式 event | 无 |
| W1 | `agri_observations` schema migration | 农业领域表 + `devkit_agri_query` MCP tool | **等 agri-paper DDL PR** |
| W2 | SSE handler 流式适配 | `messages_handler` 支持分段推送；stdio 向后兼容 | W1 完成 |
| W2 | CLI/TUI pagination | `--limit` / `--page` 参数 | 无 |
| W3–W4 | `devkit_health`/`devkit_query` 流式集成 | TUI 进度条；Agent 不再阻塞 2–5s | W2 完成 |
| W5–W8 | Daemon 内置 SSE Server + clarity 长连接 | `devbase daemon` 常驻运行 MCP SSE；clarity URL 配置 | clarity-core MCP Client SSE 配置 |

### Sprint 3（Phase 2 后半，2026-05-01 ~ 2026-05-15）

1. **`devbase-core` crate 剥离**：将 Registry/HealthEngine/SyncOrchestrator/QueryEngine 抽象为可复用库，解除 `clarity-core` path dependency
2. **`.syncdone` 文件标记落地**：集成 syncthing-rust `FolderStatus::Idle` REST endpoint
3. **农业 Persona TOML 集成**：`PersonalityConfig` 模板变量插值 + `agri_expert.toml` 校验

### Phase 3（2026-05-15 ~ 2026-07-15）

1. **依赖图谱可视化**：项目间引用关系图
2. **语义检索层**：本地嵌入模型（bge-m3 或 kalosm）替代规则模式 fallback
3. **MCP 协议版本协商**：保持与旧版 Clarity 兼容

---

## 九、术语表

- **Blob**：Git 中文件内容的不可变对象，以内容哈希命名。
- **Packfile**：Git 的后台存储优化格式，对同一文件的不同版本做 delta 压缩。
- **MCP**：Model Context Protocol，LLM 与外部工具交互的标准协议。
- **Registry**：devbase 的核心数据结构，记录所有被管理仓库的元数据。
- **SyncOrchestrator**：devbase 的同步编排器，负责按标签和策略批量更新仓库。
- **OpLog**：操作日志，记录 scan/sync/health 等关键操作的审计追踪。
- **Data Tier**：数据分级（`public`/`cooperative`/`private`），控制同步边界。
