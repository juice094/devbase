# devbase 文档导航

> **项目状态**：v0.10.0 已交付 / v0.11.0 规划中  
> **主入口**：[`AGENTS.md`](../AGENTS.md)（Agent 环境指引）· [`ROADMAP.md`](ROADMAP.md)（功能路线图）  
> **最后整理**：2026-04-26

---

## 快速跳转

| 你想了解 | 去这里 |
|---------|--------|
| 项目背景、架构红线、技术债 | [`AGENTS.md`](../AGENTS.md) |
| 功能阶段、发布计划、已完成里程碑 | [`ROADMAP.md`](ROADMAP.md) |
| 如何配置 MCP（Claude Code / 5ire） | [`guides/mcp-integration-guide.md`](guides/mcp-integration-guide.md) |
| Vault 笔记格式规范 | [`guides/VAULT_FORMAT_SPEC.md`](guides/VAULT_FORMAT_SPEC.md) |
| Workflow DSL 规范 | [`architecture/workflow-dsl.md`](architecture/workflow-dsl.md) |
| 统一实体模型设计 | [`architecture/workspace-as-schema.md`](architecture/workspace-as-schema.md) |

---

## 文档目录

### 🗺️ 路线与规划（Roadmaps & Plans）

| 文档 | 状态 | 说明 |
|------|------|------|
| [`ROADMAP.md`](ROADMAP.md) | 🟢 活跃 | 唯一活跃主路线图。Phase 1–9 全记录，v0.10.0 已交付，v0.11.0 待定 |
| [`plans/roadmap-2026.md`](plans/roadmap-2026.md) | 🔴 归档 | 版本 v0.2.3（2026-04-23），内容严重过时。保留为历史参考 |
| [`plans/skill-runtime.md`](plans/skill-runtime.md) | 🟡 历史 | Waves 16–20 设计记录，Skill Runtime 已全量实现 |
| [`plans/l0-l4-knowledge-model.md`](plans/l0-l4-knowledge-model.md) | 🟡 部分实现 | L3/L4 已交付（Wave 35–36），生长信号与遗忘机制仍为草案 |
| [`plans/sse-daemon-design.md`](plans/sse-daemon-design.md) | 🟡 草案 | SSE 传输设计，未进入实现，stdio 仍为主路径 |
| [`plans/personal-knowledge-graph.md`](plans/personal-knowledge-graph.md) | 🟡 部分实现 | v0.4.0 规划，NLQ/跨仓库搜索/对比等功能已落地 |
| [`plans/tui-skill-integration.md`](plans/tui-skill-integration.md) | 🔴 过时 | 版本 v0.2.4，TUI Skill 集成已在 Waves 25–33 中实现 |

### 🏗️ 架构设计（Architecture）

| 文档 | 状态 | 说明 |
|------|------|------|
| [`architecture/workflow-dsl.md`](architecture/workflow-dsl.md) | 🟢 活跃 | Workflow DSL v0.4.0 规范，Engine 已实现 |
| [`architecture/workspace-as-schema.md`](architecture/workspace-as-schema.md) | 🟡 思考中 | Workspace 作为 Schema 的架构思考，未进入实现 |
| [`architecture/pre-split-evaluation.md`](architecture/pre-split-evaluation.md) | 🟢 活跃 | 单 crate vs 多 crate 评估，结论：22.7 KLOC 单 crate 仍最优 |
| [`architecture/dependency-topology-plan.md`](architecture/dependency-topology-plan.md) | 🟡 草案 | 模块依赖拓扑优化计划 |

### 📖 使用指南（Guides）

