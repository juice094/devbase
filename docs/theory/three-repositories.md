# 三仓库架构：代码仓库、Skill 仓库与 MCP 仓库

> **TL;DR**: AI 基础设施不应把"代码资产"、"AI 能力"和"交互协议"混为一谈。三者是**正交分层**，各自有成熟的理论支撑和开源实现。devbase 的演进方向是成为横跨三层的统一基底。

---

## 1. 问题背景

当前 AI 工具链存在一个结构性混淆：**代码存储**、**能力编排**和**协议适配**被压缩在同一层。

- GitHub Copilot 把代码仓库直接当上下文喂给 LLM
- Claude Code 的 `.claude/skills/` 和代码放在同一个 Git 仓库里
- MCP Server 既定义协议，又内嵌业务逻辑

这导致三个问题：
1. **能力不可复用**：一个项目里的 skill 无法被另一个项目调用
2. **协议锁定**：绑死 MCP 就排斥了 OpenAPI、LangChain Tools、本地函数
3. **上下文爆炸**：把整份代码当 prompt，token 浪费且信噪比低

"三仓库"架构把三者解耦，各自用最适合的存储、注册和发现机制。

---

## 2. 三层定义

### Layer 1 — GitHub 仓库（数据面 / What exists）

**职责**：存储代码资产、文档、配置、历史版本。

**核心特征**：
- 版本控制（Git）
- 代码语义（AST、符号表、调用图）
- 人机可读（源码、README）

**成熟实现**：GitHub、GitLab、Bitbucket、OCI Artifact Registry

**在 devbase 中的对应**：
- `repos` 表：GitHub 仓库的元数据索引
- `code_symbols` / `code_call_graph`：代码语义的提取与持久化
- `sync` 模块：把 Git 状态同步到本地 SQLite 知识基底

**关键洞察**：GitHub 仓库是**被动的数据资产**，它不主动告诉 AI"我能做什么"，只回答"我里面有什么"。

---

### Layer 2 — Skill 仓库（能力面 / How to operate）

**职责**：定义 AI 可执行的能力单元——做什么、需要什么输入、产生什么输出、依赖哪些工具。

**核心特征**：
- 能力注册与发现（registry + search）
- 版本与依赖管理
- 执行隔离（sandbox / timeout / retry）
- 可跨项目复用

**成熟实现与理论支撑**：

| 项目/论文 | 核心贡献 | 与 Skill 仓库的对应 |
|---|---|---|
| **Agent Knowledge Architecture** (arXiv:2603.14805) | 提出 AKU (Atomic Knowledge Unit) Registry：可查询的原子知识单元目录，支持语义搜索和依赖拓扑 | AKU Registry = Skill 仓库；Knowledge Topology = Skill 之间的依赖/编排关系 |
| **OmniRoute Skills System** (GitHub 开源) | `registry.ts` (SQLite-backed skill registration)、`executor.ts` (timeout/retry)、`sandbox.ts` (isolation) | 完整的 Skill Runtime 实现，直接验证"devbase 存储 skill 服务"的可行性 |
| **LM-Kit.NET SkillRegistry** (开源 .NET) | `LoadFromUrlAsync` (从 GitHub 拉取)、`FindMatchesWithEmbeddings` (语义匹配)、`SkillTool` (暴露为 LLM tool)、`SkillWatcher` (热重载) | Skill 可以远程分发、动态加载、语义发现 |
| **Semantic Kernel PluginCollection** (Microsoft) | `KernelPluginCollection` 管理插件注册；`Kernel` 作为 DI 容器聚合 services + plugins | Plugin = Skill；Kernel = Skill 的执行环境 |
| **BMO Agent** (开源 TS) | `skills.ts` (registry + load_skill)、`tool-loader.ts` (动态 .mjs 加载)、Markdown+YAML frontmatter 格式 | Skill 文件格式与 Clarity/Kimi CLI 的 SKILL.md 完全兼容 |

