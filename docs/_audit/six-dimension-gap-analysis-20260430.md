# devbase 六维信息模型 Gap Analysis

> **审计日期**：2026-04-30  
> **审计对象**：devbase v0.13.0（Local Context Compiler）  
> **审计范围**：MCP 工具层（38 tools）、CLI 路由层、数据库 Schema（v23/v25）、架构文档  
> **审计方法**：源码静态分析 + 测试覆盖检查 + CLI/MCP 对称性比对

---

## Executive Summary

devbase 的六维信息模型在**纸面架构**（`docs/architecture/context-compiler.md`）与**实际实现**之间存在显著落差。核心问题：

1. **`project_context` 并非真正的“编译端点”** — 它聚合了 7 类数据，但未触及 `relations` 统一关系表、`known_limits` 风险层、`experiments` 历史层，也无法跨项目工作。
2. **38 个 MCP tools 中仅 7 个有实际 invocation 测试**，其余 31 个仅为 schema 注册测试（`test_tools_list`）。
3. **CLI 与 MCP 严重不对称** — CLI 有 20+ 条独占命令，MCP 有 15+ 个独占工具，双向能力缺口大。
4. **`relations` 表已激活（v24）但无人使用** — 没有任何 MCP 工具或 CLI 命令查询该表，统一实体模型的图遍历能力为零。

---

## 维度一：Situation — “我房间里有什么书？”

### 1.1 现存数据源

| 数据源 | 表/文件 | 说明 |
|--------|---------|------|
| 仓库清单 | `entities` (`entity_type='repo'`) + `repo_tags` + `repo_remotes` | 统一实体模型存储 |
| Skill 清单 | `skills` 表（独立于 `entities`） | 运行时注册表 |
| Vault 笔记 | `entities` (`entity_type='vault_note'`) + `vault_repo_links` | Markdown 文件系统 + SQLite 索引 |
| 论文 | `entities` (`entity_type='paper'`) | PDF 元数据 |
| 工作流 | `entities` (`entity_type='workflow'`) | YAML 定义 |
| 模块结构 | `repo_modules` | Cargo target / 模块树 |
| 代码符号 | `code_symbols` | AST 提取的函数/结构体/枚举 |

### 1.2 MCP 工具暴露

- `devkit_scan` — 扫描目录并注册仓库
- `devkit_query_repos` — 按 language/tag/status 过滤仓库
- `devkit_query` — 通用结构化查询（lang:rust stale:>30）
- `devkit_natural_language_query` — NL 过滤仓库
- `devkit_vault_search` — 笔记关键词搜索（AND 逻辑，全文件读取）
- `devkit_skill_list` — Skill 列表
- `devkit_module_graph` — Rust 模块结构
- `devkit_code_symbols` — 符号索引查询
- `devkit_project_context` — 单项目聚合（见后文批判）

### 1.3 CLI 命令暴露

- `scan`, `query`, `vault list`, `skill list`, `workflow list`, `discover`

### 1.4 闭环能力

**部分闭环，但无统一 Workspace Snapshot。**

- 仓库维度：`devkit_query_repos` + `devkit_scan` 可闭环发现 + 列举。
- Vault 维度：`devkit_vault_search` 可闭环搜索。
- Skill 维度：`devkit_skill_list` 可闭环列举。
- **跨维度断层**：没有一个工具返回“当前 workspace 所有实体类型全景”。例如：Agent 无法通过一次调用得知“我有 4 个 repo、3 个 skill、12 篇笔记、2 个 workflow”。

### 1.5 缺失与破损

| 缺口 | 严重度 | 描述 |
|------|--------|------|
| **无 `devkit_workspace_snapshot`** | 🔴 高 | `entities` 表设计为统一存储，但没有工具能按 `entity_type` 聚合返回全 workspace 目录。 |
| **Skill 不在 `entities` 中** | 🟡 中 | `skills` 表是独立表，未纳入统一实体模型，导致 `relations` 表无法关联 skill→repo。 |
| **`devkit_vault_search` 性能隐患** | 🟡 中 | 实现为逐文件 `read_note_body` + 字符串 `contains`，无 Tantivy/BM25 加速，大 vault 下 O(n) 遍历。 |
| **无工作流 MCP 工具** | 🟡 中 | `workflow` 实体有 CLI 命令（`workflow list/run/register`），但无任何 MCP 工具暴露。 |

