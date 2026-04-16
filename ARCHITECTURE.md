# devbase 架构讨论与技术决策记录

> 本文件记录了 devbase 项目从概念诞生到 MVP 实现过程中的关键技术讨论与架构决策。内容源自 2026-04-05 的系列对话，涵盖环境管理痛点分析、三层架构模型、存储粒度哲学，以及与 Clarity / syncthing-rust-rearch 的协同定位。

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

### 2.3 转化点 2：抽象层 → 应用层

通过 MCP（Model Context Protocol）接口，`devbase` 成为 Clarity Agent 的"环境感知器官"。LLM 不再"盲人摸象"，而是能直接查询：

- "用户本地有哪些 Rust 项目？"
- "系统 CMake 版本是多少？"
- "哪些第三方库超过 30 天未同步？"

---

## 三、关键架构决策：Git vs Syncthing 的粒度差异

### 3.1 问题的本质

> Git 和 Syncthing 都用哈希做内容校验，为什么 Git 是"文件级"，Syncthing 是"块级"？

**结论：粒度差异不是技术偏好，而是它们各自要解决的「事务」与所处的「环境」决定的。**

### 3.2 对比矩阵

| 维度 | **Git** | **Syncthing** |
|------|---------|---------------|
| **基本单位** | 整个文件（Blob） | 固定大小的块（Block，~128KB） |
| **核心目的** | 保存完整历史与版本树 | 最小化网络同步流量 |
| **主要维度** | **时间**（历史） | **空间**（分布） |
| **核心问题** | "这个文件**过去**长什么样？" | "这个文件在**另一台机器**上长什么样？" |
| **环境假设** | 数据量小、本地磁盘充足、完整性优先 | 数据量大、带宽昂贵、网络不可靠 |
| **改一个字的代价** | 生成全新 Blob（重复存储整文件） | 只传变动的那个块 |
| **去重粒度** | 文件级 | 块级 |
| **历史语义** | 强（Commit → Tree → Blob 的不可变快照） | 弱（只关心当前一致性） |

### 3.3 为什么 Git 不能是块级？

如果 Git 用块级管理源码：
- `git diff` 会显示"块 47 和块 112 变了"，而不是"第 38 行的函数签名改了"
- `git blame` 只能定位到"这个块最后一次修改是在 3 个月前"
- `git checkout v1.0` 需要先读取 base 再拼上几十个 delta 块，速度慢一个数量级

**源码的最小语义单位是"文件"**，块级会破坏版本控制的核心用户体验。

### 3.4 为什么 Syncthing 不能是文件级？

如果 Syncthing 用文件级同步：
- 旋转了一张 5MB 照片 90 度，同步时要重新传整个 5MB
- 改了一个 50GB 虚拟机镜像的配置字节，同步时要传 50GB

**在广域网环境下，文件级同步完全不可用。**

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
      │ 调用 MCP Tool: devkit_sync
      ▼
devbase (抽象层)
      │
      │ 查询 Registry / 生成 SyncPlan
      ▼
syncthing-rust-rearch (实体层)
      │
      │ 决定哪些目录需要被块级同步到远端
      ▼
