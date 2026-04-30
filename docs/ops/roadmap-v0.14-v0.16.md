# devbase 后续推进路线图 v0.14 → v0.16

> **基线版本**: v0.13.0 (Schema v25, 4-Change MVCC + unwrap 清零 + 行为信号表已完成)
> **生成日期**: 2026-04-26
> **约束**: SQLite + Tantivy only; No Docker/Qdrant/closed-source; Rust core in-house
> **审计报告**: `docs/_audit/2026-04-26-*.md` (架构 / 代码质量 / 六维模型 / Embedding 调研)

---

## 一、当前状态快照 (v0.13.0)

| 维度 | 状态 | 备注 |
|------|------|------|
| **Situation** | ✅ | scan + query_repos + vault_search |
| **State** | ✅ | health + index + code_metrics |
| **Relations** | 🟡 | `relations` 表已激活(v24)，但 **MCP 零暴露**；图遍历不可达 |
| **Capability** | 🟡 | 38 MCP tools，但 workflow **零 MCP 暴露**；31/38 tools 无 invocation 测试 |
| **History** | 🟡 | `activity`(oplog) + `agent_symbol_reads`(v25) 已落地；experiments 只写不读 |
| **Relevance** | 🟡 | `goal` + `hybrid_search_symbols` boosting 已落地；embedding 生成仍依赖 Python 回退 |

**工程基线**:
- 31 kSLOC / 100 .rs 文件 / 22 top-level modules
- 390 lib tests + 11 CLI tests, 0 failed, 0 warnings
- Release build: ~38s, binary 24.3 MB
- **生产代码 unwrap: 0**（全部 683 处在 `#[cfg(test)]` 中）
- unsafe: 0, TODO: 1
- 测试文件覆盖率: 58% (54/93 文件有 `#[cfg(test)]`)

**关键债务** (按审计):
- `WorkspaceRegistry` God Object (46+ 文件依赖)
- `init_db_at` 1,214 行（25 个 migration 内联）
- `mcp/tools/repo.rs` 2,376 行（25+ 工具样板重复）
- `lib.rs` 32 模块过度暴露
- `core/` 完全未使用

---

## 二、P0: v0.14 "Local Relevance Engine + 六维闭环"（~3 周）

### 2.1 本地 Embedding 生成（candle 方案）— 接口预留已完成，Sprint 14 实施

**问题**: `generate_query_embedding` 依赖本地 Python + sentence-transformers，离线时降级为纯 keyword。

**方案**: `candle` 纯 Rust 方案（调研结论，见 `docs/_audit/2026-04-26-embedding-research.md`）
- `candle-core` + `candle-nn` + `candle-transformers` + `tokenizers` + `hf-hub`
- 模型: `sentence-transformers/all-MiniLM-L6-v2` (safetensors, ~22MB)
- Feature flag: `local-embedding`（已在 `Cargo.toml` 预留）
- 接口预留: `EmbeddingProvider` trait + `default_provider()` 已落地 (`src/embedding.rs`)
- Spike 验证: ✅ `spike/candle-embedding/` 编译通过，dim=384, L2 norm=1.0

**验收**:
- `cargo test` 通过（含 feature on/off）
- embedding 输出与 Python 版余弦相似度 > 0.999
- `project_context` 的 `goal` 参数自动使用本地 embedding

### 2.2 `relations` MCP 暴露（🔴 六维最严重缺口）

**问题**: `relations` 表已激活但 38 个 tools 中无一个查询它，统一实体模型的图遍历能力完全架空。

**方案**:
- 新增 `devkit_relations` MCP tool: 查询实体的出入关系
- 新增 `devkit_relation_graph` tool: 有限深度图遍历（BFS, depth ≤ 3）
- CLI 对称: `db relations <entity_id>` / `db relation-graph <entity_id>`

### 2.3 Workflow MCP 暴露

**问题**: Workflow 引擎完全无 MCP 暴露，AI Agent 无法触发自动化。

**方案**:
- `devkit_workflow_list`
- `devkit_workflow_run`
- `devkit_workflow_status`

### 2.4 `project_context` 完整化

补全当前缺失维度:
- `relations` 图数据（top 10 相关实体）
- `known_limits`（repo 健康/索引覆盖率）
- `skills`（已安装 skill 列表）
- `workflows`（活跃 workflow 状态）
- `agent_symbol_reads` 统计（高频阅读 symbols）

---

