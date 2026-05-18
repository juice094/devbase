# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.20.1] - 2026-05-17

### Added

- **Phase 1 Production Hardening**
  - Workflow E2E 测试 — `src/mcp/tools/workflow.rs`：DAG 成功执行、失败传播验证
  - RF-7 路径隐私脱敏 — `sanitize_path()` 自动掩码 home 目录为 `~`
  - Tantivy 一致性修复 — `repair_tantivy_consistency_at()` 启动时自动检测 orphan/missing 文档
  - 性能回归基线 — `test_keyword_search_latency_regression_1k` / `_10k`（profile-aware 阈值）
  - `TempStorageBackend` — 测试隔离后端，消除 `DEVBASE_DATA_DIR` 竞态
- **Architecture Invariants CI 自动化** — `scripts/invariant-checks/run-checks.ps1`
  - G5 (RF-6)：diff-only 检测新增生产代码 `unwrap`/`expect`/`panic`（排除 `#[cfg(test)]`）
  - T11：`mcp/tools` 禁止直接调用 `rusqlite::Connection`
  - T12：`tui/render` 纯消费检查（禁止写入操作）

### Fixed

- `AppContext::with_storage()` 使用实际 storage backend 的 `index_path()` 而非硬编码默认值
- G5 invariant checker 正则修复：`tests.rs` 文件正确跳过
- `Cargo.lock` 同步版本 bump（修复 `--locked` release 构建失败）
- 平台相关测试隔离：`C:\` 路径断言加 `#[cfg(windows)]`，Linux `python3` 断言适配
- HuggingFace 网络依赖测试加 `#[ignore]`（避免 CI TLS 证书失败）

## [0.20.0] - 2026-05-16

### Added

- **知识完备性**：Vault 双向链接图遍历（BFS depth 1-3）+ `[[note#heading]]` block 引用
- **Vault 笔记历史追踪** — Git-based blob diff，`devkit_vault_history` tool
- **混合检索质量监控** — `HybridSearchMetrics`（latency/recall/overlap/keyword_source）
- **性能回归基线** — Criterion benchmarks：`index_repo_full`、`cosine_similarity`、`extract_symbols`
- **客户端无关原则** — `StorageBackend` trait 完整实现，解耦 `dirs::data_local_dir()` 硬编码
- **MCP Tools +4** (68 total)
  - `devkit_vault_history`, `devkit_vault_export`, `devkit_vault_graph`, `devkit_vault_daily`

### Changed

- 20+ 独立 crate 零循环依赖，workspace 拆分完成
- `entities` 表成为唯一真相源，`repos` 表彻底删除
- Tantivy / SQLite 补偿扫描：启动时自动同步 orphan 文档

## [0.19.0] - 2026-05-14

### Added

- **SQLite WAL 模式** — `r2d2` 连接池 + WAL journal，并发安全与增量备份
- **Tantivy 健康评分** — `devkit_index_health`：损坏检测、自动重建、孤儿文档清理
- **Vault 导出** — `devkit_vault_export`：Obsidian-compatible Markdown 批量导出
- **Redis ADR 决策** — `docs/architecture/adr-003-redis.md`：评估后决定保持 SQLite 优先
- **OpLog 审计追踪** — 结构化事件类型 `OplogEventType`，全操作不可变日志

### Changed

- Schema 迁移前自动生成 `backup-YYYYMMDD-HHMMSS.db` 快照
- 索引层反向一致性扫描与自动修复能力

## [0.18.0] - 2026-05-13

### Added

- **ClaudeCode 工作流集成** — `docs/RFC/claudecode-workflow-integration.md`
  - `devkit_project_brief` — 生成项目 Markdown 简报（架构 + 模块 + 近期提交 + 已知约束），用于 `.claude/CLAUDE.md` 注入
  - `devkit_impact_analysis` — 符号级变更影响半径分析（BFS 调用图遍历 + 相关符号发现 + 测试启发式 + 历史 oplog）
  - `scripts/devbase-claude.ps1` — PowerShell 一键启动器：自动检测 repo → 生成简报 → 注入 `.claude/CLAUDE.md` → 启动 `claude` → 可选捕获退出 diff
- **Session 导入/导出工具**
  - `devkit_session_export` — 导出会话为 Markdown / JSON；支持记忆类型图标与元数据
  - `devkit_session_import` — 从 bulk text 批量导入记忆（`[type]` 前缀解析）
- **MCP Tools +4** (64 total)
  - `devkit_project_brief`, `devkit_impact_analysis`, `devkit_session_export`, `devkit_session_import`
- **TUI Session 视图硬化**
  - 三态 MainView 切换：`RepoList → VaultList → Session`（`Tab` 键循环）
  - Session 列表：状态图标（● active / ◌ archived）+ 高亮样式
  - Session 详情：记忆类型图标（◆ decision / ▪ constraint / ★ discovery / ✗ error）+ embedding model 标签 + indexed 状态
