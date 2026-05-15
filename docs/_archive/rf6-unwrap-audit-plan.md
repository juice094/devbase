# RF-6 Implementation Plan: Eliminate `unwrap()` / `expect()` / `panic!()` from Production Code

> **Project:** devbase
> **Rule:** Production code (`src/**/*.rs` outside `#[cfg(test)]` blocks) must have zero `unwrap()`, `expect()`, `panic!()`.
> **Date:** 2026-05-08
> **Estimated Total Effort:** medium (2–3 hours)

---

## Executive Summary

Scan found **24 occurrences** across **7 files**.

**No function signatures need to change** for 21 of the 24 occurrences.
**2 functions require signature changes** in `semantic_index/mod.rs`:
- `index_repo_full` → `anyhow::Result<(Vec<CodeSymbol>, Vec<CodeCall>)>`
- `index_repo` → `anyhow::Result<Vec<CodeSymbol>>`

Only **2 callers** need updates for the signature changes.

---

## Execution Order (Lowest Risk First)

| Order | File | Occurrences | Risk |
|-------|------|-------------|------|
| 1 | `src/test_utils.rs` | 1 | Low |
| 2 | `src/query.rs` | 1 | Low |
| 3 | `src/search.rs` | 12 | Low |
| 4 | `src/search/hybrid.rs` | 2 | Low |
| 5 | `src/workflow/scheduler.rs` | 4 | Low |
| 6 | `src/discovery_engine.rs` | 2 | Low |
| 7 | `src/semantic_index/mod.rs` | 2 | **High** |

---

## Detailed Plan

### 1. `src/test_utils.rs:9`

```rust
// BEFORE
pub fn temp_db() -> rusqlite::Connection {
    WorkspaceRegistry::init_in_memory().expect("failed to create in-memory db")
}
// AFTER
pub fn temp_db() -> anyhow::Result<rusqlite::Connection> {
    WorkspaceRegistry::init_in_memory()
}
```

### 2. `src/discovery_engine.rs:181–182`

```rust
// BEFORE
let set_a = keywords_map.get(a).expect("repo id from keywords_map keys");
let set_b = keywords_map.get(b).expect("repo id from keywords_map keys");
// AFTER
let set_a = keywords_map.get(a).ok_or_else(|| anyhow::anyhow!("repo id {} missing from keywords_map", a))?;
let set_b = keywords_map.get(b).ok_or_else(|| anyhow::anyhow!("repo id {} missing from keywords_map", b))?;
```

### 3. `src/query.rs:25`

```rust
// BEFORE
let first = value.chars().next().expect("value not empty: checked above");
// AFTER
let first = value.chars().next()?;
```

### 4. `src/search.rs` (12 occurrences)

All `schema.get_field("...").expect("...")` → `schema.get_field("...")?`

Locations: 97, 99, 103, 105, 109, 128, 164, 267, 271, 272, 276, 298

### 5. `src/search/hybrid.rs`

```rust
// BEFORE
return lists.into_iter().next().expect("lists len == 1 checked above");
// AFTER
return lists.remove(0);
```

```rust
// BEFORE
1 => Ok(lists.into_iter().next().expect("lists len == 1 checked above").into_iter().take(limit).collect()),
// AFTER
1 => Ok(lists.remove(0).into_iter().take(limit).collect()),
```

### 6. `src/semantic_index/mod.rs`

Signature changes:
```rust
pub fn index_repo_full(repo_path: &Path) -> anyhow::Result<(Vec<CodeSymbol>, Vec<CodeCall>)>
pub fn index_repo(repo_path: &Path) -> anyhow::Result<Vec<CodeSymbol>> {
    Ok(index_repo_full(repo_path)?.0)
}
```

Line 191: `.expect("failed to spawn index worker")` → `?`
Line 198: `handle.join().unwrap()` → `handle.join().map_err(|e| anyhow::anyhow!("index worker panicked: {:?}", e))?`

Caller updates:
- `knowledge_engine/index.rs:186` → add `?`
- `semantic_index/mod.rs:274` → wrap with `Ok(...?)`
- Test at `semantic_index/mod.rs:602` → add `.unwrap()` (test exempt)

### 7. `src/workflow/scheduler.rs`

| Line | Before | After |
|------|--------|-------|
| 19 | `*in_degree.get_mut(...).expect(...) += 1;` | `let deg = in_degree.get_mut(...).ok_or_else(...)?; *deg += 1;` |
| 36 | `queue.pop_front().expect(...)` | `queue.pop_front().ok_or_else(...)?` |
| 37 | `wf.steps.iter().find(...).expect(...).clone()` | `wf.steps.iter().find(...).ok_or_else(...)?.clone()` |
| 43 | `in_degree.get_mut(child).expect(...)` | `in_degree.get_mut(child).ok_or_else(...)?` |

---

## Verification

```bash
cargo check
cargo test
cargo clippy -- -D clippy::unwrap_used -D clippy::expect_used
```
