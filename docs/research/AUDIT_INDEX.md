# docs 目录审计报告

> 审计日期：2026-04-23
> 代码版本：v0.2.3 (commit `2fc7872`)
> 审计人：Kimi Code CLI

---

## 当前项目状态（供对比）

| 维度 | 实际状态 |
|------|----------|
| **版本** | v0.2.3 |
| **MCP Tools** | 19 个 |
| **TUI** | 双模态（RepoList / VaultList） |
| **Vault** | 笔记系统 + 反向链接（backlinks）已落地 |
| **Repo-Vault 关联** | `vault_repo_links` 表 + TUI 联动已落地 |
| **Registry** | 已拆分（`migrate/repo/vault/links/health/metrics/workspace`） |
| **MCP Tools 组织** | 已拆分（`repo/vault/query/context` 四模块） |
| **安全** | openssl 0.10.77 → 0.10.78 已修复 |
| **测试** | src 内 153+ `#[test]`，零 warning（用户确认 159 全绿） |

---

## 审计结果总览

| 文档 | 状态 | 问题 | 建议 |
|------|------|------|------|
| mcp-5ire-integration.md | 部分过时 | 列出 13 个 tool，当前 19 个；缺少 Vault 相关 tool | 更新 |
| mcp-integration-guide.md | 部分过时 | 列出 12 个 tool；路线图说 module_graph/grep "规划中"，实际已落地 | 更新 |
| execution-plan.md | 严重过时 | Wave 1-4 甘特图为 2026-04-15 历史计划，大量任务已完成 | 归档 |
| roadmap-2026.md | 部分过时 | 写"MCP tool 数 11→15→20"，当前已达 19；code_metrics/module_graph 已落地 | 更新 |
| unified-analysis.md | 当前有效 | 战略分析无版本硬编码，双模态定位与代码一致 | 保留 |
| ai-infrastructure-analysis.md | 当前有效 | 赛道分析无版本硬编码，结论仍成立 | 保留 |
| competitive-risks.md | 当前有效 | 风险分析无具体版本号，五战路线仍为参考 | 保留 |
| competitive-roadmap-table-a.md | 当前有效 | 战略路线文档，无版本硬编码 | 保留 |
| competitive-analysis.md | 部分过时 | 表 B 写"MCP 工具数 5+"，当前 19；commit 为 `e857e27` | 更新 |
| architecture_audit_20260415.md | 部分过时 | 写"当前 10 个工具"；SSE 标记为严重缺陷（实际已边缘化）；Registry v5 评估基于旧 schema | 保留并标注历史 |
| smoke_test_report_20260418.md | 严重过时 | 版本 `main@76ccaf5`，48 passed；当前 v0.2.3 为 159 passed | 归档 |
| user_testing_guide.md | 严重过时 | 版本 `main@76ccaf5`，48 passed；tools/list 写 10 个 tool | 归档 |
| sprint_2_plan.md | 严重过时 | Sprint 2（04-18 ~ 05-01）所有 Task 已标记完成，属历史计划 | 归档 |
| memory_infrastructure_design.md | 部分过时 / 待完成 | 设计的 `devkit_query_memory/sync_memory/set_tier` 未实现；`workspace_snapshots` 仅部分落地；Vault 系统已替代部分记忆基础设施概念 | 保留，需更新与 Vault 的映射关系 |
| competitive_analysis_plan.md | 部分过时 | 写"现有 10 个 devkit 工具"，当前 19；Phase 1 状态需刷新 | 更新 |
| referenced_repos_report.md | 当前有效 | 静态参考报告，仓库信息为事实记录 | 保留 |
| mcp_contract.md | 严重过时 | v0.1 草案仅定义 4 个 tool（scan/health/sync/query），当前 19 个；MCP Server 实现已大幅演进 | 归档或重写为 v0.2 |
| STAGE_REPORT_2026-04-10.md | 严重过时 | 版本 0.1.0-beta，10 个 MCP tool；大量后续功能（Vault、拆分、安全修复）未反映 | 归档 |

---

## 详细说明

### 一、需要更新的文档（6 个）

#### 1. `mcp-5ire-integration.md`
- **核心问题**：工具清单只列了 13 个，缺少 `devkit_code_metrics`、`devkit_module_graph`、`devkit_natural_language_query` 以及 4 个 Vault tool（`vault_search/read/write/backlinks`）。
- **建议**：重写"5ire 中可用的 devbase Tool"章节，按当前 19 个 tool 重新分类（Repo / Vault / Query / Context）。

#### 2. `mcp-integration-guide.md`
- **核心问题**：
  - "可用 Tool 清单（12 个）"严重滞后。
  - 路线图说 `devkit_grep`、`devkit_module_graph` "规划中"，实际已实现（`module_graph` 在 `src/mcp/tools/repo.rs`，`grep` 能力由 TUI `/` 搜索和 `devkit_query_repos` 覆盖）。
- **建议**：更新工具清单为 19 个；刷新路线图状态；确认传输模式仍以 stdio 为主（SSE 已边缘化）。

