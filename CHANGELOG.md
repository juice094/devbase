# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
- **MCP tool count**: 25 → 27

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