- **AGENTS.md** 同步至 v0.18.0-dev 基线（64 Tools / 437 tests）

### Changed

- `src/mcp/mod.rs` Tool 注册表扩展至 64 工具（稳定 + Beta）
- `src/mcp/tests.rs` 工具计数断言同步
- TUI `render_session.rs` / `state/mod.rs` 适配 Schema v34 记忆字段（`embedding_model`, `indexed_at`）

## [0.17.0] - 2026-05-13

### Added

- **Agent Memory 向量存储** — Schema v34
  - `agent_memories` 新增 `embedding BLOB`, `embedding_model TEXT`, `indexed_at DATETIME`
  - Partial index `idx_agent_memories_embedding` 仅索引含向量的行
  - `AgentMemory` 结构体扩展向量元数据字段
- **SQLite UDF: `cosine_similarity`** — `src/registry/agent_context.rs`
  - 输入: 两个 little-endian f32 BLOB
  - 输出: REAL ∈ [-1.0, 1.0]
  - 注册时机: `WorkspaceRegistry::init_db_at` 迁移完成后自动注册
- **语义记忆搜索** — `search_memories_semantic(context_id, query_embedding, limit)`
  - 纯 SQL `ORDER BY cosine_similarity(embedding, ?) DESC`
  - 零 LLM 运行时依赖；仅执行向量比对
- **MCP Tools +2** (60 total)
  - `devkit_session_recall` — 外部向量查询 + 语义召回 top-k memories
  - `devkit_session_index` — 为已有 memory 注入外部生成 embedding
- **Skill Runtime Auto-Recall** — `src/skill_runtime/executor.rs`
  - Tier 1: Semantic recall (本地 Candle/Ollama 或外部 HTTP endpoint)
  - Tier 2: Keyword fallback (`LIKE` search on `content`)
  - 新环境变量: `DEVBASE_CONTEXT_MEMORY_COUNT`, `DEVBASE_CONTEXT_RECALL_METHOD`
  - `DEVBASE_CONTEXT_MEMORIES` 升级为 top-k 相关 memories（含 `score` + `model`）
- **外部 Embedding Provider 集成**
  - `call_external_embedding_endpoint` — `reqwest::blocking` POST `/api/embeddings`
  - 配置驱动: `config.toml [embedding]` (enabled/provider/model/base_url/timeout)
  - 端到端测试: mock TCP server 验证 Ollama 格式解析 + 错误码处理
- **RFC 文档** — `docs/RFC/agent-memory-vector-storage.md`
  - 架构决策: devbase = 向量数据库层，不做 embedding 生成
  - 参照 pgvector 边界设计

### Changed

- **Feature Flags**: `embedding` 从 `default` 移除
  - Candle/Ollama 依赖变为 opt-in: `--features embedding`
  - 默认构建零 ML 依赖，编译时间减少 30~50%
- `insert_memory` 签名扩展: 新增可选 `embedding: Option<&[f32]>` 和 `embedding_model: Option<&str>`
- `list_memories` / `search_memories` SELECT 语句扩展为 8 列（兼容新增字段）
- AGENTS.md 同步至 v0.17.0-dev 基线

### Breaking Changes

- 默认构建不再包含 `devbase-embedding` crate；需要语义生成能力的用户须显式启用 `--features embedding`
- `generate_query_embedding` 在默认构建下返回错误（提示启用 feature 或配置外部 endpoint）

## [0.16.1] - 2026-05-13

### Added

- **Workflow-Session Binding** — Schema v33
  - `workflow_executions` 新增 `context_id` 列 + 索引
  - `create_execution` 自动绑定 `resolve_active_context()`
  - MCP `devkit_workflow_run` 与 CLI `workflow run` 均支持自动绑定
  - `devkit_session_workflows` tool: 列出指定 context 的 workflow 执行历史
- `context_entity_links` 表 (Schema v32): context 与任意 entity 的多对多关联

## [0.16.0] - 2026-05-13

### Added

- **Agent Contexts (AI Agent OS)** — Schema v31
  - `agent_contexts` 表: 持久化 AI session / project scope
  - `agent_memories` 表: 结构化记忆（decision/constraint/note/discovery/error）
  - 9 个 Session MCP tools: save/list/resume/attach/detach/activate/search/capture/workflows
  - `resolve_active_context()`: 环境变量 `DEVBASE_ACTIVE_CONTEXT` → 文件 `.active_context` fallback
  - Context-aware Skill Runtime: 注入 `DEVBASE_ACTIVE_CONTEXT` + `DEVBASE_CONTEXT_MEMORIES` + `DEVBASE_CONTEXT_LINKS`
  - 所有 agent_context 操作自动写入 OpLog (`OplogEventType::AgentContext`)

