# devbase 六维模型评估报告
日期: 2026-04-26
版本: v0.13.0 (Schema v25)
框架: Context Compiler 六维闭环 (Situation/State/Relations/History/Capability/Relevance)

## 六维评分

| 维度 | 状态 | 说明 |
|------|------|------|
| **Situation** | 🟡 部分 | 无统一 workspace snapshot 工具; `entities` 无法按类型聚合; Skill 未纳入统一实体模型 |
| **State** | 🟡 半闭环 | `devkit_health` 不报告索引新鲜度; 无统一状态仪表盘 |
| **Relations** | 🟡 半闭环 | `relations` 表已激活(v24)但 **MCP 零暴露**; 图遍历能力未开放给 tools |
| **Capability** | 🟡 半闭环 | Workflow 引擎 **零 MCP 暴露**; Skill 安装/卸载/评分无 MCP 工具 |
| **History** | 🟡 半闭环 | `agent_symbol_reads`(v25) 已读写(`hybrid_search_symbols` boosting); `experiments` 只写不读; Skill 执行历史缺失 |
| **Relevance** | 🟡 代码层闭环 | `goal` + `hybrid_search_symbols` boosting 已落地; Vault 笔记无 embedding/relevance |

> ⚠️ 注: 审计 agent 指出 `relations` "零暴露"和 `agent_symbol_reads` "只写不读"，实际情况:
> - `relations`: `list_dependencies`/`list_reverse_dependencies` 已查询，但无 MCP tool 暴露
> - `agent_symbol_reads`: `hybrid_search_symbols` 已读取计数进行 boosting（v25 新增代码）

## 横向审计

### 1. 测试覆盖
- 38 个 MCP tools 中仅 **7 个**有 invocation 测试
- **31 个 tools**无任何测试
- 高危无测: `context.rs`, `dependency_graph` tools, `workflow` tools

### 2. CLI/MCP 对称性

| 侧 | 独占功能 |
|----|---------|
| CLI | workflow 生命周期、skill 安装/卸载/评分、registry backup/restore、vault 批量导入 |
| MCP | call_graph、dead_code、cross_repo_search、semantic_search、agent_symbol_reads boosting |

- **双向缺口巨大**，MCP 是 AI Agent 的主要接口，CLI 独占功能应逐步 MCP 化

### 3. `project_context`  verdict

**当前状态: 局部编译器，非真·编译端点**

缺失维度:
- `relations` 图数据
- `known_limits` (repo 健康/已知问题)
- `skills` (已安装 skill 列表)
- `workflows` (活跃 workflow 状态)
- `agent_symbol_reads` 统计
- health state / index coverage
- 仅支持单项目模式（无 workspace 级聚合）

## 🔴 最严重缺口

| 排名 | 缺口 | 影响 | 建议方案 |
|------|------|------|---------|
| 1 | `relations` 无 MCP tool | 图遍历能力架空 | 新增 `devkit_relations` tool |
| 2 | Workflow 零 MCP 暴露 | 自动化能力无法被 AI 调用 | 暴露 `devkit_workflow_*` tools |
| 3 | 31/38 tools 无 invocation 测试 | 回归风险极高 | 补充 integration tests |
| 4 | Vault 笔记无 relevance ranking | 知识检索质量低 | Vault notes embedding (v0.15) |
| 5 | `project_context` 非 workspace 级 | 跨项目分析缺失 | 新增 `workspace_context` (v0.16) |

## v0.14-v0.16  roadmap 关联

| 版本 | 对应缺口 |
|------|---------|
| v0.14 | 本地 Embedding + Vault notes relevance |
| v0.15 | `relations` MCP 暴露 + Workflow MCP 化 |
| v0.16 | Workspace snapshot + 测试覆盖 75% |
