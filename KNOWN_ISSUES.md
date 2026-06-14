# Known Issues & Technical Debt

> 本文件记录 devbase 的已知问题、技术债务和架构 blockers。
> 不是 bug 列表 — 这些问题是设计层面的权衡或待完成的工作。

---

## P0 — 阻塞发布

无当前 P0 blocker。v0.20.1 已发布，所有 P0 架构 gaps 已关闭。

---

## P1 — 测试覆盖

### MCP 工具 invocation 测试覆盖不均

**现状**：71 个工具中，约 45 个有 dedicated `invoke()` 测试，其余以 name/schema smoke tests 或间接覆盖为主。Stable 工具已实现 invocation 测试补全。

**影响**：Beta → Stable 的 promote 需要测试背书；无测试的工具在重构时存在回归风险。

**缺失测试的工具清单**：

| 工具 | Tier | 已有覆盖 |
|------|------|----------|
| `devkit_digest` | Experimental | 无 |
| `devkit_paper_index` | Experimental | 无 |
| `devkit_embedding_store` | Beta | 无 |
| `devkit_embedding_search` | Beta | 无 |
| `devkit_impact_analysis` | Beta | 无 |
| `devkit_project_brief` | Beta | 间接（scenario） |
| `devkit_session_*` × 13 | Beta/Exp | 部分 smoke；save/list/resume 已有覆盖 |

**已补充 dedicated invocation 测试**：
- `devkit_index_health`
- `devkit_index_stream`
- `devkit_note`
- `devkit_evaluate`
- `devkit_ontology_import`
- `devkit_search_quality`
- `devkit_related_symbols`
- `devkit_knowledge_report`
- `devkit_experiment_log`

**建议**：按调用频率排序，持续为 Embedding 相关工具、ImpactAnalysis、ProjectBrief、Session 记忆召回添加 dedicated 测试。

---

## P2 — 架构债务

### ~~`mcp/tools/repo.rs` 730 行~~

**现状**：~~已从 2376 行拆至 730 行，但仍超过理想阈值（~300 行/模块）。~~

**结果**：**已完成** — 拆分为 `repo/{scan,health,sync,index,query_repos,nl_query}.rs`，入口 `repo.rs` 降至 ~100 行。计划详见 `docs/architecture/split-plan.md`。

### `src/mcp/mod.rs` 工具枚举集中化

**现状**：`McpToolEnum` 是包含 71 个变体的 giant enum，`tier()` 方法是 200+ 行的 match 表达式。

**影响**：新增工具需要修改 3 处（enum 定义、match arm、tier match），容易遗漏。

**建议**：考虑使用宏或 derive 自动生成 `McpToolEnum` 和 `tier()`，减少 boilerplate。

---

## P3 — 文档与可观测性

当前无活跃 P3 债务。

## 已解决（归档）

| 问题 | 解决版本 | Commit / 实现 |
|------|----------|---------------|
| Vault 笔记全文搜索性能 | Unreleased | `devkit_vault_search` 优先使用 Tantivy BM25（`search_vault_at`），回退内存扫描；`reindex_vault_with_writer` 与仓库索引同 writer |
| 性能基准缺失 | Unreleased | 新增 `benches/vault_bench.rs`，保存 Criterion baseline `v0.20.1` |
| `relations` 表零生产读取路径 | v0.20.1 | `devkit_relation_store/query/delete` + `project_context` 读取 |
| Workflow 引擎零 MCP 暴露 | v0.20.1 | `devkit_workflow_list/run/status` |
| `project_context` 不完整 | v0.20.1 | 补充 `known_limits` + `skills` |
| `mcp/tools/repo.rs` 2376 行 | v0.20.1 | 拆分为 `tools/` 目录，repo.rs 730 行 |
| `init_db_at` 1214 行 | v0.20.1 | 拆分为 `registry/migrate.rs`（503 行）+ 子模块 |
| 工具数量文档不一致 | v0.20.1 | `mcp-tools.md` 全面更新至 71 个 |
| 3 Stable 工具缺 invocation tests | v0.20.1 | `query_repos`, `vault_search`, `vault_read` 测试 added |
| `devkit_document_convert` 工具缺失 | v0.21.0 | `src/mcp/tools/document_convert.rs` + MCP 注册 |

---

*Last updated: 2026-06-13*
