# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
