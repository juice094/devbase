# devbase Architecture Analysis

**Date:** 2026-04-30  
**Scope:** `C:\Users\22414\dev\third_party\devbase`  
**Commit:** `d0eb774` (main)  
**Analyst:** Senior Rust Architect (sub-agent)

---

## 1. Executive Summary

devbase is a monolithic Rust CLI (v0.12.0, Edition 2024) that acts as a developer workspace database and knowledge-base manager. It has 33 top-level modules, a 25-version SQLite schema migration history, and a 37-tool MCP server surface. The architecture suffers from two dominant "god objects" (`WorkspaceRegistry` and `AppContext`), significant schema-to-code drift (several tables are written but never read in production), and a high degree of tight coupling between the CLI layer, the registry, and the MCP tools.

---

## 2. Module Dependency Graph

### 2.1 Top-Level Modules (33)

Declared in `src/lib.rs`:

| Module | Category | Notes |
|--------|----------|-------|
| `arxiv` | Integration | ArXiv paper fetching |
| `asyncgit` | Git wrapper | Async git operations |
| `backup` | Registry ops | Export/import SQLite backups |
| `config` | Config | TOML config with many default fns |
| `core` | **Dead** | Only exports `node.rs`; **zero cross-module usage** |
| `daemon` | Background | Daemon tick loop (health, index, discover, digest) |
| `dependency_graph` | Analysis | Code dependency analysis |
| `digest` | Reporting | Daily knowledge digest |
| `discovery_engine` | Analysis | `discover_dependencies`, `discover_similar_projects` |
| `embedding` | ML infra | Vector embedding helpers |
| `health` | Git ops | Repo health check (`ahead`/`behind`, workspace hash) |
| `i18n` | UI | Localization (en/zh); has dead code |
| `knowledge_engine` | Indexing | Repo summarization and module indexing |
| `mcp` | Protocol | MCP server + 37 tools |
| `oplog_analytics` | Analytics | Operation log analytics |
| `query` | Query engine | CLI query parser and evaluator |
| `registry` | **Core** | 11 sub-modules; dominates the architecture |
| `scan` | Discovery | Directory scanner for git/non-git workspaces |
| `search` | Search | Tantivy vault search + hybrid symbol search |
| `semantic_index` | Indexing | Tree-sitter symbol/call extraction |
| `skill_runtime` | Execution | Skill parser, executor, registry, scoring |
| `skill_sync` | Sync | Vault-to-SKILL.md sync |
| `storage` | Infrastructure | `AppContext`, `StorageBackend` trait |
| `symbol_links` | Analysis | `code_symbol_links` generator; **no production caller** |
| `sync` | Git ops | Repository sync with upstream |
| `sync_protocol` | Protocol | Version vectors, `SyncIndex`; mostly dead code |
| `syncthing_client` | Integration | Syncthing REST API client |
| `tui` | UI | Ratatui interactive UI (optional feature) |
| `vault` | Storage | Vault note scanner, indexer, backlinks |
| `watch` | Infrastructure | File system watcher (optional feature) |
| `workflow` | Orchestration | Workflow engine (YAML-defined pipelines) |

### 2.2 High Fan-Out Modules (Imported by Many Others)

| Module | Approx. # of Dependent Files | Role |
|--------|------------------------------|------|
| `registry` (and sub-modules) | **~46** | Central database access layer |
| `config` | ~10 | Global configuration access |
| `storage` | **~17** | `AppContext` provider |
| `mcp` tools | ~37 tool modules | MCP tool implementations |
| `scan` | ~5 | Entry point for repo discovery |

**Key observation:** `registry` is the absolute hub. Every major feature (health, sync, query, vault, skills, workflows, MCP tools) imports from it. This creates a "big ball of mud" around the `WorkspaceRegistry` struct.

---

## 3. Data Flow: Scan → Index → Entities → Relations → MCP Tools

### 3.1 Happy-Path Flow