---

## 维度二：State — “哪些书还没读完？”

### 2.1 现存数据源

| 数据源 | 表/文件 | 说明 |
|--------|---------|------|
| Git 健康状态 | `repo_health` + `entities.metadata` | dirty/ahead/behind/diverged |
| 代码指标 | `repo_code_metrics` | 行数/语言分布/文件数 |
| 索引覆盖度 | `code_symbols` 计数 + `code_embeddings` 存在性 | 符号是否被索引、是否有 embedding |
| 调用图密度 | `code_call_graph` 边数 | 可计算 but 未暴露 |
| 操作审计 | `oplog` | 最近 scan/index/sync 事件 |
| 已知限制 | `known_limits` | Hard Veto / Known Bug |
| 环境状态 | `health::analyze_repo` 运行时检测 | Rust/Go/Node 版本 |

### 2.2 MCP 工具暴露

- `devkit_health` — 返回仓库 Git 状态 + 环境检查
- `devkit_code_metrics` — 返回代码统计
- `devkit_knowledge_report` — 返回符号数/embedding 数/调用边数/activity
- `devkit_oplog_query` — 返回操作日志
- `devkit_known_limit_list` — 返回已知限制

### 2.3 CLI 命令暴露

- `health`, `oplog`, `digest`, `limit list`

### 2.4 闭环能力

**半闭环 — 数据分散在多个工具中，无统一仪表盘。**

- `devkit_health` 仅覆盖 Git 状态，不报告索引新鲜度（“上次索引是何时？”）。
- `devkit_knowledge_report` 覆盖索引统计，但不报告 Git 健康。
- `devkit_oplog_query` 覆盖事件历史，但默认仅返回原始日志，无状态推导（“哪些 repo 超过 7 天未索引？”）。

### 2.5 缺失与破损

| 缺口 | 严重度 | 描述 |
|------|--------|------|
| **无 Index Freshness 检查** | 🔴 高 | `devkit_health` 不检查 `repo_modules`/`code_symbols`/`code_call_graph` 的生成时间。Agent 无法判断“这个 repo 的符号索引是否过期”。 |
| **`devkit_knowledge_report` 未测** | 🟡 中 | 38 tools 中唯一在 `test_tools_list` 里被断言存在，但无任何 invocation 测试的 Experimental 级工具。 |
| **无统一状态仪表盘** | 🟡 中 | 没有“Workspace State Compiler”将 Git 健康 + 索引覆盖 + 已知限制 + oplog 聚合成一张快照。 |
| **`repo_health` 与 `entities` 不同步风险** | 🟡 中 | `repo_health` 是独立表，`entities.metadata` 也含 `last_synced_at`，双源存储可能导致不一致。 |

---

## 维度三：Relations — “这本书引用了哪本？”

### 3.1 现存数据源

| 数据源 | 表/文件 | 说明 |
|--------|---------|------|
| 统一关系图 | `relations` (v24 激活) | `from_entity_id` → `to_entity_id`，有向图 |
| 代码调用图 | `code_call_graph` | 函数级调用边 |
| 概念符号链接 | `code_symbol_links` | 相似签名 / 同文件关联 |
| 跨仓库依赖 | `dependency_graph` 模块 + `repo_relations` (legacy) | Cargo.toml/package.json/go.mod 解析 |
| Vault 反链 | `vault` 文件系统 wikilink | `[[note-id]]` 语法 |
| 仓库-笔记链接 | `vault_repo_links` | 显式双向链接 |

### 3.2 MCP 工具暴露

- `devkit_dependency_graph` — 跨仓库依赖（outgoing/incoming）
- `devkit_call_graph` — 函数调用图（caller/callee）
- `devkit_related_symbols` — 概念关联符号
- `devkit_vault_backlinks` — Vault 笔记反链
- `devkit_project_context` — 包含 `related_symbols`（对 top symbols 的有限关联）

### 3.3 CLI 命令暴露

- `discover` — 自动发现 repo 间依赖和相似性，写入 `relations`（legacy `repo_relations`）

### 3.4 闭环能力

**严重断裂 — `relations` 表存在但完全不可访问。**

