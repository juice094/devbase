# devbase 依赖拓扑规划

> 本文件按**模块间依赖关系**（而非时间排期）重新组织 devbase 的功能演进顺序。
> 
> 原则：**下层为上层提供契约，上层为下层提供场景验证。** 改动下层（高扇入枢纽）影响面大，需更严格测试；叶节点可独立迭代实验。

---

## 一、拓扑总览

```text
Tier 11  ┌────────────────────────────────────────┐
         │  main.rs / daemon.rs                   │  ← CLI 入口、守护进程
Tier 10  ├────────────────────────────────────────┤
         │  tui/ (theme/layout/state/event/render)│  ← 终端交互层
Tier 9   ├────────────────────────────────────────┤
         │  mcp/ (mod + tools/*)                  │  ← MCP 协议适配层，35 tools
Tier 8   ├────────────────────────────────────────┤
         │  knowledge_engine, skill_sync, vault   │  ← 高级聚合、跨层桥接
Tier 7   ├────────────────────────────────────────┤
         │  workflow/ (model/parser/validator/    │  ← Workflow 编排引擎
         │            interpolate/scheduler/      │
         │            state/executor)             │
Tier 6   ├────────────────────────────────────────┤
         │  skill_runtime/ (parser/registry/      │  ← Skill 全生命周期
         │                 discover/dependency/   │
         │                 executor/scoring/      │
         │                 publish/clarity_sync)  │
Tier 5   ├────────────────────────────────────────┤
         │  sync/ (policy/tasks/orchestrator)     │  ← 同步编排
         │  watch                                 │  ← 文件系统监控
Tier 4   ├────────────────────────────────────────┤
         │  query, health, oplog_analytics        │  ← 查询与报告
         │  search/hybrid                         │  ← 混合检索
Tier 3   ├────────────────────────────────────────┤
         │  dependency_graph, symbol_links        │  ← 代码分析关联
         │  vault/indexer, vault/backlinks        │  ← Vault 索引与链接
Tier 2   ├────────────────────────────────────────┤
         │  scan, vault/scanner, discovery_engine │  ← 扫描与发现
         │  backup, test_utils                    │  ← 备份与测试辅助
Tier 1   ├────────────────────────────────────────┤
         │  registry/ (类型定义 + CRUD impl)      │  ← 数据契约中心
         │  vault/{fs_io,frontmatter,wikilink}    │  ← Vault 原子操作
         │  semantic_index, embedding, search     │  ← 索引原子能力
         │  arxiv                                 │  ← 外部论文 API
Tier 0   ├────────────────────────────────────────┤
         │  core, i18n, config, asyncgit          │  ← 类型根与配置根
         │  sync_protocol                         │  ← 同步协议基础类型
         └────────────────────────────────────────┘
```

---

## 二、逐层规划

### Tier 0 — 原子基础层

**目的**：为整个系统提供**无业务语义**的底层能力，任何改动都不应破坏上层编译。

| 模块 | 功能 | 依赖数 | 被依赖数 |
|:---|:---|:---:|:---:|
| `core` | NodeType / Node / Edge 统一实体枚举 | 0 | 中 |
| `i18n` | 静态字符串表（en/zh_cn） | 0 | **高** |
| `config` | 配置结构体：LLM、Embedding、Sync、Daemon | 0 | **高** |
| `asyncgit` | 异步 Git 状态通知通道（crossbeam） | 0 | 中 |
| `sync_protocol` | FileInfo / SyncIndex / scan_directory | 0 | 低 |

**迭代策略**：极度稳定，变更需全量回归。`i18n` 新增字段需同步所有语言文件。

---

### Tier 1 — 数据契约与原子能力层

**目的**：定义**领域模型**和**可独立工作的原子能力**。registry 是整个系统的"心脏"，所有业务数据最终落盘于此。