**在 devbase 中的对应**：
- `vault/examples/skill-sync-prototype.md`：Vault → Skill 同步原型
- `docs/theory/AI_TOOL_CONTEXT_RESEARCH.md`：Vault 笔记作为"可共享 Skill"的长期规划
- **待实现**：`devbase skill` 子命令（install / list / run / sync）

**关键洞察**：Skill 仓库是**主动的能力声明**。一个 Skill 可以说"我能做代码审计"，而不需要把审计规则写死在某个 GitHub 仓库里。

---

### Layer 3 — MCP 仓库（协议面 / How to interact）

**职责**：标准化 AI 与外部系统的交互协议——不是"做什么"，而是"怎么调用"。

**核心特征**：
- Protocol-agnostic（协议无关）
- Schema 自动生成与验证
- 多协议适配（MCP、OpenAPI、LangChain Tools、本地函数）
- 执行引擎（sync/async、并发、错误回退）

**成熟实现与理论支撑**：

| 项目/论文 | 核心贡献 | 与 MCP 仓库的对应 |
|---|---|---|
| **Model Context Protocol** (Anthropic) | 定义 AI 与外部工具/数据源交互的标准协议（JSON-RPC 2.0 之上） | MCP 是协议规范本身，不是仓库。MCP Server = 协议适配器 |
| **ToolRegistry** (arXiv:2507.10593 + 开源 Python) | "Protocol-agnostic tool management library that unifies diverse tool sources (native Python, MCP, OpenAPI, LangChain) under a single interface" | **核心验证**：MCP 只是众多协议中的一种，Skill 应该通过 protocol-agnostic 层调用任何协议的工具 |
| **AGNTCY Agent Directory Service** (arXiv:2509.18787) | 联邦化的 Agent 能力发现系统：Schema Layer + Indexing Layer + Storage Layer + Distribution Layer + Security Layer | 长期愿景：devbase 可以作为本地 ADS 的轻量实现 |
| **LangChain BaseToolkit** | 把一组相关工具打包为 `get_tools()` 入口，支持 registry/factory 模式 | Toolkit = 协议面的工具集合；Registry = 协议面的注册中心 |

**在 devbase 中的对应**：
- `src/mcp/`：31 个 MCP tools 的实现
- `src/mcp/tools/`：按 Stable / Beta / Experimental 分级的 tool 注册表
- **待实现**：protocol-agnostic 工具管理层（MCP + OpenAPI + 本地函数统一入口）

**关键洞察**：MCP 是**通道**，不是**能力**。Skill 说"我要做代码审计"，MCP 说"通过 `devkit_hybrid_search` 这个 JSON-RPC 调用来实现"。两者不能混为一谈。

---

## 3. 正交性原则

三层的核心关系是**正交**（orthogonal）——改变一层不应破坏另一层。

```
┌─────────────────────────────────────────┐
│  Layer 2: Skill 仓库（能力面）            │
│  "我要做代码审计"                         │
│  ── 不依赖具体协议或代码位置 ──           │
├─────────────────────────────────────────┤
│  Layer 3: MCP 仓库（协议面）              │
│  "通过 devkit_hybrid_search 获取代码知识"  │
│  ── 不依赖 skill 的业务逻辑 ──            │
├─────────────────────────────────────────┤
│  Layer 1: GitHub 仓库（数据面）           │
│  "audit-target 仓库的符号、调用图、README" │
│  ── 被动的数据资产 ──                     │
└─────────────────────────────────────────┘
```

**正交性验证**：

| 场景 | Layer 1 | Layer 2 | Layer 3 |
|---|---|---|---|
| 同一个 GitHub 项目，换一套审计规则 | 不变 | 替换 Skill | 不变 |
| 审计规则不变，改用 OpenAPI 而非 MCP | 不变 | 不变 | 替换协议适配器 |
| 审计一个完全不同的 GitHub 项目 | 替换数据源 | 不变 | 不变 |

---

## 4. devbase 在三仓库中的定位