- `devkit_dependency_graph` 查询的是 `dependency_graph` 模块（manifest 解析），**不查询 `relations` 表**。
- `devkit_project_context` 的 `related_symbols` 来自 `code_symbol_links` + `find_related_symbols`，**不查询 `relations` 表**。
- `devkit_call_graph` 查询 `code_call_graph`，**不查询 `relations` 表**。
- **没有任何工具可以执行：`SELECT * FROM relations WHERE from_entity_id = ?`**。

### 3.5 缺失与破损

| 缺口 | 严重度 | 描述 |
|------|--------|------|
| **`relations` 表零暴露** | 🔴 高 | 架构文档声称 v24 已激活并迁移 `repo_relations` → `relations`，但 38 个 tools 中没有任何一个查询 `relations`。统一实体模型的核心优势（“新增概念无需改表”）因缺乏查询端点而被架空。 |
| **无通用图遍历工具** | 🔴 高 | 没有 `devkit_relations_query` 或 `devkit_entity_neighbors` 工具。Agent 无法问“与这个 skill 相关的 repo 有哪些？” |
| **`devkit_project_context` 关系不完整** | 🟡 中 | 仅返回 `related_symbols`（代码级），不返回 `relations` 表中的 repo→repo、repo→paper、repo→workflow 关系。 |
| **Vault 反链非实时** | 🟡 中 | `devkit_vault_backlinks` 每次调用都重新 `build_backlink_index`，无缓存，大 vault 性能差。 |
| **CLI `discover` 无 MCP 镜像** | 🟡 中 | `discover` 命令能自动推断并保存关系，但 MCP 侧没有对应工具，Agent 无法触发关系发现。 |

---

## 维度四：Capability — “我可以用笔划线、用书签标记”

### 4.1 现存数据源

| 数据源 | 表/文件 | 说明 |
|--------|---------|------|
| Skill 目录 | `skills` 表 | 已安装 skill 元数据 |
| Skill 执行历史 | `skill_executions` | 运行时记录 |
| Skill 评分 | `skill_scores` | 成功率/评分 |
| Workflow 定义 | `workflows` 表 + `entities` | 多步骤编排 |
| MCP 工具注册表 | `mcp::McpServer` 运行时 | 38 个 tools |

### 4.2 MCP 工具暴露

- `devkit_skill_list` — 列举 skill
- `devkit_skill_search` — 文本搜索 skill
- `devkit_skill_run` — 执行 skill（destructive gate）
- `devkit_skill_discover` — 将项目封装为 skill（destructive gate）
- **38 个 tools 本身** — 即 capability 的具象化

### 4.3 CLI 命令暴露

- `skill list`, `skill search`, `skill run`, `skill discover`, `skill info`, `skill install`, `skill uninstall`, `skill validate`, `skill publish`, `skill sync`, `skill recalc-scores`, `skill top`, `skill recommend`
- `workflow list`, `workflow show`, `workflow run`, `workflow register`, `workflow delete`

### 4.4 闭环能力

**Skill 维度闭环，Workflow 维度完全缺失。**

- Skill 发现 → 搜索 → 执行：`devkit_skill_list` → `devkit_skill_search` → `devkit_skill_run` 形成完整闭环。
- **Workflow 零暴露**：Agent 无法通过 MCP 知道有哪些 workflow、无法触发 workflow 执行。

### 4.5 缺失与破损

| 缺口 | 严重度 | 描述 |
|------|--------|------|
| **无 Workflow MCP 工具集** | 🔴 高 | `workflow` 是 `entities` 中定义的内置类型，有完整 CLI 命令集，但 MCP 侧没有任何 workflow 工具（list/show/run/register/delete）。 |
| **无 Skill 安装/卸载 MCP 工具** | 🟡 中 | CLI 有 `skill install <git-url>`、`skill uninstall <id>`，MCP 侧没有。Agent 无法通过对话安装新 skill。 |
| **无 Skill 评分/推荐 MCP 工具** | 🟡 中 | CLI 有 `skill top`、`skill recommend`、`skill recalc-scores`，MCP 侧没有。Agent 无法获取“最适合当前任务的 skill”。 |
| **无 Skill 详情 MCP 工具** | 🟡 中 | CLI 有 `skill info` 返回完整元数据（author/inputs/outputs/dependencies），MCP 仅有 `skill_list` 返回基础字段。 |
| **Capability 未在 `project_context` 中体现** | 🟡 中 | `project_context` 返回 repo + vault + symbols，但不返回“该项目可用的 skill 或 workflow 有哪些”。 |