## [0.15.0] - 2026-05-04

### Added

- **P1: Tantivy BM25 代码符号搜索** — `search/symbol_index.rs`
  - 独立 Schema (`repo_id`, `name`, `signature`, `file_path`, `line_start`)
  - `keyword_search_symbols` 主路径走 Tantivy BM25，SQLite LIKE 回退
  - 索引流程 `index.rs` 自动同步写入 symbol_index
  - `StorageBackend` 扩展 `symbol_index_path()`（6 实现）
- **P3: Embedding 多后端** — Candle (默认) + Ollama (配置切换)
  - 新增 `OllamaProvider` (`ureq` HTTP `/api/embed`)
  - `create_provider(backend, model, base_url, timeout)` 配置化创建
  - `generate_query_embedding` 通过 `OnceLock` 懒加载配置化 provider
  - 默认模型改为 `all-minilm` (384-dim，与 Candle 维度兼容)
- **P4: Health 环境检测扩展** — `EnvVersionCache` 从 5 工具 → 9 工具
  - 新增: `python`, `bun`, `zig`, `java`
  - `get_tool_version` 支持 stderr fallback (Java 输出到 stderr)
  - `fmt_version` 改进: Java 引号提取、Docker/Python 格式处理
- **P5: 架构不变量自动化 CI** — `scripts/invariant-checks/run-checks.ps1`
  - G5: diff-only 检测新增生产代码 unwrap/expect/panic（排除 `#[cfg(test)]`）
  - T11: 检测 `mcp/tools/*` 直接调用 `rusqlite::Connection`
  - T12: 检测 `tui/render/*` 写入操作
  - CI job `invariant-check` 加入 `.github/workflows/ci.yml`
- **P2 Phase 1: AppContext 职责拆分** — 6 个 Client trait impl 迁出 `storage.rs`
  - `scan.rs` / `health.rs` / `sync.rs` / `digest.rs` / `knowledge_engine/mod.rs` / `registry.rs`
  - `storage.rs` 860 → 430 行 (-50%)
  - 删除冗余 `conn_mut()`
- **P2 Phase 2: 内联 SQL 下沉** — 新增 `registry/code_symbols.rs` + `registry/dead_code.rs`
  - `CodeSymbolRow` / `DeadCodeRow` + 纯函数查询 (12 个单元测试)
  - `RegistryClient` 退化为纯代理层

### Changed

- `EmbeddingConfig` 默认模型 `nomic-embed-text` → `all-minilm` (384-dim)
- AGENTS.md 阶段描述更新: v0.14.3 → v0.15.0 推进中 → v0.15.0 全部完成

### Fixed

- **TTL 缓存负值 bug** (`97172ec`): `elapsed < ttl_seconds` → `elapsed >= 0 && elapsed < ttl_seconds`
  - 防止系统时间回溯导致缓存永不过期
- `crates/devbase-embedding/src/lib.rs` 遗留 unwrap 清零 (`encode_with_candle` → `ok_or`)

## [0.14.3] - 2026-05-05

### Added

- **Schema v30** — `code_symbols.attributes` 列，tree-sitter 提取 `#[test]`/`#[tokio::test]` 等属性
  - `devkit_dead_code` 自动过滤测试函数，消除假阳性
  - `rust_node_to_symbol` 支持 `prev_sibling()` 回溯收集属性节点
- **Tantivy/SQLite 补偿扫描** — 启动时自动检测并清理 orphan 文档
  - 新增 `search::sync_index_to_db(conn)`，对比 Tantivy `list_indexed_repo_ids()` 与 SQLite `entities`
  - `AppContext` 初始化后自动调用，失败仅 warn 不阻塞启动
- **Feature flags** — `mcp` + `embedding`，支持 `--no-default-features` 最小化编译
  - `default = ["tui", "mcp", "embedding"]`
  - `devbase-embedding` 设为 `optional = true`
  - 新增 `src/clients.rs` 提取 MCP client traits，避免 mcp feature 关闭时 trait 不可用
- **Kimi CLI MCP 集成文档** — AGENTS.md 新增 Kimi CLI 集成状态，项目级 skill 位于 `.kimi/skills/devbase-project/`

### Changed

- **RF-1 架构红线** — `init_db()` 全局路径残留清零
  - `init_db()` 标记 `#[deprecated]`，新增 `init_db_with(backend: &dyn StorageBackend)`
  - `workflow/executor.rs`、`workflow/state.rs`、`storage.rs` 全部改为注入式
  - `examples/` + `benches/` 中额外 5 处残留同步修复
- `index_repo_full` 合并用户 `scan.exclude_patterns` 与默认排除模式
- `cargo fmt` + `cargo clippy --fix` 全量格式化（8 文件，6 处 warning 修复）
- `CONTRIBUTING.md` 新增 sccache 构建加速指南

