# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