```
scan::discover_repos()
  └─> inspect_repo() / inspect_non_git_workspace()
  └─> WorkspaceRegistry::save_repo()          [src/registry/repo.rs:155]
        ├─> upsert_entity_for_repo()          [dual-write to entities table]
        ├─> INSERT repo_tags
        └─> INSERT repo_remotes

knowledge_engine::run_index()
  └─> WorkspaceRegistry::save_summary()
  └─> WorkspaceRegistry::save_modules()       [cargo metadata extraction]

health::run_json()
  └─> WorkspaceRegistry::list_repos()
  └─> WorkspaceRegistry::save_health()

discovery_engine::discover_dependencies()
  └─> WorkspaceRegistry::save_relation()      [writes to relations table]

MCP tools (e.g., DevkitQueryTool)
  └─> invoke(args, &mut AppContext)
        └─> AppContext::conn() / pool()
              └─> WorkspaceRegistry::<query>()
```

### 3.2 Bottlenecks & Missing Links

| Stage | Bottleneck / Missing Link | Location |
|-------|---------------------------|----------|
| **Scan → Index** | `knowledge_engine::index_repo()` is only called by the daemon and `commands::simple::run_index`. There is no automatic trigger after `scan --register`. | `src/daemon.rs:106`, `src/commands/simple.rs:36` |
| **Index → Embeddings** | `code_embeddings` table is populated by `save_embeddings()`, but the only production caller is the MCP tool `DevkitEmbeddingStoreTool`. No batch indexing pipeline exists. | `src/registry/knowledge.rs:287` |
| **Entities → Relations** | `relations` table receives writes via `save_relation()`, but **no production code reads from it**. The unified entity model is write-only for relations. | `src/registry/knowledge.rs:68` |
| **Symbol Links** | `symbol_links::generate_and_save_links()` computes `similar_signature` and `co_located` links, but **it has no production caller**. The `code_symbol_links` table is dead weight. | `src/symbol_links.rs:127` |
| **Call Graph** | `semantic_index::save_calls()` writes to `code_call_graph`, but **no production code queries the call graph**. The "who calls X" feature is unimplemented. | `src/semantic_index.rs:759` |
| **Agent Reads** | `agent_symbol_reads` is written by `record_symbol_read()` and read by `get_symbol_read_counts()` for hybrid search boosting, but `record_symbol_read()` has **no production callers**. | `src/registry/knowledge.rs:440` |

---

## 4. Interface Coupling & God Objects

### 4.1 WorkspaceRegistry — The Ultimate God Object

- **Definition:** `src/registry.rs:152` — a struct with only `version: String` and `entries: Vec<RepoEntry>`.
- **Usage pattern:** Every database method is implemented as `impl WorkspaceRegistry { pub fn ... }`, even though the struct carries no connection state. This is a namespace abuse; it should be a plain module or a connection wrapper.
- **Fan-out:** Used in **46 files** across `commands/`, `mcp/tools/`, `registry/`, `workflow/`, `skill_runtime/`, `vault/`, `daemon.rs`, `health.rs`, `query.rs`, `scan.rs`, `sync.rs`, etc.
- **Impact:** Changing any registry method signature triggers recompilation of nearly the entire crate. Unit tests cannot easily mock the registry because it is not a trait.

### 4.2 AppContext — The Global State Bucket

- **Definition:** `src/storage.rs:75`
```rust
pub struct AppContext {
    pub storage: Arc<dyn StorageBackend>,
    pub config: crate::config::Config,
    pool: Pool<SqliteConnectionManager>,
}
```
- **Fan-out:** Used in **17 files**: `main.rs`, `commands/simple.rs`, `commands/skill.rs`, `commands/workflow.rs`, `commands/limit.rs`, `mcp/mod.rs`, `mcp/tools/*.rs`, `tui/mod.rs`, `tui/state.rs`.
- **Coupling:** The `McpTool` trait hardcodes `&mut crate::storage::AppContext` in its `invoke` method (`src/mcp/mod.rs:28`). This makes MCP tools impossible to test without constructing a full `AppContext` (which initializes SQLite pools and reads config from disk).

### 4.3 McpToolEnum — The Giant Enum

- **Definition:** `src/mcp/mod.rs:56` — an enum with **37 variants**, one per tool.
- **Anti-pattern:** Every tool addition requires editing `McpToolEnum`, `impl McpTool for McpToolEnum`, `build_server_with_tiers()`, and the CLI enum in `main.rs`. This is the "expression problem" in reverse.
- **Line count:** The `invoke`/`schema`/`name` match blocks alone span ~250 lines of pure boilerplate.

---

## 5. Schema vs Code Alignment