## 三、P1: v0.15 "System Hardening"（~2-3 周）

### 3.1 架构债务清偿

| 债务项 | 严重度 | 方案 | 预估 |
|--------|--------|------|------|
| `WorkspaceRegistry` God Object | 🔴 高 | 拆分为 facade + `KnowledgeRegistry`/`RepoRegistry`/`VaultRegistry`/`OplogRegistry` | 2-3 天 |
| `init_db_at` 1,214 行 | 🟡 中 | 拆分为 `migrations/v{n}.rs` | 4h |
| `mcp/tools/repo.rs` 2,376 行 | 🟡 中 | 提取 `mcp_schema!` 宏或 builder | 1 天 |
| `lib.rs` 32 模块过度暴露 | 🟡 中 | 降级为 `pub(crate)` | 2h |
| `core/` 完全未使用 | 🟢 低 | 删除 | 30min |

### 3.2 测试覆盖率攻坚

**目标**: 58% → 75% 文件有测试。

**重点**:
- 31/38 MCP tools 补 invocation 测试（Mock registry + in-memory DB）
- `semantic_index.rs` — AST 提取 smoke test
- `dependency_graph.rs` — 多语言解析完整性

### 3.3 `vault_repo_links` → `relations` 迁移

- `save_vault_note` 双写 `vault_repo_links` + `relations`
- `get_linked_repos` / `get_linked_vaults` 逐步切换
- Schema v26 迁移: 复制现有数据

---

## 四、P2: v0.16 "Performance + Polish"（~2 周）

### 4.1 CI/CD 硬化

- `rust-cache` 集成（clean build 5min → 2min）
- `cargo audit` + `cargo deny`（供应链安全）
- clippy `-D warnings`

### 4.2 Binary 体积控制

- 当前: 24.3 MB
- `local-embedding` feature 开启后预估: ~30-32 MB
- 若超过 30 MB: 评估 `devbase-skill` 或 `devbase-search` crate 拆分

### 4.3 死代码清理

- `ai_discoveries` / `entity_types` 表删除
- `sync_protocol.rs` 死代码清理
- 14 项 `#[allow(dead_code)]` 逐一审计

---

## 五、Hard Veto 检查

| 提案 | Veto? | 说明 |
|------|-------|------|
| 本地 Embedding (`candle`) | ✅ 通过 | 纯 Rust，无云端依赖，单 binary |
| `relations` MCP 暴露 | ✅ 通过 | 利用已有表，无新增外部系统 |
| Workflow MCP 暴露 | ✅ 通过 | 已有引擎，仅加接口层 |
| Registry 拆分 | ✅ 通过 | 纯 Rust 重构 |
| CI cargo audit | ✅ 通过 | 安全加固 |
| Qdrant/向量数据库 | ❌ 否决 | Hard Veto |
| Docker 容器化 | ❌ 否决 | Hard Veto |
| 闭源 embedding API | ❌ 否决 | Hard Veto |

---

## 六、里程碑与验收标准

| 版本 | 核心交付 | 验收标准 |
|------|----------|----------|
| **v0.14** | Local Relevance + 六维闭环 | `goal` 无需外部 provider；`relations`/`workflow` MCP 暴露；`project_context` 6 维完整；Schema v25→v26 |
| **v0.15** | System Hardening | Registry facade 拆分；init_db_at 模块化；测试覆盖率 ≥ 75%；`vault_repo_links` → `relations` |
| **v0.16** | Performance + Polish | CI < 3min；cargo audit 通过；binary ≤ 30 MB；死代码清零 |

---

## 七、审计 Actionable 速查

| 来源 | 事项 | 版本 |
|------|------|------|
| 架构审计 | `WorkspaceRegistry` facade 拆分 | v0.15 |
| 架构审计 | `ai_discoveries`/`entity_types`/`core/` 清理 | v0.16 |
| 代码质量 | `init_db_at` 1,214 行拆分 | v0.15 |
| 代码质量 | `mcp/tools/repo.rs` schema 宏提取 | v0.15 |
| 代码质量 | `lib.rs` pub mod 降级 | v0.15 |
| 六维评估 | `relations` MCP 暴露 | **v0.14** |
| 六维评估 | Workflow MCP 暴露 | **v0.14** |
| 六维评估 | 31/38 tools 补 invocation 测试 | v0.15 |
| Embedding 调研 | `candle` 本地 embedding 实施 | **v0.14** |