### Fixed

- `cargo clippy --all-targets -D warnings` — 7 warnings → 0
- `cargo fmt --check` — 全量通过

## [0.14.2] - 2026-05-02

### Changed

- health dirty 检测修复（排除 ignored 文件）
- scan 路径规范化 + syncthing-rust 识别修复
- experiment_log / CodeMetrics / ModuleGraph / CallGraph / DeadCode 提升为 Beta tier
- 48 tools: Stable 5 / Beta 40 / Experimental 3

## [0.14.1] - 2026-05-01

### Added

- CLI JSON 输出补全 (`--json` / `--recalc`)
- relations MCP 工具加固
- License headers 全量补录
- Vault Daily / Vault Graph MCP tools

## [0.14.0] - 2026-04-28

### Added

- Workspace 拆分：6 个零耦合 crate 提取
- MCP trait 化：`mcp/tools/repo.rs` `crate::` 引用 68→41

## [0.13.0] - 2026-04-26

### Added

- Registry God Object 拆解：10 子模块提取为 free function
- `WorkspaceRegistry` 退化为纯 facade

## [0.12.0] - 2026-04-30

### Added

- **Schema v22** — drop `vault_notes`, `papers`, `workflows` orphan tables; `entities` becomes sole source of truth for all entity types
- **Managed-Gate Fail-Safe Defaults** — `devbase sync` defaults to managed repos only
  - Management tags: `mirror`, `reference`, `third-party`, `collaborative`, `team`, `own-project`, `tool`, `active`, `managed`
  - Untagged / non-management repos are registered but skipped by default sync
  - `--filter-tags` bypasses the gate for explicit selection
- **`.devbase-ignore`** — directory-level opt-out exclusion during scan
- `scan --register` no longer auto-tags repos with `"discovered"`
- i18n hint for unmanaged repos

### Changed

- `inspect_repo`: remove `"discovered"` from default tags; `-main`/`-master` repos keep `zip-snapshot` + `needs-migration`
- `collect_tasks`: default mode filters by management tags
- All `list_workflows` / `list_papers` / `list_vault_notes` queries migrated to `entities` table + `json_extract`
- Generic `upsert_entity` abstraction for entity dual-write
- `ENTITY_TYPE_*` constants extracted across 10 files (~25 replacements)
- `cargo test --lib`: 374 → 379 passed

### Breaking Changes

- Existing repos tagged `"discovered"` are **no longer synced by default**.  
  Use `devbase tag <repo> managed` (or any management tag) to opt a repo into automatic sync.

## [0.10.0] - 2026-04-26

### Added

- **L3 Risk Layer MVP** — `known_limits` 表 + Registry CRUD + MCP tools + CLI subcommand
  - Schema v18: `known_limits` 表（id, category, description, source, severity, first_seen_at, last_checked_at, mitigated）
  - Registry CRUD: `save`/`get`/`list`/`delete`/`resolve`/`seed_hard_vetoes`
  - MCP tools: `devkit_known_limit_store` / `devkit_known_limit_list`（Beta tier）
  - CLI: `devbase limit {add,list,resolve,delete,seed}`
  - OpLog 集成: create/update/resolve/delete/seed 自动写入 oplog（event_type = `KnownLimit`）
  - Hard Veto 种子: AGENTS.md 中的 5 条硬约束自动填充
- **L4 元认知层 MVP** — `knowledge_meta` 表 + L3-L4 联动
  - Schema v19: `knowledge_meta` 表（id, target_level, target_id, correction_type, correction_json, confidence, created_at）
  - Registry CRUD: `save`/`get`/`list`/`delete`
  - CLI 联动: `devbase limit resolve <id> --reason "..."` 自动创建 L4 meta 记录
- **Hard Veto 运行时守卫** — Skill 执行前自动检查未解决 hard veto
  - `skill_runtime::executor::run_skill` 执行前查询 `known_limits`
  - 未解决 hard veto 存在时，警告注入 `stderr`，同时写入 OpLog
  - 零破坏性：skill 仍执行成功，但输出中包含 `[HARD-VETO-WARNING]`

### Changed

- `cargo test --all-targets`: 279 → 288 passed
- MCP tool 总数: 35 → 37

## [0.11.3] - 2026-04-26

### Changed

- **Phase 1 主从表切换 — Stage 3 完成**（`repos` 表删除）
  - `save_repo` / `update_repo_*` / `run_clean` 不再写入 `repos`
  - Schema v21 迁移：重建 11 个子表（去 FK）→ 删除 `repos` 表
  - `test_helpers.rs` SCHEMA_DDL 同步去 `repos` + 去 FK
  - `entities` 成为真正的读写唯一数据源

## [0.11.2] - 2026-04-26

### Changed

