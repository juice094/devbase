# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

devbase is a Rust workspace (edition 2024, Rust 1.95+) that compiles a developer's local workspace (Git repos, notes, skills, workflows) into structured context consumable by AI agents. It exposes 71 MCP tools over stdio, provides a ratatui terminal dashboard, and maintains a local SQLite registry with Tantivy BM25 + vector search.

**Primary dev platform**: Windows 11 (CI runs on `windows-latest`). Linux/macOS are community-supported.

**License**: AGPL-3.0-or-later / dual-licensed commercial. New source files must include the SPDX header.

---

## Build, Test, and Lint

### Core commands
```bash
# Full build (release)
cargo build --release

# Run all tests (lib + integration + examples + bins)
cargo test --all-targets

# Run a specific test (note: .cargo/config.toml pins RUST_TEST_THREADS=1 locally)
cargo test <test_name> -- --test-threads=1 --nocapture

# Lint (zero warnings enforced in CI)
cargo clippy --all-targets -D warnings

# Format check
cargo fmt --check
```

### Feature flags
- **Default**: `tui`, `mcp`, `lang-rust`, `lang-python`, `lang-js-ts`, `lang-go`
- **Optional**: `embedding` (Candle/Ollama backends), `greptimedb`, `watch`
- Build without TUI/MCP: `cargo build --no-default-features`

### Workspace crates
The `crates/` directory holds 12 extracted sub-crates (e.g., `devbase-registry`, `devbase-vault-wikilink`, `devbase-workflow-model`). Test or build a single crate:
```bash
cargo test -p devbase-registry
cargo build -p devbase-core-types
```

### Local CI shortcut
```powershell
scripts/ci-local.ps1   # Windows
scripts/ci-local.sh    # Linux/macOS
```

---

## High-Level Architecture

### Three layers
1. **Application/Protocol** — CLI (`commands/`), TUI (`tui/`), MCP Server (`mcp/`)
2. **Semantic/Knowledge** — Registry (`registry/`), Search (`search/`), Knowledge Engine (`knowledge_engine/`), Vault (`vault/`), Workflow (`workflow/`), Skill Runtime (`skill_runtime/`)
3. **Physical/Storage** — SQLite WAL (`registry.db`), Tantivy index, Git (git2), filesystem

### Entry points
- **`src/main.rs`** — CLI only; delegates to `commands/` submodules. Hard ceiling: <1000 lines (RF-4).
- **`src/lib.rs`** — Exports all 30+ modules; the binary is a thin wrapper.

### Key architectural patterns

#### MCP Tool trait
All 71 tools implement `McpTool` in `src/mcp/mod.rs`:
```rust
pub trait McpTool: Send + Sync + Clone {
    fn name(&self) -> &'static str;
    fn schema(&self) -> serde_json::Value;
    async fn invoke(&self, args: serde_json::Value, ctx: &mut AppContext) -> anyhow::Result<serde_json::Value>;
    async fn invoke_stream(...) -> anyhow::Result<Vec<ToolStreamEvent>>;
}
```
Tools are registered in two places:
- `src/mcp/tools/mod.rs` — module declarations and `pub use`
- `src/mcp/mod.rs` — `McpToolEnum` variant + `handle_request` routing

#### Storage abstraction
`StorageBackend` trait (`src/storage.rs`) abstracts DB path, workspace dir, and index path. `AppContext` holds a `dyn StorageBackend` and provides `connection() -> rusqlite::Connection`. This enables hermetic testing with injected temp paths.

#### Registry & Schema migrations
- SQLite WAL mode, `PRAGMA user_version` drives migrations.
- `src/registry/migrate.rs` — `CURRENT_SCHEMA_VERSION = 36`; sequential migration functions.
- `src/registry/test_helpers.rs` — `SCHEMA_DDL` defines the in-memory schema for tests.
- **Critical**: any schema change must update **both** `migrate.rs` **and** `test_helpers.rs` `SCHEMA_DDL` (RF-3).
- Migrations must call `backup::auto_backup_before_migration()` before altering schema.

#### Hybrid search
- **BM25**: Tantivy (`src/search/`) for full-text over code symbols and vault notes.
- **Vector**: SQLite BLOB + custom `cosine_similarity` UDF (`src/registry/migrate.rs`); zero ML runtime dependency in default build.
- Orchestrated in `src/search/hybrid.rs`.

#### Workflow engine
YAML-based DAG executor (`src/workflow/`). 5 step types: `skill`, `subworkflow`, `parallel`, `condition`, `loop`. Parsed into `workflow-model` crate, scheduled topologically, variables interpolated via `workflow-interpolate` crate.

#### Vault / PARA notes
Markdown notes with YAML frontmatter, wikilinks (`[[note]]`, `[[note#heading]]`, `[[note#^block-id]]`). Stored in workspace `vault/` under PARA folders (`00-Inbox`, `01-Projects`, `02-Areas`, `03-Resources`, `04-Archives`, `99-Meta`). BFS graph traversal for backlinks (`src/vault/backlinks.rs`).

#### Skill runtime lifecycle
Discovery (`skill_runtime/discover.rs`) → Install → Execute (`executor.rs`) → Score (`scoring.rs`) → Publish (`publish.rs`). `SKILL.md` frontmatter parsed by `devbase-skill-runtime-parser`. Context-aware execution injects `DEVBASE_ACTIVE_CONTEXT` env var.

#### Sync engine
`src/sync/orchestrator.rs` coordinates batch sync operations. `sync_protocol.rs` and `devbase-sync-protocol` crate define version-vector directory sync. Syncthing integration lives in `devbase-syncthing-client`.

---

## Architecture Guardrails (RF Rules)