| 文档 | 状态 | 说明 |
|------|------|------|
| [`guides/mcp-integration-guide.md`](guides/mcp-integration-guide.md) | 🟢 活跃 | Claude Code 等 MCP Client 的配置指南 |
| [`guides/mcp-5ire-integration.md`](guides/mcp-5ire-integration.md) | 🟢 活跃 | 5ire MCP Client 的配置指南 |
| [`guides/VAULT_FORMAT_SPEC.md`](guides/VAULT_FORMAT_SPEC.md) | 🟢 活跃 | Vault 笔记 YAML frontmatter + Markdown body 规范 |

### 🔬 研究分析（Research）

| 文档 | 状态 | 说明 |
|------|------|------|
| [`research/unified-analysis.md`](research/unified-analysis.md) | 🟢 有效 | 双模态（Human TUI + AI MCP）战略定位，无版本硬编码 |
| [`research/ai-infrastructure-analysis.md`](research/ai-infrastructure-analysis.md) | 🟢 有效 | AI 赛道分析 |
| [`research/competitive-risks.md`](research/competitive-risks.md) | 🟢 有效 | 风险矩阵与修正路线 |
| [`research/competitive-roadmap-table-a.md`](research/competitive-roadmap-table-a.md) | 🟢 有效 | 五战蚕食战略路线 |
| [`research/competitive-analysis.md`](research/competitive-analysis.md) | 🟡 部分过时 | 功能对比表需更新 tool 数量 |
| [`research/competitive_analysis_plan.md`](research/competitive_analysis_plan.md) | 🟡 部分过时 | Phase 状态需刷新 |
| [`research/architecture_audit_20260415.md`](research/architecture_audit_20260415.md) | 🟡 历史参考 | 基于 2026-04-15 代码状态，顶部已标注历史 |
| [`research/memory_infrastructure_design.md`](research/memory_infrastructure_design.md) | 🟡 待更新 | 记忆基础设施设计，Vault 系统已部分替代 |
| [`research/referenced_repos_report.md`](research/referenced_repos_report.md) | 🟢 有效 | 静态参考仓库报告 |
| [`research/AUDIT_INDEX.md`](research/AUDIT_INDEX.md) | 🔴 归档 | 2026-04-23 文档审计报告，版本 v0.2.3，结论已过时 |

### 🗄️ 归档（Archive）

> 历史文档，保留只读价值，不再维护。

| 文档 | 归档理由 |
|------|----------|
| [`archive/DEVELOPMENT_ROADMAP_0423.md`](archive/DEVELOPMENT_ROADMAP_0423.md) | 三项目协同路线图（0423 会议），波次未执行 |
| [`archive/0423-cross-project-meeting.md`](archive/0423-cross-project-meeting.md) | 0423 跨项目会议纪要 |
| [`archive/execution-plan.md`](archive/execution-plan.md) | Wave 1–4 甘特图，任务已完成 |
| [`archive/sprint_2_plan.md`](archive/sprint_2_plan.md) | Sprint 2（04-18 ~ 05-01）全部完成 |
| [`archive/smoke_test_report_20260418.md`](archive/smoke_test_report_20260418.md) | 旧版本 `76ccaf5` 测试报告（48 passed） |
| [`archive/user_testing_guide.md`](archive/user_testing_guide.md) | 旧版本用户测试指南 |
| [`archive/mcp_contract.md`](archive/mcp_contract.md) | v0.1 草案仅 4 个 tool，实现已大幅演进 |
| [`archive/STAGE_REPORT_2026-04-10.md`](archive/STAGE_REPORT_2026-04-10.md) | 版本 0.1.0-beta，10 个 tool，历史里程碑 |

---

## 文档维护原则

1. **活跃文档**（🟢）必须与实际代码状态同步；出现矛盾时优先修正文档。
2. **归档文档**（🔴）禁止修改内容，仅可添加顶部归档声明。
3. **新增文档**必须在本文档注册，否则视为孤立文档，下次审计时标记为待归档。
4. **状态标注**：每个 Markdown 文档的顶部应包含 `> **状态**：...` 标注，格式见 [`plans/skill-runtime.md`](plans/skill-runtime.md)。