- **Phase 1 主从表切换 — Stage 2 完成**（读路径迁移）
  - `list_repos` / `list_repos_stale_health` / `list_repos_need_index` / `list_workspaces_by_tier` 全部改为从 `entities` 读取（`json_extract`）
  - `digest.rs` / `health.rs` / `daemon.rs` / `backup.rs` / `knowledge_engine.rs` / `sync/*.rs` / `tui/state.rs` / `mcp/tools/repo.rs` 等所有 `list_repos()` 调用方自动迁移
  - 直接 SQL 查询迁移：`dependency_graph.rs`, `registry/links.rs`, `registry/knowledge.rs`, `query.rs`, `oplog_analytics.rs`, `commands/simple.rs`
  - `update_entity_metadata_field` 修复 `json_set` 字符串引号问题：原始字符串直接传递，`"null"` 时自动 `json_remove`
  - `repo_tags` / `repo_remotes` 子表保留，通过 `repo_id` JOIN 读取（FK 仍指向 `repos`）

## [0.11.1] - 2026-04-26

### Changed

- **Phase 1 主从表切换 — Stage 0 完成**（entities 第一公民前置）
  - Schema v20: Flat ID 命名空间迁移（`repo:devbase` → `devbase`，`skill:xxx` → `xxx`）
  - `sync_repo_to_entities_by_id` 重构为 `upsert_entity_for_repo`：直接由 `RepoEntry` 写入 entities，不再读取 repos
  - `update_repo_*` 改为先写 entities metadata（`json_set`），再写 repos
  - `save_repo` 写入顺序反转：entities → repos → repo_tags → repo_remotes
  - `run_tag` 补全 entities 双写：`sync_repo_tags_to_entity`
  - `run_clean` 改为先删 entities，再删 repos（保留 CASCADE 行为）
  - Skill entities 同步同理去除 `skill:` 前缀

## [0.11.0] - 2026-04-26

### Added

- **AppContext Pool 化** — 全链路数据库连接池统一
  - `AppContext` 持 `r2d2::Pool<SqliteConnectionManager>`，替代单 `Connection`
  - `scan`/`health`/`sync`/`backup`/`daemon`/`query` 等深层模块全部迁移
  - `init_db()` 调用点从 89 处降至 5 处合法保留（Pool 前 schema 引导 ×2、migrate 定义 ×1、workflow 测试辅助 ×2）
  - 根治 `spawn_blocking` / `thread::spawn` 闭包无法传递裸 `Connection` 的问题
- **MCP 测试隔离** — 全部 MCP 集成测试改用临时目录
  - `DEVBASE_DATA_DIR` 指向 `tempfile::TempDir` + `AppContext::with_defaults()`
  - 多线程并发测试全部通过，无 flaky
- **Search 测试竞态自愈** — `SEARCH_TEST_LOCK` + 临时目录隔离，多线程 (`--test-threads=4`) 稳定通过

### Changed

- `cargo test --all-targets`: 288 → 374 passed（+86 个新增/迁移测试）
- CI 测试并行度: `--test-threads=1` → `--test-threads=4`，回归测试耗时 ~13s → ~4s
- `rusqlite` 0.34 + `r2d2_sqlite` 0.27.0 版本锁定

## [0.9.0] - 2026-04-26

### Added

- **Workflow Loop Step 完整执行** — 5 种 step 类型全部可执行
  - `StepType::Loop { for_each, body }`：遍历集合，执行 body 子步骤
  - 变量插值：`${loop.item}` / `${loop.index}`
  - 结果聚合：stdout 按迭代索引标记，outputs 合并
  - 失败处理：单迭代失败按 body step 的 `on_error` 策略处理
- **12 个新增单元测试** — model/interpolate/validator/executor 全覆盖

### Changed

- `cargo test --all-targets`：267 → 279 passed

## [0.8.0] - 2026-04-25

### Added

- **Workflow 子类型执行** — Subworkflow / Parallel / Condition 全部可执行
  - `execute_subworkflow_step`：递归调用 `execute_workflow`
  - `execute_parallel_step`：子步骤串行执行 + 结果聚合
  - `execute_condition_step`：字符串插值后 true/false 评估
- **NLQ 自然语言查询结果可执行** — TUI `[:]` 搜索结果按 Enter 直接运行 skill
- **NLQ smoke test** — `run_nlp_selected_skill` 空列表/无技能/执行管道测试
- **TUI SkillPanel 拆分** — `SkillPanelState` 提取 7 个字段，App 51→44 字段

### Fixed

- 29 个生产代码 unwrap 全部清零
- 30 个 clippy 警告清零

## [0.7.0] - 2026-04-20

### Added

- **NLQ 自然语言查询** — TUI `[:]` 触发 embedding 语义搜索，fallback 降级文本搜索
- **智能同步建议** — `sync/policy.rs::recommend_sync_action` 基于 safety/ahead/behind 生成建议

