# devbase v0.4.0 — AI Skill Orchestration Infrastructure

**Full Changelog**: https://github.com/juice094/devbase/compare/v0.3.0...v0.4.0

## What's New

### Schema v16: Unified Entity Model
- **New tables**: `entity_types`, `entities`, `relations`
- **Progressive dual-write**: `install_skill` / `uninstall_skill` atomically syncs to `entities` table
- **Reserved**: `workflows` + `workflow_executions` tables for v0.5.0 Workflow Engine

### Skill Auto-Discovery (`devbase skill discover`)
- Analyze any project directory and auto-generate a **SKILL.md** + **entry_script** wrapper
- **Project type detection**: Cargo.toml → Rust, package.json → Node, pyproject.toml → Python, go.mod → Go, Dockerfile → Docker
- **CLI surface extraction**: Rust bin targets, npm scripts, Python console scripts
- **Taxonomy inference**: Auto-categorizes as `ai`, `dev`, `data`, `infra`, or `communication` (with sub-categories like `dev/cli`, `ai/agent`)

### Git URL Discover
- `devbase skill discover https://github.com/owner/repo.git`
- Clones → analyzes → registers as Skill in one command

### Workflow DSL Specification (v0.4.0-reserved)
- YAML Schema frozen in `docs/architecture/workflow-dsl.md`
- Supports: skill invocation, sub-workflows, parallel execution, conditions, loops
- Engine implementation deferred to **v0.5.0**

### MCP: 35 Tools
- New: `devkit_skill_discover` — expose auto-discovery to AI agents

### Executor Fix
- `skill run` now passes arguments as **JSON via stdin** to entry scripts, fixing the interface mismatch with discover-generated wrappers

## Assets

| File | Size | Description |
|------|------|-------------|
| `devbase.exe` | ~22.9 MB | Windows x86_64 release binary |

## Verification

```bash
devbase --version  # devbase 0.4.0-alpha
devbase skill discover . --dry-run
devbase skill list
```

## Tests

- **244 passed, 0 failed, 3 ignored**