#### 3. `roadmap-2026.md`
- **核心问题**：
  - 成功指标写"MCP tool 数 11 | Month 1 目标 15 | Month 3 目标 20"，当前已达 19。
  - Phase 2.1 `code_metrics` 和 Phase 2.2 `module_graph` 已落地。
- **建议**：更新基线数据；将已完成功能标记为 ✅；调整后续 Phase 目标。

#### 4. `competitive-analysis.md`
- **核心问题**：表 B 写"MCP 工具数 5+"，当前 19；版本引用为旧 commit `e857e27`。
- **建议**：更新功能对比表中的 tool 数量；刷新 commit 引用为当前 `2fc7872`。

#### 5. `competitive_analysis_plan.md`
- **核心问题**：
  - 多处写"现有 10 个 `devkit_*` 工具"。
  - Phase 1 状态列表需要刷新。
- **建议**：更新工具数量；标记已完成/放弃的 PoC。

#### 6. `memory_infrastructure_design.md`
- **核心问题**：
  - 设计的 `devkit_query_memory`、`devkit_sync_memory`、`devkit_set_tier` 尚未实现。
  - Vault 笔记系统（`devkit_vault_*`）已部分替代了原文中的"记忆基础设施"概念。
  - `workspace_snapshots` 表仅在 schema 中，openclaw/generic 工作区类型仍为设计阶段。
- **建议**：增加一节"与 Vault 系统的映射"，说明 Vault 是当前记忆基础设施的落地形态；更新待实现清单。

---

### 二、建议归档的文档（6 个）

| 文档 | 归档理由 |
|------|----------|
| `execution-plan.md` | 2026-04-15 的 Wave 1-4 甘特图和任务分解，所有 Wave 1 任务（grep、code_metrics、文档）早已完成，失去执行参考价值。 |
| `smoke_test_report_20260418.md` | 基于旧版本 `76ccaf5`（48 tests），当前 `2fc7872`（159 tests），测试项和 Bug 状态已全部过期。 |
| `user_testing_guide.md` | 同上，版本和测试数严重滞后；tool 数量不对；TUI 操作描述已随双模态重构变化。 |
| `sprint_2_plan.md` | Sprint 2（2026-04-18 ~ 05-01）全部 Task 已标记完成，作为历史计划保留价值低。 |
| `mcp_contract.md` | v0.1 草案仅定义 4 个 tool，当前 19 个，schema 和实现均已大幅演进。建议重写为 `mcp_contract_v0.2.md`。 |
| `STAGE_REPORT_2026-04-10.md` | 版本 0.1.0-beta，10 个 tool，大量后续功能未反映。作为历史里程碑归档。 |

> **归档建议操作**：在 docs 目录下新建 `archive/` 子目录，将上述 6 个文档移入，并在原位置放置 `.archived` 标记或重定向说明。

---

### 三、当前有效的文档（5 个）

| 文档 | 保留理由 |
|------|----------|
| `unified-analysis.md` | 双模态（Human TUI + AI MCP）战略定位，无版本硬编码，与当前代码一致。 |
| `ai-infrastructure-analysis.md` | AI 赛道分析，竞品关系图谱仍成立。 |
| `competitive-risks.md` | 风险矩阵和修正路线仍为有效参考。 |
| `competitive-roadmap-table-a.md` | 五战蚕食路线，战略文档不受功能数量变化影响。 |
| `referenced_repos_report.md` | 静态事实报告，仓库验证信息未过期。 |

---

### 四、需要特别关注的架构文档

#### `architecture_audit_20260415.md`
- **状态**：部分过时，但具有历史决策参考价值。
- **不一致点**：
  1. 写"当前 10 个工具"，实际 19 个。
  2. MCP SSE 被标记为"严重缺陷"，建议"移除"；实际上 SSE 虽未删除但已边缘化，stdio 为主路径。
  3. Registry v5 (12 tables) 的评估基于旧 schema，当前 Registry 已拆分为多模块，表结构已有变化。
- **建议**：保留原文，但在顶部增加 **历史标注**（"本报告基于 2026-04-15 的代码状态，部分结论已过时"），避免后人误读为当前架构状态。

---

## 整理行动清单

```
docs/
├── AUDIT_INDEX.md              ← 本报告（新增）
├── archive/                    ← 新建（建议）
│   ├── execution-plan.md
│   ├── smoke_test_report_20260418.md
│   ├── user_testing_guide.md
│   ├── sprint_2_plan.md
│   ├── mcp_contract.md
│   └── STAGE_REPORT_2026-04-10.md
├── mcp-5ire-integration.md     ← 待更新
├── mcp-integration-guide.md    ← 待更新
├── roadmap-2026.md             ← 待更新
├── competitive-analysis.md     ← 待更新
├── competitive_analysis_plan.md ← 待更新
├── memory_infrastructure_design.md ← 待更新
├── architecture_audit_20260415.md ← 加历史标注
├── unified-analysis.md         ← 保留
├── ai-infrastructure-analysis.md ← 保留
├── competitive-risks.md        ← 保留
├── competitive-roadmap-table-a.md ← 保留
└── referenced_repos_report.md  ← 保留
```

---

*审计完成。以上结论基于代码状态 v0.2.3 (2fc7872) 与 18 份文档的逐一对照。*