---

## 维度五：History — “上次读这本书到哪一章？”

### 5.1 现存数据源

| 数据源 | 表/文件 | 说明 |
|--------|---------|------|
| 操作日志 | `oplog` | scan/index/sync/health 等事件 |
| Agent 符号读取历史 | `agent_symbol_reads` (v25 激活) | 行为信号，用于 relevance boosting |
| Skill 执行历史 | `skill_executions` | stdout/stderr/exit_code/duration |
| 实验记录 | `experiments` 表 | 实验配置与结果 |
| 已知限制历史 | `known_limits` | first_seen_at / last_checked_at / mitigated |

### 5.2 MCP 工具暴露

- `devkit_oplog_query` — 查询操作日志（全局或按 repo）
- `devkit_project_context` — 包含最近 10 条 oplog（activity 字段）
- `devkit_experiment_log` — 记录实验（write-only）
- `devkit_known_limit_list` — 查询已知限制

### 5.3 CLI 命令暴露

- `oplog`, `digest`, `limit list`

### 5.4 闭环能力

**半闭环 — 写得多，读得少。**

- `devkit_oplog_query` 提供全局/按 repo 的日志读取，是完整闭环。
- `agent_symbol_reads` 被 `project_context` **写入**（记录 symbol 读取），但**没有任何工具可以读取该历史**。Agent 无法问“我最近常看哪些函数？”。
- `devkit_experiment_log` 是 write-only；没有 `devkit_experiment_list` 或 `devkit_experiment_query`。
- `skill_executions` 表有数据，但 MCP 侧无查询工具。

### 5.5 缺失与破损

| 缺口 | 严重度 | 描述 |
|------|--------|------|
| **`agent_symbol_reads` 只写不读** | 🔴 高 | v25 引入的行为信号表，仅用于 `hybrid_search_symbols` 的内部 boosting，未向 Agent 暴露。丧失了“记忆我看过什么”的核心价值。 |
| **无 `devkit_experiment_list` 工具** | 🟡 中 | `devkit_experiment_log` 可写，但无对应读取工具。Agent 无法审计实验历史。 |
| **无 Skill 执行历史 MCP 工具** | 🟡 中 | `skill_executions` 有详细记录，但 MCP 侧无查询接口。 |
| **`devkit_digest` 未测试** | 🟡 中 | 生成每日摘要，但无 invocation 测试，且其输出未与 `project_context` 集成。 |
| **`project_context` 的 activity 只有 10 条** | 🟢 低 | 硬编码 limit=10，无参数可调整，对于高频操作 repo 可能丢失关键上下文。 |

---

## 维度六：Relevance — “我现在要写论文，哪些书相关？”

### 6.1 现存数据源

| 数据源 | 表/文件 | 说明 |
|--------|---------|------|
| 代码 Embedding | `code_embeddings` | 符号级向量 |
| 混合搜索索引 | `hybrid_search_symbols` (RRF 合并) | 向量相似度 + 关键词 BM25 |
| 符号读取计数 | `agent_symbol_reads` | 行为 boosting |
| Vault 内容 | Markdown 文件系统 | 全文但无向量索引 |
| Tantivy 索引 | `search::index` | repo 摘要/关键词/模块结构 |

### 6.2 MCP 工具暴露

- `devkit_project_context` (with `goal`) — 按 goal 对 symbols 做 hybrid search relevance ranking
- `devkit_hybrid_search` — 符号级混合搜索（向量+关键词 RRF）
- `devkit_semantic_search` / `devkit_embedding_search` — 纯向量搜索
- `devkit_related_symbols` — 概念关联符号
- `devkit_cross_repo_search` — 跨仓库混合搜索
- `devkit_natural_language_query` — NL 过滤仓库（非 relevance rank）

### 6.3 CLI 命令暴露

- 无直接对应命令（Relevance 是 MCP 层独有概念）

### 6.4 闭环能力

**代码符号维度闭环，Vault/跨实体维度断裂。**

- 代码搜索：`devkit_hybrid_search` → `devkit_related_symbols` → `devkit_cross_repo_search` 形成符号发现闭环。
- **Vault 无 Relevance**：`devkit_vault_search` 是纯关键词 AND 匹配，无 embedding、无 goal-based ranking。Agent 无法问“与我的目标相关的笔记有哪些？”。
- **跨实体无 Relevance**：没有工具能回答“给定 goal，哪些 repo + 哪些 vault notes + 哪些 skills 最相关？”。