| 模块/子模块 | 功能 | 依赖 | 被依赖 |
|:---|:---|:---|:---|
| `registry/` | SQLite Schema v17 + CRUD：repos/repo_tags/code_symbols/code_embeddings/code_call_graph/code_symbol_links/oplog/vault_notes/papers/experiments/skills/skill_executions/entities/entity_types/relations/workflows/workflow_executions | Tier 0 (dirs) | **几乎所有上层** |
| `vault/fs_io` | Vault 文件读写原子操作 | 无 | vault/* |
| `vault/frontmatter` | YAML frontmatter 解析 | 无 | vault/scanner, skill_sync |
| `vault/wikilink` | `[[wikilink]]` 提取 | 无 | vault/scanner |
| `semantic_index` | tree-sitter 多语言符号提取（Rust/Python/TS/Go） | 无 | scan, mcp tools, search/hybrid |
| `embedding` | cosine_similarity、BLOB 序列化、query embedding 生成 | 无 | search/hybrid, mcp tools |
| `search` | Tantivy 索引初始化（id/title/content/tags/doc_type） | 无 | vault/indexer, search/hybrid, mcp tools |
| `arxiv` | arXiv API 元数据抓取 | 无 | mcp tools |

**关键决策**：registry 的 Schema 变更是**全局阻塞点**。任何新增表/字段必须：
1. 更新 `registry/migrate.rs` 的 `PRAGMA user_version`
2. 触发 `backup::auto_backup_before_migration()`
3. 同步修改 `oplog_analytics.rs` 的表存在性检查

---

### Tier 2 — 扫描与发现层

**目的**：将**无结构的外部世界**（文件系统、Git 仓库）转化为 registry 中的结构化数据。

| 模块 | 功能 | 上游依赖 | 下游产出 |
|:---|:---|:---|:---|
| `scan` | Git 仓库发现、语言检测、代码统计（tokei）、blake3 快照、自动注册 | registry, config | registry 各表 |
| `vault/scanner` | Vault 目录遍历、frontmatter/wikilink 提取、注册到 vault_notes | registry, vault/* | registry.vault_notes |
| `discovery_engine` | 跨仓库 Cargo.toml 依赖关联发现 | registry::RepoEntry | registry（待扩展） |
| `backup` | Schema 迁移前自动快照（保留 10 个） | registry | registry（文件系统） |
| `test_utils` | 测试辅助：临时 registry、mock repo | registry | 测试代码 |

**迭代策略**：scan 是数据入口，新增语言支持或检测规则在此层实验，不影响查询层。

---

### Tier 3 — 分析与关联层

**目的**：在已结构化数据上建立**语义关联**，为查询层提供"图"能力。

| 模块 | 功能 | 输入数据 | 产出 |
|:---|:---|:---|:---|
| `dependency_graph` | 跨仓库依赖图构建 | scan 发现的 Cargo.toml/package.json | 图结构（待深化） |
| `symbol_links` | Jaccard 签名相似度 + 同文件聚类 → code_symbol_links | registry.code_symbols | registry.code_symbol_links |
| `vault/indexer` | Vault 笔记入 Tantivy 全文索引 | vault_notes, search | Tantivy 索引文档 |
| `vault/backlinks` | Vault 反向链接图谱 | vault_notes.outgoing_links | 关联查询结果 |

**迭代策略**：symbol_links 的阈值（默认 0.3）和算法可在此层独立调优，不破坏下游 API。

---

### Tier 4 — 查询与报告层

**目的**：为上层（MCP/TUI）提供**只读查询接口**，是"知识库"的对外窗口。

| 模块 | 功能 | 依赖 |
|:---|:---|:---|
| `query` | 结构化表达式解析：`lang:rust stale:>30 behind:>10 tag:x` | registry |
| `health` | dirty/ahead/behind/diverged 计算、workspace 快照哈希 | registry, git2 |
| `oplog_analytics` | 知识覆盖报告：symbol/embedding/call 覆盖率、健康汇总、最近活动 | registry（多表聚合） |
| `search/hybrid` | RRF 归并：向量语义搜索 + Tantivy BM25 关键词 + 自动降级 | semantic_index, search, embedding |

**关键约束**：查询层必须保持**向后兼容**。MCP tool schema 的 breaking change 只能通过新增 tool（如 `_v2`）而非修改现有 tool。

---

### Tier 5 — 同步编排层

**目的**：将 registry 中的**期望状态**（upstream_url、tags、policy）与文件系统的**实际状态**对齐。

| 模块 | 功能 | 依赖关系 |
|:---|:---|:---|
| `sync/policy` | SyncPolicy / SyncMode / RepoSyncTask 定义、错误分类 | 独立（git2） |
| `sync/tasks` | 单仓库 fetch/pull/rebase/merge 执行、OpLog 写入 | registry::WorkspaceRegistry, policy |
| `sync/orchestrator` | 并发信号量控制、批量执行、超时处理、进度回调 | policy, tasks |
| `watch` | notify 文件系统监控、事件聚合 | sync_protocol |

**迭代策略**：sync 是**危险操作层**。新增 sync 策略（如 `SyncPolicy::StashThenRebase`）需先在 policy 层定义，再在 tasks 层实现，最后由 orchestrator 编排。

---

### Tier 6 — Skill Runtime 层

**目的**：实现 Skill 的**全生命周期管理**，是阶段二的核心交付物。

| 子模块 | 功能 | 依赖 |
|:---|:---|:---|
| `parser` | SKILL.md YAML frontmatter → SkillMeta | 独立 |
| `registry` | skills/skill_executions 表 CRUD、skill 安装/卸载 | registry::WorkspaceRegistry |
| `discover` | 项目类型检测（Rust/Node/Python/Go/Docker/Generic）→ SKILL.md 自动生成 + entry_script 包装 | parser |
| `dependency` | Skill 依赖图拓扑排序（Kahn）、DFS 环检测 | skill_runtime::registry |
| `executor` | Process-based 执行、interpreter 自动探测、timeout、stdout/stderr 捕获 | skill_runtime types |
| `scoring` | success_rate / usage_count / rating 计算（0–5 分公式） | registry + skill_runtime::registry |
| `publish` | validate → git tag → push remote | git2 + registry |
| `clarity_sync` | Skill 导出为 Clarity plan JSON | registry + skill_runtime types |

**依赖关系**：
```text
parser → discover
registry → dependency → executor
registry + executor → scoring
```

---

### Tier 7 — Workflow 引擎层

**目的**：将多个 Skill/子工作流编排为**可复用的自动化流程**。

| 子模块 | 功能 | 依赖 |
|:---|:---|:---|
| `model` | WorkflowDefinition / StepDefinition / StepType / ErrorPolicy | 独立 |
| `parser` | YAML → model（untagged 反序列化） | model |
| `validator` | 步骤 ID 唯一性、依赖存在性检查 | model |
| `interpolate` | `${inputs.x}` / `${steps.y.outputs.z}` 变量插值 | 独立 |
| `scheduler` | Kahn 拓扑排序 → ExecutionBatch（可并行步骤组） | model |
| `state` | workflow_executions 表 CRUD、状态机转换 | model + registry |
| `executor` | batch 并行执行（`std::thread::scope`）、错误策略（Fail/Continue/Retry/Fallback）、子工作流递归 | scheduler + model + skill_runtime::executor + registry |

**依赖关系**：
```text
model ← parser / validator / scheduler
scheduler → executor
skill_runtime::executor → executor
```

---

### Tier 8 — 高级聚合与跨层桥接

**目的**：连接原本正交的子系统，产生**涌现能力**。

| 模块 | 功能 | 跨层连接 |
|:---|:---|:---|
| `knowledge_engine` | block_on_async 安全封装、README 摘要提取、模块信息探测 | registry ↔ 外部进程 |
| `skill_sync` | Vault 笔记（ai_context=true）→ Clarity SKILL.md 格式导出 | vault ↔ skill_runtime |
| `vault/mod` | Vault 子系统统一出口 | 聚合 vault/* |

---

### Tier 9 — MCP 协议层

**目的**：将所有下层能力**封装为标准协议接口**，供 Clarity 等 AI Agent 调用。

| 子模块 | 对应 Tools | 依赖的下层模块 |
|:---|:---|:---|
| `tools/repo` | scan, health, sync, query_repos | scan, health, sync, query |
| `tools/query` | query, index, code_metrics, module_graph | query, scan, semantic_index |
| `tools/vault` | vault_search, vault_read, vault_write, vault_backlinks | vault/*, search |
| `tools/skill` | skill_list, skill_search, skill_run, skill_top | skill_runtime/* |
| `tools/context` | project_context, natural_language_query, hybrid_search, cross_repo_search, related_symbols, knowledge_report, embedding_store, embedding_search | search/hybrid, embedding, oplog_analytics, query |
| `mcp/mod` | 框架：McpTool trait、invoke_stream、ToolStreamEvent | 聚合上述所有 tools |

**关键契约**：
- 所有状态变更操作必须**幂等**（`ON CONFLICT ... DO UPDATE/NOTHING`）
- 所有批量操作包裹在 SQLite transaction 中
- Breaking change 通过新增 tool 实现，不修改现有 tool schema

---

### Tier 10 — TUI 交互层

**目的**：为人类开发者提供**终端仪表盘**。

| 子模块 | 功能 | 依赖 |
|:---|:---|:---|
| `theme` | Design Token（primary/secondary/error 等色彩语义） | 独立 |
| `layout` | 响应式布局计算（35/65 分栏、compact 模式、居中弹窗） | 独立 |
| `state` | App 状态机：仓库列表、Skill 面板、Workflow 弹窗、NLQ 输入、同步进度 | asyncgit, registry, sync::SyncOrchestrator, config |
| `event` | 键盘事件路由、异步通知消费（AsyncNotification） | state, render::ui |
| `render/detail` | 右侧详情页（Overview/Health/Insights） | state, theme, layout |
| `render/list` | 左侧仓库列表（状态图标、排序、过滤） | state, theme, layout |
| `render/popups` | 搜索/同步/Skill/Workflow/NLQ 弹窗 | state, theme, layout |
| `render/help` | 快捷键帮助覆盖层 | state, theme, layout |
| `render/logs` | 底部日志面板 | state, theme, layout |

**迭代策略**：TUI 是纯消费者层，新增面板（如 `render/loop_step.rs`）不改动任何下层逻辑。

---

### Tier 11 — 系统入口

| 模块 | 功能 | 依赖 |
|:---|:---|:---|
| `main.rs` | CLI 子命令解析（clap）、模块路由 | **所有上层** |
| `daemon.rs` | 定时 tick：stale repo health check、自动 sync | config, health, sync |

---

## 三、枢纽分析

### 高扇入模块（改动影响大，需谨慎）

| 模块 | 扇入来源 | 风险等级 |
|:---|:---|:---:|
| `registry::WorkspaceRegistry` | scan, health, backup, query, oplog_analytics, sync, skill_runtime, workflow, vault, mcp tools, tui | 🔴 |
| `i18n` | sync, digest, tui/render/* | 🟡 |
| `config` | scan, tui/state, sync, daemon | 🟡 |
| `skill_runtime::executor` | workflow/executor, mcp tools, tui | 🟡 |

### 叶节点模块（可独立迭代，影响面小）

| 模块 | 说明 | 实验优先级 |
|:---|:---|:---:|
| `arxiv` | 仅被 1 个 MCP tool 使用 | ⭐⭐⭐ |
| `semantic_index` | 仅被 scan 和 search/hybrid 使用，接口稳定 | ⭐⭐⭐ |
| `embedding` | 纯函数工具包，无副作用 | ⭐⭐⭐ |
| `vault/wikilink` | 解析规则可独立调优 | ⭐⭐ |
| `workflow/model` | 模型字段扩展不影响下层 | ⭐⭐ |

---

## 四、基于依赖的迭代策略

### 1. 根节点加固（Tier 0–1）
- **registry Schema** 的任何变更需经过：migration 脚本 → 备份逻辑 → oplog_analytics 表存在性检查 → MCP tool schema 兼容性审查
- **i18n** 新增字段需全语言覆盖，否则 TUI 会出现空字符串

### 2. 数据管道扩展（Tier 2–3）
- 新增语言支持（如 `tree-sitter-zig`）：在 `semantic_index` 扩展 → `scan` 触发索引 → `symbol_links` 自动关联 → `search/hybrid` 无需改动即可搜索
- 新增 Vault 笔记格式：在 `vault/frontmatter` 或 `vault/wikilink` 实验 → `vault/scanner` 消费 → `vault/indexer` 入 Tantivy

### 3. 查询能力叠加（Tier 4）
- `search/hybrid` 的 RRF 权重、Tantivy 与 embedding 的融合策略可独立调优
- `oplog_analytics` 的新报表类型只需读取 registry，不改动写入路径

### 4. 编排能力实验（Tier 5–7）
- **Skill Runtime** 和 **Workflow** 是相对独立的"应用层"，可并行开发
- Workflow 的 `StepType::Loop` 新增：只需在 `workflow/model` 加枚举 → `workflow/parser` 反序列化 → `workflow/executor` 实现循环逻辑 → 不影响 Skill Runtime

### 5. 协议与界面适配（Tier 9–10）
- 新增 MCP tool：在对应 `tools/*` 子模块实现，注册到 `mcp/mod.rs` 的 `McpToolEnum`，零改动下层业务逻辑
- 新增 TUI 面板：在 `render/` 新增子模块，由 `event.rs` 路由按键，零改动下层

---

## 五、与排期路线的对比

| 排期路线（时间线） | 拓扑路线（依赖线） | 差异说明 |
|:---|:---|:---|
| Wave 1–15b：数据层与索引 | Tier 0 → Tier 1 → Tier 2 → Tier 3 | 一致 |
| Wave 16–21：Skill Runtime | Tier 6（parser → registry → discover → dependency → executor → scoring） | 拓扑更细粒度展示子模块依赖 |
| Wave 22–25：Workflow Engine | Tier 7（model → parser → scheduler → executor） | 明确 Workflow 依赖 Skill Runtime executor |
| Wave 26–27：NLQ + Mind Market | Tier 4（search/hybrid）+ Tier 6（scoring） | NLQ 是查询层增强，Mind Market 是 Skill 层增强，二者**无依赖关系**，可并行 |
| Wave 28–33：风险修复与硬化 | 跨 Tier（unwrap 清零涉及 Tier 6–7） | 质量工作横向穿透所有层 |
| Future：L0–L4 知识模型 | 可能新增 Tier 3.5（知识图谱层）或扩展 Tier 1 Schema | 需先定义模型与 registry 的映射 |
| Future：跨设备同步 | Tier 5（sync）+ Tier 11（daemon）扩展 | 依赖 syncthing-rust 的 REST API |

---

*本文档作为架构决策参考，应与 `ARCHITECTURE.md` 和 `AGENTS.md` 同步维护。*