devbase 的当前架构已经**天然横跨三层**，只是没有明确形式化：

| devbase 模块 | 所属层 | 当前状态 |
|---|---|---|
| `registry` (repo sync, code symbols, call graph) | Layer 1 (GitHub 仓库) | ✅ 成熟：48 repos, 42K+ symbols, 300K+ calls |
| `search/hybrid` + `semantic_index` | Layer 1→2 桥梁 | ✅ 成熟：keyword + vector + RRF |
| `mcp/` (31 tools) | Layer 3 (MCP 仓库) | ✅ 成熟：Stable/Beta/Experimental 三级 |
| `vault/` + `skill-sync-prototype` | Layer 2 (Skill 仓库) | 🔄 原型：Vault→Skill 同步设计完成，待实现 |
| `tools/embedding-provider/` | Layer 2→3 桥梁 | ✅ 可用：local.py 引擎无关 |
| **Skill Runtime** (`devbase skill` CLI) | Layer 2 (Skill 仓库) | ❌ 未开始：registry + executor + sandbox |
| **Protocol Adapter** (MCP + OpenAPI 统一) | Layer 3 (MCP 仓库) | ❌ 未开始：ToolRegistry 模式移植 |

**devbase 的独特价值**：
- 它是**唯一一个把三层数据存在同一个 SQLite 文件里**的系统
- GitHub 仓库的符号、Skill 的元数据、MCP tool 的调用记录，共享同一个知识图谱
- 这让 cross-layer 查询成为可能："找到做过代码审计的 skill，然后用它审计这个仓库"

---

## 5. 与 Clarity / Kimi CLI 生态的协同

| 系统 | 在三仓库中的角色 | 与 devbase 的接口 |
|---|---|---|
| **Clarity** | Layer 2 消费者：加载 Skill，注入 system prompt | `devbase skill sync --target clarity` (计划中) |
| **Kimi CLI** | Layer 2 消费者：通过 SKILL.md 加载能力 | devbase 提供 `.kimi/skills/devbase/` 项目级 skill |
| **MCP Client** (Clarity/Kimi/VS Code) | Layer 3 消费者：调用 MCP tools | `devbase mcp` 子命令启动 stdio/sse server |
| **GitHub** | Layer 1 数据源 | `devbase sync` 拉取仓库元数据 |

---

## 6. 参考文献

1. **Agent Knowledge Architecture** — *AI Skills as the Institutional Knowledge Primitive for Agentic Software Development*, arXiv:2603.14805v1, 2026.
2. **OmniRoute** — GitHub: `diegosouzapw/OmniRoute`, TypeScript Agent Gateway with SQLite-backed Skill Registry.
3. **LM-Kit.NET** — `lm-kit.com`, .NET Skill Registry with embedding-based semantic matching and hot reload.
4. **ToolRegistry** — *ToolRegistry: A Protocol-Agnostic Tool Management Library*, arXiv:2507.10593v1 + GitHub: `Oaklight/ToolRegistry`.
5. **AGNTCY ADS** — *The AGNTCY Agent Directory Service*, arXiv:2509.18787v1, Cisco/AGNTCY, 2025.
6. **Semantic Kernel** — Microsoft, `KernelPluginCollection` + `Kernel` DI container architecture.
7. **BMO Agent** — GitHub: `joelhans/bmo-agent`, Bun/TypeScript agent with Markdown+YAML Skill system.
8. **MCP Protocol** — Anthropic, Model Context Protocol Specification.

---

## 7. 待决策事项

- [ ] 是否在 devbase 中内置 `SkillRegistry` SQLite 表（schema 设计）
- [ ] Skill 执行引擎的隔离级别：进程级（BMO）、Wasm 级（ArmaraOS）、还是纯函数级
- [ ] Protocol Adapter 的实现策略：直接集成 ToolRegistry Python 库，还是 Rust 原生实现
- [ ] Skill 分发机制：Git 子模块、GitHub Release、还是独立 registry 服务