Peer Device (另一台机器的实体层)
```

### 4.3 稀缺性

市场上没有一款工具整合了这三层：
- `chezmoi` / `ghq`：只做实体层 + 薄抽象层
- `Moon` / `Turborepo`：只做抽象层（Monorepo 任务图）
- `Cursor` / `Claude Code`：只做应用层，但对本地环境感知是"盲人摸象"

**devbase + Clarity + syncthing-rust-rearch 的垂直整合是稀缺能力**。

### 4.4 当前阶段声明（2026-04-05）

> **三者目前独立开发，融合尚未成熟。**
>
> - `devbase` 先完成自身的 CLI 和 Registry 抽象层
> - `Clarity` 先完成自身的 Agent 执行与 MCP 应用层
> - `syncthing-rust-rearch` 先完成自身的 P2P 块同步实体层
>
> 成熟期后，再通过明确的层间协议（MCP / REST / 配置契约）进行对接，而非过早耦合。

---

## 五、Memory Sovereignty（记忆主权）

### 5.1 从"仓库管理"到"记忆管家"

devbase 的演进方向不仅是管理 Git 仓库，而是成为**开发者知识资产的主权层**。这意味着：

- **数据物理上属于你**：所有元数据存储在本地 SQLite，无需云端数据库账号
- **同步边界由你定义**：通过 `data_tier`（`public` / `cooperative` / `private`）明确哪些数据可以离开本机
- **Agent 记忆的托管方**：Clarity 的 `SOUL.md` / `MEMORY.md`、agri-paper 的知识库，都可以被 devbase 追踪和保鲜

### 5.2 为什么 SQLite + 本地优先？

| 选择 | 理由 | 对立面（云服务）的风险 |
|------|------|----------------------|
| **SQLite** | 零外部依赖、单文件可复制、可被版本控制 | 云端数据库需要账号、网络、可能泄露查询模式 |
| **本地优先** | 代码/对话永不出境，满足合规与隐私需求 | SaaS 工具默认上传用户代码用于训练 |
| **Syncthing P2P** | 端到端加密、无中心化服务器、用户掌控 peer 列表 | 云同步服务商可见文件内容 |

### 5.3 `data_tier` 的架构意义

Registry Schema v2 引入了三层数据分级：

- `private`：默认状态。原始对话、私有代码、个人笔记。不同步到任何外部节点。
- `cooperative`：经授权后可参与模式聚合。例如去标识化的工具调用序列、农业诊断案例统计。
- `public`：完全开放的知识。例如开源文档、去标识化的通用百科。

CLI 已支持通过 `devbase meta <repo_id> --tier <tier>` 动态调整分级，使"数据主权"从架构设计落地为可操作的日常命令。

### 5.4 Syncthing 的对接价值

Syncthing 解决的是"数据怎么在机器之间搬运"，而 devbase 解决的是"搬运的数据有什么语义、值不值得更新"。

两者的衔接点：
- devbase 决定**哪些目录**需要被同步（基于 `data_tier` 和 `workspace_type`）
- syncthing-rust 决定**怎么高效同步**（块级 P2P、端到端加密）
- devbase 监控 `.sync-conflict` 文件，将冲突升华为"diverged"状态等待用户 merge

---

## 六、当前实现状态（2026-04-15）

### 5.1 CLI 功能清单

| 命令 | 状态 | 说明 |
|------|------|------|
| `devbase scan <path> --register` | ✅ 已实现 | 递归扫描 `.git`，注册到 SQLite；支持语言自动检测与 ZIP 快照标记 |
| `devbase health --detail` | ✅ 已实现 | 输出注册仓库数量与明细；增加环境工具链检测（rustc/cargo/node/go/cmake）、summary 统计、health 缓存 |
| `devbase sync --strategy=fetch-only` | ✅ 已实现 | 用 `git2` 原生 `fetch` + ahead/behind 差异计算；支持 `--filter-tags`、并发编排（`SyncOrchestrator`）、错误分类 |
| `devbase sync --strategy=auto-pull` | ✅ 已实现 | 自动快进合并；dirty 时自动跳过 |
| `devbase sync --dry-run` | ✅ 已实现 | 只预览不执行 |
| `devbase query <expression>` | ✅ 已实现 | 支持结构化查询表达式：`lang:rust`、`stale:>30`、`behind:>10`、`tag:third-party` 及关键词搜索 |
| `devbase tag <repo_id> <tags>` | ✅ 已实现 | 分类标签（own-project / third-party / tool） |
| `devbase clean` | ✅ 已实现 | 清理备份目录记录 |
| `devbase tui` | ✅ 已实现 | `ratatui` 交互式界面：异步事件循环、后台 Git 操作不阻塞 UI、inline spinner、按键 `s`/`S`/`t`/`h`/`Home`/`End` |
| `devbase mcp --transport stdio` | ✅ 已实现 | MCP Server 模式，暴露 `devkit_scan`/`devkit_health`/`devkit_sync`/`devkit_query` 4 个工具 |
| `devbase meta <repo_id> --tier <tier>` | ✅ 已实现 | Registry Schema v2：支持 `workspace_type` 和 `data_tier` 字段的动态更新 |
| `devbase watch <path> --duration` | ✅ 已实现 | 目录监控 + 事件聚合 + 变更调度；基于 `notify` 的 `ReadDirectoryChangesW` 实现 |

### 5.2 TUI 设计原则

`devbase tui` 是人类与 AI 共用的观测入口：

- **对人类**：键盘驱动、实时面板、一键刷新（`r`）、上下键导航、`q` 退出
- **对 AI**：同一状态可导出为结构化 JSON（未来实现 `--export` 模式）

布局：

```text
┌─────────────────┬──────────────────────────────────────┐
│  Repositories   │  Details                             │
│  📁 clarity     │  ID: clarity                         │
│  🔗 openclaw    │  Path: C:\...\clarity                │
│  🔗 lazygit     │  Branch: main                        │
│                 │  Tags: own-project,no-upstream       │
│                 │  Upstream: (none)                    │
├─────────────────┼──────────────────────────────────────┤
│  Logs           │                                      │
│  [12:47:01] ... │                                      │
└─────────────────┴──────────────────────────────────────┘
```

### 5.3 注册表现状

**总计 22 个项目**：

- **自有项目（3 个）**：`clarity`、`syncthing-rust-rearch`、`devbase`
  - `tags: own-project,no-upstream` 或 `tool`
  - `sync` 自动跳过

- **第三方参考库（30 个）**：已全部从 ZIP 迁移为 Git 官方仓库
  - 位置：`C:\Users\<user>\dev\third_party\`
  - `tags: third-party,reference`
  - 包含：`openclaw`、`lazygit`、`gitui`、`ollama`、`dify`、`codex`、`kimi-cli`、`iroh`、`tailscale`、`vllm`、`coze-studio`、`nanobot`、`claude-code-rust`、`zeroclaw`、`desktop`、`openhanako`、`AutoCLI`

---

## 七、后续规划

### 7.1 短期（1~2 周）

1. ~~**MCP 接口契约文档**~~ ✅ **已完成**
   - 定义 `devkit_scan`、`devkit_sync`、`devkit_query`、`devkit_health` 的输入输出模式
   - 完成"抽象层 → 应用层"的协议桥接

2. ~~**查询引擎增强**~~ ✅ **已完成**
   - 支持结构化查询表达式：`lang:rust stale:>30 behind:>10`
   - 解析 `Cargo.toml` / `package.json` / `go.mod` 提取语言与依赖关系

3. **系统健康检查**
   - 检测已安装工具链版本（rustc、node、go、cmake） ✅ 已实现
   - 检测磁盘空间与项目体积

4. **TUI 批量同步进度弹窗**
   - 为 `S` 键批量同步增加独立进度条或结果弹窗，替代当前 Logs 输出方式

5. **Registry 缓存预热**
   - 在 `health` 和 `query` 中复用 `repo_health` 缓存，减少重复 `git2::Repository::open` 的 IO 开销

### 7.2 中期（1 个月内）

1. **封装 `devbase-core` Rust 库**
   - 将核心能力（Registry、HealthEngine、SyncOrchestrator、QueryEngine）抽象为可复用 crate

2. **集成到 Clarity 的 MCP 工具集**
   - 让 Clarity Agent 能直接调用 devbase 能力
   - 实现"环境上下文自动注入"

3. **Syncthing REST API 对接**
   - 基于 `SyncIndex` 和 `WatchAggregator`，通过 Syncthing REST API 动态创建/更新 folder 配置
   - 完成「devbase 决策 → syncthing 执行」的同步闭环

### 7.3 长期（3 个月内）

1. **依赖图谱可视化**
   - 绘制项目间的引用关系（哪些项目依赖 tokio、哪些参考了 openclaw）

2. **与 syncthing-rust-rearch 的目录同步集成**
   - devbase 决定"哪些目录需要被同步"
   - syncthing-rust-rearch 决定"怎么高效同步"

---

## 七、术语表

- **Blob**：Git 中文件内容的不可变对象，以内容哈希命名。
- **Packfile**：Git 的后台存储优化格式，对同一文件的不同版本做 delta 压缩。
- **MCP**：Model Context Protocol，LLM 与外部工具交互的标准协议。
- **Registry**：devbase 的核心数据结构，记录所有被管理仓库的元数据。
- **SyncOrchestrator**：devbase 的同步编排器，负责按标签和策略批量更新仓库。
- **ZIP 快照**：指通过 GitHub "Download ZIP" 获取的源码快照，不含 `.git` 历史。
