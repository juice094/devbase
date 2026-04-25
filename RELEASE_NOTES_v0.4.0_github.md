# devbase v0.4.0 — AI Skill Orchestration Infrastructure

**Full Changelog**: https://github.com/juice094/devbase/compare/v0.3.0...v0.4.0

## 🎯 What's Changed

### New Features

- **Schema v16: Unified Entity Model** (`a4f50f9`)
  - New tables: `entity_types`, `entities`, `relations`
  - Progressive dual-write: Skill registry changes automatically mirror to the unified entity model
  - Reserved: `workflows` + `workflow_executions` tables for future Workflow Engine

- **Skill Auto-Discovery** (`a4f50f9`)
  - `devbase skill discover <path>` — analyze any project and auto-generate SKILL.md + entry_script wrapper
  - Supports **Rust** (Cargo.toml bin targets), **Node.js** (package.json scripts), **Python** (pyproject.toml/setup.py), **Go** (go.mod), **Docker**, and **Generic** fallbacks
  - Auto-infers taxonomy category: `ai`, `dev`, `data`, `infra`, `communication` (+ sub-categories like `dev/cli`, `ai/agent`)

- **Git URL Discover** (`d9c4b06`)
  - `devbase skill discover https://github.com/owner/repo.git`
  - Clone → analyze → register as Skill in one command

- **MCP Tool: `devkit_skill_discover`** (`ddba4e9`)
  - AI agents can now trigger auto-discovery via MCP
  - Total tools: **34 → 35**

### Fixes

- **Atomic Dual-Write Transaction** (`d9c4b06`)
  - `install_skill` now wraps skills + entities writes in a single SQLite transaction, preventing data inconsistency on partial failure

- **Executor JSON Stdin Interface** (`5a2b1b2`)
  - `skill run` now passes arguments as JSON via stdin to entry scripts, fixing the interface mismatch with discover-generated Python wrappers

### Documentation

- **Workflow DSL Specification** (`a4f50f9`)
  - YAML Schema, variable interpolation, error handling strategies frozen in `docs/architecture/workflow-dsl.md`
  - Engine implementation deferred to v0.5.0

- **Project repositioning** (`a4f50f9`)
  - README, ROADMAP, AGENTS.md updated: devbase is now "AI Skill Orchestration Infrastructure" rather than a personal knowledge base

## 🧪 Test Results

```
cargo test --lib
244 passed; 0 failed; 3 ignored
```

## 📦 Assets

| File | Size | Description |
|------|------|-------------|
| `devbase.exe` | ~22.9 MB | Windows x86_64 release binary |

## ⚡ Quick Start

```bash
devbase --version              # devbase 0.4.0-alpha
devbase skill discover .       # auto-package current project
devbase skill list             # view registered skills
```

## 📋 All Changes (v0.3.0 → v0.4.0)

```
a4f50f9 feat: Schema v16 unified entity model + skill discover (v0.4.0-alpha)
d9c4b06 fix: atomic dual-write transaction + Git URL discover support
ddba4e9 feat: MCP devkit_skill_discover tool (35 tools total)
5a2b1b2 fix: executor passes JSON via stdin to entry scripts + MCP discover test
```

**Previous tag**: `v0.3.0`