### 5.1 Tables with No Production Read Path (Write-Only / Dead)

| Table | Schema Location | Write Location | Read Location | Status |
|-------|-----------------|----------------|---------------|--------|
| `relations` | `migrate.rs:641` | `knowledge.rs:68` | **None** (only tests) | 🔴 Dead reads |
| `code_call_graph` | `migrate.rs:461` | `semantic_index.rs:759` | **None** (only tests) | 🔴 Dead reads |
| `code_symbol_links` | `migrate.rs:531` | `symbol_links.rs:127` | `knowledge.rs:490` | 🟡 Writer never called in production |
| `ai_discoveries` | `migrate.rs:201` | `knowledge.rs:86` | **None** (only tests) | 🔴 Dead reads |
| `entity_types` | `migrate.rs:607` | Migration seed | **None** | 🔴 Dead reads |
| `repos` | `migrate.rs:67` | Migration legacy | Dropped at v21 | 🟡 Ghost table (recreated by `CREATE TABLE IF NOT EXISTS`, then dropped at end of `init_db_at`) |

### 5.2 Tables with Data but Fragmented Query Paths

| Table | Production Queries | Gaps |
|-------|-------------------|------|
| `entities` | `list_repos`, `list_vault_notes`, `list_papers`, `list_workflows` | No generic entity list or filter by `entity_type` outside hardcoded queries. |
| `repo_notes` | `query.rs:429` (for `note:` conditions) | No CLI command or MCP tool to add/view notes. Only `query` uses it. |
| `repo_modules` | `knowledge.rs:50` (list) | Only written during scan; no MCP tool exposes module data. |
| `repo_relations` | Migrated to `relations` at v24 | Legacy table still exists but should be considered deprecated. |
| `workflow_executions` | `workflow/state.rs` (CRUD) | Used internally by workflow engine; no external query tool. |
| `skill_executions` | `skill_runtime/registry.rs` | Used for scoring; no direct CLI list command. |

### 5.3 Schema Versioning Debt

- `CURRENT_SCHEMA_VERSION = 25` (`src/registry/migrate.rs:5`).
- Migrations v1–v25 are all inline in a single 1273-line function (`init_db_at`).
- The `repos` table is a **ghost**: `CREATE TABLE IF NOT EXISTS repos` still appears at line 67, but v21 drops it unconditionally at line 1014. This is defensive against stale binaries but adds confusion.

---

## 6. Unused / Dead Code

### 6.1 `#[allow(dead_code)]` Items

| File | Line | Item | Why It Is Dead |
|------|------|------|----------------|
| `src/i18n/mod.rs` | 3, 11, 67, 78, 121, 152 | Multiple `set_xxx` fns | i18n strings are loaded once at startup; setters are never called |
| `src/sync_protocol.rs` | 22, 35, 58 | `VersionVector::update`, `merge`, `compare` | Core logic for a future P2P sync protocol; no production caller |
| `src/watch.rs` | 10, 67, 116 | `FolderScheduler` fields/methods | Partially implemented watch system |
| `src/registry/repo.rs` | 216 | `update_repo_last_synced_at` | No CLI or sync code calls it (sync writes health, not last_synced_at) |
| `src/registry/repo.rs` | 228 | `list_workspaces_by_tier` | No CLI command or MCP tool uses tier filtering |
| `src/tui/theme.rs` | 65 | A color constant | Likely reserved for future TUI theming |

### 6.2 Modules with Zero Production Callers

| Module | Files | Evidence |
|--------|-------|----------|
| `core` | `src/core/mod.rs`, `src/core/node.rs` | `grep -r 'crate::core'` returns **zero** hits outside its own directory. The `Node`/`Edge` types are entirely unused. |
| `symbol_links` | `src/symbol_links.rs` | `generate_and_save_links()` has no callers in production code. Only tests exercise it. |
| `sync_protocol` | `src/sync_protocol.rs` | `scan_directory()` is used only in tests. `VersionVector` methods are `#[allow(dead_code)]`. |

### 6.3 Functions with No Production Callers