## [0.6.0] - 2026-04-18

### Added

- **Mind Market 评分系统** — `skill_runtime::scoring`
  - `success_rate` + `usage_count` + `rating`（0-5 分公式）
  - CLI：`skill recalc-scores` / `skill top` / `skill recommend`
- **TUI Workflow 执行** — `[w]` 详情页 `r/Enter` 运行 + 结果弹窗

## [0.5.0] - 2026-04-17

### Added

- **Workflow Engine v0.5.0** — YAML 编排多步骤自动化
  - 5 种 step 类型：skill / subworkflow / parallel / condition / loop
  - 拓扑调度（Kahn 算法）+ batch 并行执行
  - 变量插值：`${inputs.x}` / `${steps.y.outputs.z}`
  - 错误策略：Fail / Continue / Retry / Fallback
  - Schema v17：`workflows` + `workflow_executions` 表
- **CLI/TUI Workflow 集成** — `devbase workflow {list,show,register,run,delete}` + `[w]` 面板

## [0.4.0] - 2026-04-15

### Added

- **Schema v16 统一实体模型** — `entity_types` + `entities` + `relations` 表，渐进双写
- **Skill 自动封装** — `devbase skill discover <path>` 自动分析项目 CLI/API，生成 SKILL.md
- **Git URL Discover** — `devbase skill discover https://github.com/...` 克隆+分析+注册
- **MCP `devkit_skill_discover`** — 35 tools 总数

## [0.3.0] - 2026-04-12

### Added

- **34 MCP tools 全量通过 MCP Inspector**
- **README Quick Start 三步内跑通**
- **CI/CD** — `.github/workflows/ci.yml`（check / test / fmt / clippy on Windows）
- **GitHub Release 预编译二进制**

## [0.2.4] - 2026-04-20

### Architecture

- **Outboard Brain Embedding Architecture** — Embedding generation moved to external Skill/MCP Server
  - `embedding.rs` stripped of Ollama/OpenAI generation logic; storage protocol only (`embedding_to_bytes`, `bytes_to_embedding`, `cosine_similarity`)
  - `knowledge_engine.rs` no longer generates embeddings during indexing
  - Aligns with "store + search in devbase, compute in Clarity/Skill" boundary

### Changed

- **Breaking** — `devkit_semantic_search` now accepts `query_embedding: number[]` instead of `query: string`
  - Embedding generation is the caller's responsibility (external MCP Server or Skill)
  - Removed `config.embedding.enabled` gate; search works as long as embeddings exist in DB

### Added

- **`devkit_embedding_store`** — Store externally-generated embedding vectors into SQLite
  - Parameters: `repo_id`, `symbol_name`, `embedding: number[]`
  - Upsert semantics (ON CONFLICT UPDATE)
- **`devkit_embedding_search`** — Alias for `devkit_semantic_search` with vector-based interface
  - Same parameters and behavior, alternative name for workflow clarity
- **MCP tool count**: 25 → 31

## [0.2.4] - 2026-04-20 (continued)

### Added

- **`devkit_hybrid_search`** — Hybrid vector + keyword search via RRF merge (Beta)
  - `search::hybrid.rs`: `rrf_merge()` (Reciprocal Rank Fusion, k=60), `keyword_search_symbols()` (SQLite LIKE on name/signature), `hybrid_search_symbols()` (auto-fallback to keyword when embedding missing)
  - `registry::knowledge::hybrid_search_symbols()` wrapper
  - Recommended default search tool for code concept discovery
- **`devkit_cross_repo_search`** — Cross-repository symbol search filtered by tags (Beta)
  - `registry::knowledge::cross_repo_search_symbols()`: INTERSECT-based tag filtering (AND semantics), per-repo hybrid search, global dedup+sort
  - Searches all repos matching ALL specified tags
- **`devkit_knowledge_report`** — Workspace knowledge coverage report (Beta)
  - `src/oplog_analytics.rs`: `generate_report()` with table-existence guards for resilient querying
  - Reports: repo_count, total_symbols, total_embeddings, total_calls, coverage_pct, per-repo breakdown, health_summary, recent_activity
- **`devkit_related_symbols`** — Explicit symbol-to-symbol knowledge links (Experimental)
  - Schema v13: `code_symbol_links` table (source_repo, source_symbol, target_repo, target_symbol, link_type, strength)
  - `src/symbol_links.rs`: `compute_similar_signature_links()` (Jaccard token overlap), `compute_co_located_links()` (same-file clustering)
  - `generate_and_save_links()`: persists links with ON CONFLICT IGNORE upsert
- **External Embedding Provider** — Reference Python implementation in `examples/embedding-provider/`
  - `index.py`: Ollama `/api/embeddings` client, batch generation, cross-platform registry DB path
  - Byte-compatible f32 little-endian serialization via `struct.pack`
  - CLI: `--repo-id`, `--model`, `--ollama-url`, `--batch-size`, `--force`