These are enforced in CI via `scripts/invariant-checks/run-checks.ps1`. A violation is a blocking failure.

| Rule | Summary |
|------|---------|
| **RF-1** | Dependency injection over global state. No new `dirs::data_local_dir()` / `std::env::var_os` hard-coding. |
| **RF-2** | Hermetic testing. No `std::env::set_var` in tests. Use `tempfile` + injected `StorageBackend`. Tantivy/SQLite FS tests must be serialized. |
| **RF-3** | `SCHEMA_DDL` and `migrate.rs` must stay atomically in sync. |
| **RF-4** | `main.rs` ≤ 1000 lines; CLI commands live in `commands/`. |
| **RF-5** | No cyclic `crate::` dependencies between modules. |
| **RF-6** | **Zero** `unwrap()` / `expect()` / `panic!()` in production code (outside `#[cfg(test)]`). |
| **RF-7** | New modules with >5 internal `crate::` refs cannot be extracted to workspace crates. Re-export files (`src/symbol_links.rs`, etc.) are `RE-EXPORT ONLY`. |

**Additional tiered checks** (from `run-checks.ps1`):
- **T11**: `mcp/tools/*.rs` must not use `rusqlite::Connection` directly. Known exceptions: `repo.rs`, `brief.rs`, `impact.rs`.
- **T12**: `tui/render/` must be pure consumer — no `.execute(`, `.prepare(`, `registry::save/insert/update/delete`.

---

## Testing Conventions

### Test helpers
`src/test_utils.rs` provides:
- `temp_db()` — in-memory SQLite connection with full schema
- `fixture_repo(id, path)` / `fixture_repo_with_tags(...)` — minimal `RepoEntry`

### `git2` testing requirements (RF-2.3)
- Always use `git2::Signature::now("Test", "test@example.com")` instead of `repo.signature()` (CI has no global git identity).
- Always `repo.set_head("refs/heads/main")` and commit to `"refs/heads/main"`; default branch varies by platform.

### Path comparison (RF-2.2)
`TempDir` may return short filenames (`TEMP~1`) while `dunce::canonicalize` returns long filenames. Normalize **both** sides before comparing paths in tests.

### Running tests
Current test count: **605** passed / 7 ignored (from `cargo test --workspace -- --list`).

Local `.cargo/config.toml` sets `RUST_TEST_THREADS = "1"`. CI uses `--test-threads=4`. If you encounter flaky SQLite/Tantivy tests locally, reduce threads:
```bash
cargo test -- --test-threads=1
```

---

## Adding an MCP Tool (Standard Path)

1. Create `src/mcp/tools/<tool_name>.rs`
2. Implement `McpTool` trait
3. Register in `src/mcp/tools/mod.rs` (`pub mod` + `pub use`)
4. Add variant to `McpToolEnum` in `src/mcp/mod.rs`
5. Add routing arm in `src/mcp/mod.rs` `handle_request`
6. Add unit tests in `src/mcp/tests.rs`
7. Update README Tool matrix and `AGENTS.md` tool count

**All state-changing tools must be idempotent** — use `ON CONFLICT ... DO UPDATE` or equivalent upsert logic.

---

## Adding a Schema Migration

1. Add migration function in `src/registry/migrate.rs` (sequential version block)
2. Use `ALTER TABLE ... ADD COLUMN` (SQLite limitation)
3. Call `backup::auto_backup_before_migration()` at the start
4. Update `CURRENT_SCHEMA_VERSION`
5. Mirror changes into `src/registry/test_helpers.rs` `SCHEMA_DDL`
6. Update `AGENTS.md` schema version number

---

## Commit Convention

Conventional Commits: `feat(mcp):`, `fix(registry):`, `refactor(search):`, `docs:`, `perf:`, `test:`.

Pre-commit gate (enforced by CI):
```bash
cargo test --all-targets
cargo clippy --all-targets -D warnings
cargo fmt --check
```

---

## Important Files and Directories

| Path | Purpose |
|------|---------|
| `src/main.rs` | CLI entry (thin wrapper) |
| `src/lib.rs` | Public module exports |
| `src/commands/` | CLI subcommand implementations |
| `src/mcp/mod.rs` | MCP server, `McpTool` trait, tool routing |
| `src/mcp/tools/mod.rs` | Tool module registry |
| `src/registry/migrate.rs` | Schema migrations, `CURRENT_SCHEMA_VERSION` |
| `src/registry/test_helpers.rs` | `SCHEMA_DDL`, in-memory test fixtures |
| `src/storage.rs` | `StorageBackend` trait, `AppContext` |
| `src/search/hybrid.rs` | BM25 + vector hybrid search orchestration |
| `src/workflow/executor.rs` | YAML workflow DAG executor |
| `src/vault/backlinks.rs` | Wikilink BFS graph traversal |
| `crates/` | 12 workspace sub-crates (zero internal coupling) |
| `scripts/invariant-checks/run-checks.ps1` | CI architecture invariant checks |
| `.cargo/config.toml` | Local cargo config (`RUST_TEST_THREADS=1`) |
| `rustfmt.toml` | `edition = "2024"`, `max_width = 100` |

---

## Data Locations (Runtime)

- **Registry DB**: `%LOCALAPPDATA%/devbase/registry.db` (SQLite, WAL mode)
- **Workspace**: `%LOCALAPPDATA%/devbase/workspace/` (vault notes, assets, repo manifests)
- **Config**: `~/.config/devbase/config.toml` (credentials, preferences)
- **Index**: Tantivy indices under workspace dir

These paths are never committed; `.gitignore` covers `*.db`, `.devbase/`, `.env*`, `*.local.toml`.
