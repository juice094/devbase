# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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

### Changed

- `cargo test --all-targets`: 279 → 286 passed
- MCP tool 总数: 35 → 37

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
- **External Embedding Provider** — Reference Python implementation in `tools/embedding-provider/`
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

[0.1.0]: https://github.com/juice094/devbase/releases/tag/v0.1.0