- **Schema v13** — `code_symbol_links` table for explicit conceptual relationships

### Engineering

- **Context Safety Mechanism** — Formalized as long-term architecture principle
  - Sub-agent execution: serial + commit-isolated work directories (prevents compilation races)
  - MCP tool idempotency: all state-mutating tools use ON CONFLICT UPDATE / transaction boundaries
  - OpLog as immutable audit trail for all state transitions

---

## [0.2.3] - 2026-04-20

### Added

- **Semantic Vector Search (Wave 1)** — Cosine-similarity code symbol search
  - `code_embeddings` table (Schema v11): `repo_id + symbol_name` PK, BLOB embedding, `generated_at`
  - `embedding.rs`: Ollama/OpenAI-compatible generation + `cosine_similarity` + byte serialization
  - `devkit_semantic_search` MCP tool (Beta): natural-language → embedding → top-K symbols
- **Multi-Language Symbol Extraction (Wave 2)** — tree-sitter AST parsing beyond Rust
  - `tree-sitter-python`, `tree-sitter-typescript`, `tree-sitter-go` dependencies
  - `SymbolType` expanded: Function, Struct, Enum, Trait, Impl, Module, Class, Interface, TypeAlias, Constant, Static
  - Per-language call-target resolvers for Call Graph construction
  - Languages supported: Rust, Python, JavaScript, TypeScript, Go
- **Call Graph Analysis** — Intra-repo function call relationship extraction
  - `code_call_graph` table (Schema v10): caller → callee edges with line numbers
  - `devkit_call_graph` MCP tool: "Who calls `register_tool`?"
- **Cross-Repo Dependency Graph expansion**
  - `CMakeLists.txt` parsing: `find_package`, `add_subdirectory`, `FetchContent_Declare`, `target_link_libraries`
  - `ManifestKind::CMake` added to dependency graph builder
- **Dead Code Detection** — `devkit_dead_code` MCP tool (Experimental)
  - SQL `NOT EXISTS` query over call graph to find functions with zero incoming edges
  - `LIKE 'pub%fn%'` heuristic to exclude non-public functions
- **arXiv Integration** — Pure string-parsing Atom XML fetcher (zero heavy XML deps)
  - `arxiv.rs`: `PaperMetadata` with title/authors/summary/published/category
  - `devkit_arxiv_fetch` MCP tool (Beta): fetch by arXiv ID
- **Performance Benchmarks** — Criterion suite (`benches/semantic_index.rs`)
  - `index_repo_full` (small/medium/full parameterization)
  - `cosine_similarity` (128/512/768 dims)
  - `extract_symbols` (Rust/Python/Go comparison)
  - `parse_cmake_lists` (CMake parsing)
- **Structured OpLog (Schema v12)** — Typed event system
  - `OplogEventType` enum replacing free-text `operation` field
  - JSON metadata + `duration_ms` for observability
  - Migration: `CASE` mapping from legacy strings to enum variants

### Fixed

- **`scan` async panic** — `fetch_github_stars` now runs in `std::thread::spawn` isolation
  - Prevents `reqwest::blocking::Client` drop inside tokio runtime from causing panic
  - `block_on_async()` helper detects runtime context and uses `mpsc` or temporary runtime
- **Dead code false positives** — `pub fn` → `pub%fn%` SQL LIKE match covers `pub async fn` / `pub(crate) fn` / `pub unsafe fn`
  - Excludes `main()` from dead code results
- **Clippy warnings** — 12+ lints resolved (`manual_strip`, `collapsible_if`, `FromStr`, `type_complexity`, `useless_format`, etc.)

### Changed

- **`nl_filter_repos`** — Now uses Tantivy full-text search as primary path
  - Falls back to structured SQL filtering when Tantivy is unavailable

---

## [0.2.2] - 2026-04-21

### Added

- **Vault Backlinks** — Find notes that link to a given note
  - `vault::backlinks:<note_id>` query prefix
  - TUI detail panel shows "被引用" section with backlink count and list
  - MCP tool `devkit_vault_backlinks` — AI can discover note relationships
  - `vault/backlinks.rs` with `build_backlink_index()` and `get_backlinks()`

### Changed

- **Schema v8** — `vault_notes` table no longer has `content` column
  - Migration: auto-creates `vault_notes_v2`, migrates data, drops old table
  - `save_vault_note` / `list_vault_notes` SQL updated to 8 columns
  - Filesystem-first architecture now complete at the database level

## [0.2.1] - 2026-04-20

### Added

- **Vault Watch** — Filesystem watcher for `workspace/vault/`
  - Auto-refresh TUI vault list when notes are edited externally
  - 500ms debounce to avoid excessive reloads