### 6.5 缺失与破损

| 缺口 | 严重度 | 描述 |
|------|--------|------|
| **无 Vault Embedding/Relevance** | 🔴 高 | Vault 笔记是跨会话记忆的核心载体，但搜索仍停留在字符串 contains 阶段，无法与代码符号的 hybrid search 对齐。 |
| **`project_context` 的 `goal` 仅影响 symbols** | 🟡 中 | 传入 `goal` 后，仅 `symbols` 和 `calls` 被 relevance 过滤/排序，`vault_notes` 和 `assets` 不受影响。 |
| **无 Workspace-wide Goal Match** | 🟡 中 | `devkit_project_context` 要求传入 `project` 参数，无法做 workspace 级别的“给定 goal，返回最相关的实体（任意类型）”。 |
| **`agent_symbol_reads` boosting 不透明** | 🟡 中 | `project_context` 在无 `goal` 时使用 `agent_symbol_reads` 做 behavioral boosting，但 boost 值（0.05×count, max 0.5）未向 Agent 披露，形成黑盒排序。 |
| **`devkit_natural_language_query` 非真正语义** | 🟢 低 | 实现为硬编码 regex 过滤（language keywords + stars parse + status keywords），无 LLM/embedding 参与，名称具有误导性。 |

---

## 横向审计 A：MCP 工具测试覆盖

### 实际有 Invocation 测试的工具（7 / 38）

| 工具名 | 测试函数 | 备注 |
|--------|----------|------|
| `devkit_health` | `test_tools_call_devkit_health` | ✅ |
| `devkit_query` | `test_tools_call_devkit_query` | ✅ |
| `devkit_project_context` | `test_tools_call_devkit_project_context` | 仅测空项目 case |
| `devkit_arxiv_fetch` | `test_tools_call_devkit_arxiv_fetch` | 测空 ID error case |
| `devkit_skill_list` | `test_tools_call_devkit_skill_list` | ✅ |
| `devkit_skill_search` | `test_tools_call_devkit_skill_search` | ✅ |
| `devkit_skill_discover` | `test_tools_call_devkit_skill_discover` | dry_run 模式 |
| `devkit_skill_run` | `test_tools_call_devkit_skill_run` | ⚠️ `#[ignore]` |

### 无任何 Invocation 测试的工具（31 / 38）

```
devkit_scan, devkit_sync, devkit_index, devkit_note, devkit_digest,
devkit_paper_index, devkit_experiment_log, devkit_github_info,
devkit_code_metrics, devkit_module_graph, devkit_query_repos,
devkit_natural_language_query, devkit_code_symbols, devkit_dependency_graph,
devkit_call_graph, devkit_dead_code, devkit_semantic_search,
devkit_embedding_store, devkit_embedding_search, devkit_vault_search,
devkit_vault_read, devkit_vault_write, devkit_vault_backlinks,
devkit_cross_repo_search, devkit_knowledge_report, devkit_related_symbols,
devkit_hybrid_search, devkit_known_limit_store, devkit_known_limit_list,
devkit_oplog_query
```

> **风险**：这些工具的 schema 与实际实现可能已发生漂移（schema 承诺了字段，但 `invoke` 可能未返回），且运行时 panic 无测试捕获。

---

## 横向审计 B：CLI ↔ MCP 对称性

### CLI 独占命令（无 MCP 等价工具）

| CLI 命令 | 功能 | 缺口影响 |
|----------|------|----------|
| `clean` | 删除备份实体 | Agent 无法清理注册表垃圾 |
| `tag <repo> <tags>` | 为仓库打标签 | Agent 无法动态标签分类 |
| `meta --tier/--workspace-type` | 更新元数据 | Agent 无法修改数据分级 |
| `discover` | 自动发现 repo 关系 | Agent 无法触发关系发现 |
| `skill install/uninstall/info/validate/publish/sync/recalc-scores/top/recommend` | Skill 生命周期管理 | Agent 无法完整管理 skill |
| `workflow list/show/run/register/delete` | 工作流引擎 | **Agent 完全无法使用 workflow** |
| `limit add/resolve/delete/seed` | 已知限制管理 | Agent 无法 resolve/delete limit |
| `registry export/import/backups/clean` | 备份恢复 | Agent 无法备份 |
| `vault scan/reindex` | Vault 索引维护 | Agent 无法重建 vault 索引 |
| `syncthing-push` | P2P 同步 | Agent 无法触发 |
| `skill-sync` | 同步到 Clarity | Agent 无法触发 |

