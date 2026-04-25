# Architecture Pre-Split Evaluation (P2)

> **Status**: Deferred — single crate remains optimal at current scale  
> **Evaluated at**: v0.2.4, 22.7 KLOC, 34 MCP tools, Schema v15  
> **Revisit trigger**: 50+ MCP tools OR compile time > 60s (clean) OR binary size > 20 MB

---

## 1. Current State

| Metric | Value |
|--------|-------|
| Total Rust LOC | ~22,750 |
| Modules (`src/`) | 29 top-level |
| MCP Tools | 34 |
| Schema Version | 15 |
| Clean Build Time* | ~18–25 s (release) |
| Binary Size* | ~8.5 MB (release, stripped) |
| Test Count | 239 passed / 0 failed / 3 ignored |

*Measured on Windows 11, Ryzen 7, Rust 1.94.1.

### Largest Modules (LOC)

| Module | Lines | Domain |
|--------|-------|--------|
| `mcp/tools/repo.rs` | 1,913 | MCP / Repository tools |
| `main.rs` | 1,029 | CLI routing |
| `tui/state.rs` | 928 | TUI state machine |
| `knowledge_engine.rs` | 927 | Knowledge graph |
| `semantic_index.rs` | 920 | Search / Embeddings |
| `dependency_graph.rs` | 735 | Analysis / Call graph |
| `registry/migrate.rs` | 714 | Registry / Schema |
| `query.rs` | 692 | Query engine |
| `mcp/mod.rs` | 601 | MCP transport |
| `scan.rs` | 560 | Discovery |

---

## 2. Split Candidates & Trade-offs

### Candidate A: `devbase-mcp` (MCP server + tools)
- **Contents**: `mcp/`, all `mcp/tools/*.rs`, `mcp/mod.rs`
- **Size**: ~3,500 LOC
- **Pros**: MCP protocol changes are isolated; could be published as a standalone server binary.
- **Cons**: Heavily coupled to `registry`, `query`, `knowledge_engine`, `semantic_index` — would require extracting **public APIs** first.
- **Effort**: Medium-High (2–3 days to define stable internal APIs).

### Candidate B: `devbase-search` (Tantivy + semantic index)
- **Contents**: `semantic_index.rs`, `embedding.rs`, `search.rs`, `discovery_engine.rs`
- **Size**: ~2,200 LOC
- **Pros**: Tantivy and embedding logic are self-contained; useful as a library.
- **Cons**: Still needs `registry` for repo metadata; embedding provider is an external Python script.
- **Effort**: Medium (1–2 days).

### Candidate C: `devbase-registry` (SQLite schema + CRUD)
- **Contents**: `registry/`, `registry/migrate.rs`, `registry/knowledge.rs`, etc.
- **Size**: ~3,000 LOC
- **Pros**: Schema and migrations are the most stable surface; other crates would depend on this.
- **Cons**: `registry` already imports `knowledge_engine` and `semantic_index` for indexing — circular dependency risk.
- **Effort**: High (requires untangling indexing from storage).

### Candidate D: `devbase-skill` (Skill runtime + registry)
- **Contents**: `skill_runtime/`, `skill_sync/`
- **Size**: ~2,000 LOC
- **Pros**: Fastest to extract; already has clear boundaries (parser, executor, registry, dependency).
- **Cons**: Skill registry depends on main `registry` for SQLite connection pooling.
- **Effort**: Low-Medium (1 day).

---

## 3. Recommended Split Order (When Triggered)

If any revisit trigger fires, execute in this order to minimize disruption:

1. **Phase 1 — Extract `devbase-skill`**
   - Lowest coupling, highest independence.
   - Skill runtime already uses `anyhow::Result` and plain structs — minimal API surface.

2. **Phase 2 — Extract `devbase-search`**
   - Requires defining a `SearchIndex` trait that `devbase` implements.
   - Lets `devbase-search` be reused by other projects.

3. **Phase 3 — Extract `devbase-mcp`**
   - Only after `devbase-registry` and `devbase-search` expose stable APIs.
   - MCP tool handlers become thin wrappers over crate APIs.

4. **Phase 4 — Extract `devbase-registry`** (optional)
   - Only if the registry schema stabilizes (no new tables for 3+ releases).
   - Otherwise the internal API churn is not worth the compile-time savings.

---

## 4. Immediate Preparations (No Split Yet)

To make a future split painless, apply these **incremental refactorings** now:

| Task | File(s) | Effort |
|------|---------|--------|
| Define `RegistryConn` trait instead of raw `rusqlite::Connection` | `registry/*.rs` | ½ day |
| Move `McpTool` trait to `lib.rs` and document it | `mcp/tools/mod.rs` | 1 h |
| Add `#[derive(Debug, Clone)]` to all public data structs | `registry/`, `skill_runtime/` | 1 h |
| Gate TUI dependencies behind `tui` feature flag | `Cargo.toml`, `main.rs` | ½ day |
| Gate `git2` / `syncthing` behind `sync` feature flag | `Cargo.toml` | ½ day |

These changes **do not split the crate** but make the split a mechanical cut later.

---

## 5. Decision Matrix

| Condition | Action |
|-----------|--------|
| Compile time > 60 s clean | Start Phase 1 (`devbase-skill`) |
| MCP tools > 50 | Start Phase 3 (`devbase-mcp`) |
| Binary size > 20 MB | Enable feature flags first, then split |
| Schema changes < 1 per release for 3 releases | Consider Phase 4 (`devbase-registry`) |
| External consumer asks for `devbase` as a library | Immediately start Phase 2 (`devbase-search`) |

---

## 6. Conclusion

**Defer split.** At 22.7 KLOC and ~20 s release builds, the single-crate model is still faster to iterate than a workspace. The recommended next 3 milestones remain feature-driven (Waves 21–23), not structural.

When the time comes, `devbase-skill` is the obvious first extraction — it already behaves like a sub-crate in spirit.