- `WorkspaceRegistry::update_repo_last_synced_at()` (`registry/repo.rs:216`)
- `WorkspaceRegistry::list_workspaces_by_tier()` (`registry/repo.rs:228`)
- `WorkspaceRegistry::record_symbol_read()` (`registry/knowledge.rs:440`) — called by tests, never by production
- `symbol_links::generate_and_save_links()` (`symbol_links.rs:127`)
- `semantic_index::save_calls()` (`semantic_index.rs:759`) — called by tests and the indexing pipeline, but the data is never queried

---

## 7. Circular Dependencies

### 7.1 Compile-Time Cycles

**No module-level circular `use` dependencies were detected.** The crate compiles because all modules are in the same crate, and Rust allows intra-crate cycles as long as they do not form `use` loops at the item level.

### 7.2 Logical / Architectural Cycles

**`storage ↔ registry` tight coupling:**

- `storage::AppContext::with_defaults()` calls `crate::registry::WorkspaceRegistry::init_db_at()` (`storage.rs:87`).
- `registry::WorkspaceRegistry::db_path()` calls `crate::storage::DefaultStorageBackend {}.db_path()` (`registry/migrate.rs:9`).

This is a two-way logical dependency: the storage layer knows about the registry's initialization logic, and the registry knows about the default storage backend. In a clean architecture, `storage` should initialize the DB path independently, and `registry` should receive a `&Connection` or a connection factory.

**`commands ↔ mcp ↔ registry` cycle:**
- `commands::simple::run_mcp()` launches `mcp::run_stdio()`.
- `mcp::run_stdio()` constructs its own `AppContext` (`mcp/mod.rs:533`), duplicating the initialization logic from `main.rs`.
- Both `commands` and `mcp` tools call `registry` methods directly.

This means the MCP server is not truly a separate transport layer; it is a second CLI entry point that re-implements context setup.

---

## 8. Risk Areas & Recommendations

### 8.1 Immediate Risks

1. **God Object Monolith:** `WorkspaceRegistry` should be split into trait-based repositories (e.g., `RepoRepository`, `VaultRepository`, `SkillRepository`). This enables mocking and reduces recompilation.
2. **Dead Schema Tables:** `relations`, `code_call_graph`, `ai_discoveries`, and `code_symbol_links` are accumulating data (or schema space) with no read path. Either implement the read features or drop the tables to reduce cognitive load.
3. **MCP Tool Scalability:** Adding a 38th tool requires ~8 edits across enum, match arms, tier mapping, and CLI routing. Consider a dynamic registration pattern (e.g., `HashMap<String, Box<dyn McpTool>>`) or a derive macro.

### 8.2 Medium-Term Refactors

1. **Extract `AppContext` from `McpTool` trait:** Replace `&mut AppContext` with a smaller `ToolContext` trait that only exposes `conn()` and `config()`. This decouples MCP tools from the full storage backend.
2. **Unified Query Layer:** The `entities` table was designed to be the single source of truth (v16/v20/v21), but most queries still hardcode `entity_type = 'repo'` with `json_extract`. A generic `EntityRepository` with typed queries would align schema and code.
3. **Remove `core` module:** Since `Node` and `Edge` are unused, delete `src/core/` to reduce noise.
4. ** Consolidate migration logic:** The 1273-line `init_db_at` function should be broken into per-version migration modules or use a migration runner library.

---

## 9. Appendix: File Index for Key Artifacts

| Concern | Primary File(s) |
|---------|-----------------|
| Module declarations | `src/lib.rs` |
| CLI routing | `src/main.rs` |
| Central DB methods | `src/registry.rs`, `src/registry/repo.rs`, `src/registry/knowledge.rs`, `src/registry/health.rs`, `src/registry/vault.rs`, `src/registry/metrics.rs`, `src/registry/known_limits.rs`, `src/registry/knowledge_meta.rs`, `src/registry/workspace.rs`, `src/registry/links.rs` |
| Schema & migrations | `src/registry/migrate.rs` |
| AppContext / StorageBackend | `src/storage.rs` |
| MCP server & tool enum | `src/mcp/mod.rs` |
| MCP tool declarations | `src/mcp/tools/mod.rs` |
| CLI command handlers | `src/commands/simple.rs`, `src/commands/skill.rs`, `src/commands/workflow.rs`, `src/commands/limit.rs` |
| Config | `src/config.rs` |
| Dead / unused modules | `src/core/mod.rs`, `src/core/node.rs`, `src/symbol_links.rs`, `src/sync_protocol.rs` |

---

*End of report.*