### MCP 独占工具（无 CLI 等价命令）

| MCP 工具 | 功能 | 缺口影响 |
|----------|------|----------|
| `devkit_cross_repo_search` | 跨仓库符号搜索 | 人类用户无法从命令行使用 |
| `devkit_related_symbols` | 概念关联符号 | 人类用户无法从命令行使用 |
| `devkit_hybrid_search` | 混合搜索 | 人类用户无法从命令行使用 |
| `devkit_semantic_search` / `devkit_embedding_search` | 向量搜索 | 人类用户无法从命令行使用（CLI skill search 有语义模式，但仅针对 skill） |
| `devkit_call_graph` | 函数调用图 | 人类用户无法从命令行使用 |
| `devkit_dead_code` | 死代码检测 | 人类用户无法从命令行使用 |
| `devkit_code_symbols` | 符号查询 | 人类用户无法从命令行使用 |
| `devkit_dependency_graph` | 依赖图 | 人类用户无法从命令行使用 |
| `devkit_knowledge_report` | 知识覆盖报告 | 人类用户无法从命令行使用 |
| `devkit_note` | 为 repo 添加短笔记 | 人类用户无法从命令行使用 |
| `devkit_github_info` | GitHub API 查询 | 人类用户无法从命令行使用 |
| `devkit_experiment_log` | 记录实验 | 人类用户无法从命令行使用 |
| `devkit_paper_index` | 索引论文 | 人类用户无法从命令行使用 |
| `devkit_module_graph` | 模块图 | 人类用户无法从命令行使用 |
| `devkit_code_metrics` | 代码指标 | 人类用户无法从命令行使用 |

---

## 横向审计 C：`project_context` 是否是真·编译端点？

### 它聚合了什么（✅）

| 维度 | 数据来源 | 字段 |
|------|----------|------|
| Repo 元数据 | `entities` + `repo_tags` | `repo` |
| Vault 笔记 | `vault_repo_links` + 关键词匹配 | `vault_notes` |
| 模块结构 | `repo_modules` | `modules` |
| 代码符号 | `code_symbols` (LIMIT 50) / `hybrid_search_symbols` | `symbols` |
| 调用边 | `code_call_graph` (LIMIT 50) | `calls` |
| 操作历史 | `oplog` (LIMIT 10) | `activity` |
| 概念关联 | `code_symbol_links` | `related_symbols` |
| 资产文件 | 文件系统 `assets/` 目录 | `assets` |

### 它没有聚合什么（❌）

| 维度 | 缺失数据 | 影响 |
|------|----------|------|
| **Relations 表** | 不查询 `relations` | 无法展示 repo→repo、repo→skill、skill→workflow 关系 |
| **Known Limits** | 不查询 `known_limits` | Agent 不知道该项目是否有 Hard Veto |
| **Experiments** | 不查询 `experiments` | Agent 不知道该项目是否有正在进行的实验 |
| **Skills** | 不查询 `skills` | Agent 不知道有哪些 skill 可用 |
| **Workflows** | 不查询 `workflows` | Agent 不知道有哪些 workflow 可用 |
| **State/Health** | 不查询 `repo_health` | 不返回 dirty/ahead/behind 状态 |
| **Index Coverage** | 不查询 `code_embeddings` | 不返回“有多少符号已 embedding” |
| **Agent Symbol Reads** | 写入但不读取 | 不向 Agent 暴露“你之前看过这些符号” |
| **Vault Relevance** | 关键词匹配 only | `goal` 参数不影响 vault_notes 排序 |

###  verdict

`project_context` 是一个**局部编译器**（Partial Compiler），而非架构文档宣称的“统一情境编译端点”。它编译了代码层 + 笔记层 + 活动层的子集，但：

1. **漏掉了统一实体模型的核心——`relations` 图**。
2. **漏掉了 Capability 层**（skill/workflow）。
3. **漏掉了 Risk 层**（known limits）。
4. **漏掉了 History 层的可读面**（agent_symbol_reads）。
5. **仅支持单项目模式**，无法做 workspace 级编译。