- **Vault Tantivy Search** — `vault:` queries now use Tantivy full-text index
  - Replaces slow SQLite LIKE + per-file reading
  - Supports keyword scoring and ranking
- **MCP Registry Manifest** — `server.json` for official MCP Registry submission

### Changed

- `query.rs` vault branch: uses `search_vault()` instead of in-memory filtering

## [0.2.0] - 2026-04-20

### Added

- **Vault System** — Markdown note management with Obsidian-compatible PARA structure
  - `vault/` directory with PARA folders: 00-Inbox, 01-Projects, 02-Areas, 03-Resources, 04-Archives, 99-Meta
  - Filesystem-first architecture: note content lives in `.md` files, SQLite only indexes metadata
  - YAML frontmatter parsing (title, tags, aliases, date)
  - WikiLink `[[...]]` extraction and backlink index building
- **TUI Vault View** — Press `Tab` to switch between Repo list and Vault note list
  - Vault list shows note titles with tag indicators
  - Detail panel previews note content (first 20 lines), tags, and outgoing links
  - `Enter` opens selected note in VS Code
- **MCP Vault Tools** — 3 new tools for AI Agent vault interaction
  - `devkit_vault_search` — full-text search across vault notes
  - `devkit_vault_read` — read note content and frontmatter by path
  - `devkit_vault_write` — write or append to vault notes
- **P2-lite: repos.toml** — Optional static configuration override for repositories
  - Declare tags, tier, and workspace_type in `workspace/repos.toml`
  - Overrides are applied on top of auto-discovered repo metadata
- **Unified Node Model** — `core::node::{Node, NodeType, Edge}` abstraction
  - `NodeType::GitRepo | VaultNote | Asset | ExternalLink`
  - Foundation for future Knowledge Graph unification
- **Workspace Directory** — `%LOCALAPPDATA%/devbase/workspace/` with `vault/` and `assets/`
- **MCP Client Config** — `mcp.json` for Claude Desktop / Cursor integration

### Changed

- **Architecture principle**: File system = source of truth; SQLite/Tantivy = derived index/cache
- Vault notes no longer store `content` in SQLite (read from disk on demand)

## [0.1.0] - 2026-04-20

### Added

- **TUI Dashboard** — Terminal UI for multi-repository workspace management
  - Repository list with status icons, stars, and tag indicators
  - Detail panel with Overview / Health / Insights tabs
  - Stars Trend sparkline (30-day history)
  - Help Overlay with categorized keyboard shortcuts
  - Responsive layout: compact / standard / wide screen modes
  - Cross-repository code search (ripgrep + Tantivy dual mode)
  - One-key launch into gitui / lazygit
- **MCP Server** — 14 tools for AI Agent integration (stdio transport)
  - `devkit_scan`, `devkit_health`, `devkit_sync`, `devkit_query_repos`
  - `devkit_code_metrics`, `devkit_module_graph`, `devkit_natural_language_query`
  - `devkit_index`, `devkit_query`, `devkit_note`, `devkit_digest`
  - `devkit_github_info`, `devkit_paper_index`, `devkit_experiment_log`
- **Safe Sync Engine** — Four-tier sync policies: Mirror / Conservative / Rebase / Merge
  - Pre-sync safety assessment (dirty, diverged, detached HEAD detection)
  - Dry-run preview with per-repo recommendations
  - Async batch sync with concurrency control and timeout
- **Registry & Indexing** — SQLite-backed workspace registry
  - Automatic Git + non-Git workspace discovery
  - Schema migrations with automatic backup snapshots
  - GitHub Stars cache with TTL and historical tracking
  - Tantivy full-text index for repository knowledge search
- **Health Monitoring** — Workspace-wide health checks
  - Git status tracking (dirty / ahead / behind / diverged)
  - Blake3 hash snapshots for non-Git workspaces
  - Environment tool version detection
- **i18n** — Chinese and English bilingual support
- **CI/CD** — GitHub Actions workflow for check, test, fmt, clippy on Windows

### Engineering

- Modular architecture: 22 crates modules with clear separation of concerns
- Dual lib+bin mode: `lib.rs` exports all modules for programmatic use
- Theme system with semantic color tokens (dark/light ready)
- Render layer split from monolithic 1026-line file into 6 focused submodules

### Security

- `cargo audit` clean (0 vulnerabilities in direct dependencies)

[0.20.1]: https://github.com/juice094/devbase/releases/tag/v0.20.1
[0.20.0]: https://github.com/juice094/devbase/releases/tag/v0.20.0
[0.19.0]: https://github.com/juice094/devbase/releases/tag/v0.19.0
[0.18.0]: https://github.com/juice094/devbase/releases/tag/v0.18.0
[0.1.0]: https://github.com/juice094/devbase/releases/tag/v0.1.0