---

## 关键发现汇总

| # | 发现 | 严重度 | 建议修复 |
|---|------|--------|----------|
| 1 | `relations` 表零暴露 — 统一实体模型的图遍历能力未实现 | 🔴 | 新增 `devkit_relations_query` 工具；CLI `discover` 增加 MCP 镜像 |
| 2 | `project_context` 不是真·编译端点 — 缺 Relations/Capability/Risk/History | 🔴 | 扩展 `project_context` 或新增 `devkit_workspace_context`；纳入 `relations`、`known_limits`、`skills`、`workflows` |
| 3 | Workflow 引擎零 MCP 暴露 | 🔴 | 新增 `devkit_workflow_list/run/register/delete` 工具集 |
| 4 | 38 tools 中 31 个无任何 invocation 测试 | 🔴 | 为核心 tools 补充集成测试（至少 Stable + Beta tier） |
| 5 | `agent_symbol_reads` 只写不读 | 🔴 | 新增 `devkit_symbol_read_history` 工具；或在 `project_context` 中暴露读取历史 |
| 6 | Vault 搜索无 relevance/embedding | 🟡 | 为 Vault 笔记建立 Tantivy/embedding 索引；`devkit_vault_search` 支持 `goal` 参数 |
| 7 | CLI 与 MCP 严重不对称 | 🟡 | 对双向独占功能做对称补全；优先补全 Workflow MCP 和 Skill 生命周期 MCP |
| 8 | `devkit_natural_language_query` 名不副实 | 🟡 | 重命名或接入真正的 LLM/embedding 语义解析 |
| 9 | `devkit_health` 无索引新鲜度 | 🟡 | 增加 `last_indexed_at`、`symbol_count`、`embedding_coverage` 字段 |
| 10 | `devkit_vault_search` 全文件 O(n) 扫描 | 🟡 | 使用 Tantivy reader 替代逐文件读取 |

---

## 附录：数据源 ↔ 工具映射矩阵

| 数据源 | Situation | State | Relations | Capability | History | Relevance |
|--------|-----------|-------|-----------|------------|---------|-----------|
| `entities` (repos) | ✅ scan/query/query_repos/nlq | ✅ health | ❌ 无工具 | ❌ 无工具 | ❌ 无工具 | ⚠️ nlq 仅过滤 |
| `entities` (vault) | ✅ vault_search | ❌ 无 | ⚠️ backlinks | ❌ 无 | ❌ 无 | ⚠️ 关键词 only |
| `entities` (workflow) | ❌ 无 MCP | ❌ 无 | ❌ 无 | ❌ 无 | ❌ 无 | ❌ 无 |
| `skills` | ✅ skill_list | ❌ 无 | ❌ 无 | ✅ skill_list/search/run | ⚠️ 执行历史无暴露 | ❌ 无 |
| `relations` | ❌ **零暴露** | ❌ 无 | ❌ **零暴露** | ❌ 无 | ❌ 无 | ❌ 无 |
| `code_symbols` | ✅ code_symbols | ✅ knowledge_report | ⚠️ related_symbols | ❌ 无 | ❌ 无 | ✅ hybrid_search |
| `code_call_graph` | ❌ 无直接暴露 | ✅ knowledge_report | ✅ call_graph | ❌ 无 | ❌ 无 | ❌ 无 |
| `code_embeddings` | ❌ 无 | ✅ knowledge_report | ❌ 无 | ❌ 无 | ❌ 无 | ✅ semantic_search |
| `agent_symbol_reads` | ❌ 无 | ❌ 无 | ❌ 无 | ❌ 无 | ❌ **只写不读** | ⚠️ 内部 boosting |
| `oplog` | ❌ 无 | ✅ health/summary | ❌ 无 | ❌ 无 | ✅ oplog_query | ❌ 无 |
| `known_limits` | ❌ 无 | ✅ known_limit_list | ❌ 无 | ❌ 无 | ✅ known_limit_list | ❌ 无 |
| `experiments` | ❌ 无 | ❌ 无 | ❌ 无 | ❌ 无 | ❌ **只写不读** | ❌ 无 |

---

*Report generated by codebase exploration agent. All table names, tool names, and file paths verified against `src/` at commit `d0eb774`.*
