# Schema v23 参考

> **当前版本**：v23  
> **数据库**：`%LOCALAPPDATA%\devbase\registry.db`（SQLite）  
> **外键策略**：`PRAGMA foreign_keys = ON`，但 `repo_*` 关联表已移除 FK（v21/v22 迁移）

---

## 迁移历史摘要

| 版本 | 日期 | 关键变更 |
|------|------|----------|
| v1 | 2026-04 | 初始 schema：`repos`, `repo_tags`, `repo_remotes` |
| v5 | 2026-04 | `repo_modules`（旧版，含 `module_path`/`public_apis`） |
| v7 | 2026-04 | `repo_summaries`（README 摘要 + 关键词） |
| v9 | 2026-04 | `code_symbols`（AST 符号） |
| v10 | 2026-04 | `code_call_graph`（调用边） |
| v16 | 2026-04 | **统一实体模型**：`entity_types`, `entities`, `relations` |
| v20 | 2026-04 | Flat ID namespace：移除 `repo:/skill:` 前缀 |
| v21 | 2026-04 | **删除 `repos` 表**；`repo_*` 关联表移除 FK |
| v22 | 2026-04 | 删除 `vault_notes`/`papers`/`workflows` 表；重建 `repo_modules`（新 schema） |
| **v23** | 2026-04 | **删除 `repo_modules_legacy`**；`init_db_at` 末尾自动 drop `repos` 幽灵表 |

---

## 核心表

### `entity_types` — 实体类型定义

```sql
CREATE TABLE entity_types (
    name            TEXT PRIMARY KEY,
    schema_json     TEXT NOT NULL,
    description     TEXT,
    created_at      TEXT NOT NULL
);
```

内置类型：`repo`, `skill`, `paper`, `vault_note`, `workflow`

### `entities` — 统一实体存储

```sql
CREATE TABLE entities (
    id              TEXT PRIMARY KEY,
    entity_type     TEXT NOT NULL REFERENCES entity_types(name),
    name            TEXT NOT NULL,
    source_url      TEXT,
    local_path      TEXT,
    metadata        TEXT,
    content_hash    TEXT,
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL
);
CREATE INDEX idx_entities_type ON entities(entity_type);
CREATE INDEX idx_entities_name ON entities(name);
CREATE INDEX idx_entities_source ON entities(source_url);
```

**metadata JSON 示例（repo）**：
```json
{
  "language": "Rust",
  "workspace_type": "git",
  "data_tier": "private",
  "discovered_at": "2026-04-30T00:00:00Z",
  "last_synced_at": "2026-04-30T12:00:00Z",
  "stars": 42
}
```

### `relations` — 实体关系（有向图）

```sql
CREATE TABLE relations (
    id              TEXT PRIMARY KEY,
    from_entity_id  TEXT NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
    to_entity_id    TEXT NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
    relation_type   TEXT NOT NULL,
    metadata        TEXT,
    confidence      REAL NOT NULL DEFAULT 1.0,
    created_at      TEXT NOT NULL
);
CREATE INDEX idx_relations_from ON relations(from_entity_id);
CREATE INDEX idx_relations_to ON relations(to_entity_id);
CREATE INDEX idx_relations_type ON relations(relation_type);
```

---

## 关联表（以 `repo_id` 关联 `entities.id`）

| 表名 | 说明 | 关键列 |
|------|------|--------|
| `repo_tags` | 仓库标签 | `repo_id`, `tag` |
| `repo_remotes` | 远程仓库信息 | `repo_id`, `remote_name`, `upstream_url`, `default_branch` |
| `repo_health` | Git 健康状态 | `repo_id`, `ahead`, `behind`, `checked_at` |
| `repo_summaries` | README 摘要 | `repo_id`, `summary`, `keywords`, `generated_at` |
| `repo_modules` | Cargo target / 模块结构 | `repo_id`, `module_name`, `module_type`, `module_path` |
| `repo_relations` | 仓库间关系 | `from_repo_id`, `to_repo_id`, `relation_type`, `confidence` |
| `repo_notes` | AI 发现笔记 | `repo_id`, `note_text`, `author`, `timestamp` |
| `repo_code_metrics` | 代码统计 | `repo_id`, `total_lines`, `source_lines`, `test_lines`, ... |
| `repo_stars_cache` | GitHub stars 缓存 | `repo_id`, `stars`, `fetched_at` |
| `repo_stars_history` | Stars 历史趋势 | `repo_id`, `stars`, `fetched_at` |
| `vault_repo_links` | Vault 笔记 ↔ 仓库链接 | `vault_id`, `repo_id` |

> **注意**：这些表在 v21 移除了 `REFERENCES repos(id)` 外键约束，因为 `repos` 表已废弃。`repo_id` 列语义上引用 `entities.id`，但无数据库级 FK。

---

## 代码索引表

### `code_symbols`

```sql
CREATE TABLE code_symbols (
    repo_id     TEXT NOT NULL,
    file_path   TEXT NOT NULL,
    symbol_type TEXT NOT NULL,
    name        TEXT NOT NULL,
    line_start  INTEGER,
    line_end    INTEGER,
    signature   TEXT,
    PRIMARY KEY (repo_id, file_path, name)
);
CREATE INDEX idx_code_symbols_repo ON code_symbols(repo_id);
CREATE INDEX idx_code_symbols_name ON code_symbols(name);
```

### `code_call_graph`

```sql
CREATE TABLE code_call_graph (
    repo_id       TEXT NOT NULL,
    caller_file   TEXT NOT NULL,
    caller_symbol TEXT NOT NULL,
    caller_line   INTEGER,
    callee_name   TEXT NOT NULL
);
CREATE INDEX idx_call_graph_repo ON code_call_graph(repo_id);
CREATE INDEX idx_call_graph_callee ON code_call_graph(callee_name);
```

### `code_embeddings`

```sql
CREATE TABLE code_embeddings (
    repo_id      TEXT NOT NULL,
    symbol_name  TEXT NOT NULL,
    embedding    BLOB NOT NULL,
    generated_at TEXT NOT NULL,
    PRIMARY KEY (repo_id, symbol_name)
);
```

### `code_symbol_links`

```sql
CREATE TABLE code_symbol_links (
    source_repo   TEXT NOT NULL,
    source_symbol TEXT NOT NULL,
    target_repo   TEXT NOT NULL,
    target_symbol TEXT NOT NULL,
    link_type     TEXT NOT NULL,
    strength      REAL NOT NULL DEFAULT 0.0,
    created_at    TEXT NOT NULL,
    PRIMARY KEY (source_repo, source_symbol, target_repo, target_symbol, link_type)
);
```

---

## 运维与审计表

### `oplog`

```sql
CREATE TABLE oplog (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    operation     TEXT NOT NULL,
    repo_id       TEXT,
    details       TEXT,
    status        TEXT NOT NULL,
    timestamp     TEXT NOT NULL,
    event_type    TEXT,
    duration_ms   INTEGER,
    event_version INTEGER DEFAULT 1
);
CREATE INDEX idx_oplog_operation ON oplog(operation);
CREATE INDEX idx_oplog_timestamp ON oplog(timestamp);
CREATE INDEX idx_oplog_event_type ON oplog(event_type);
CREATE INDEX idx_oplog_repo ON oplog(repo_id);
```

### `known_limits`

```sql
CREATE TABLE known_limits (
    id              TEXT PRIMARY KEY,
    category        TEXT NOT NULL,
    description     TEXT NOT NULL,
    source          TEXT,
    severity        INTEGER,
    first_seen_at   TEXT NOT NULL,
    last_checked_at TEXT,
    mitigated       INTEGER DEFAULT 0
);
```

---

## 已删除的表（历史记录）

| 表名 | 删除版本 | 替代方案 |
|------|----------|----------|
| `repos` | v21 | `entities`（`entity_type = 'repo'`） |
| `vault_notes` | v22 | `entities`（`entity_type = 'vault_note'`） |
| `papers` | v22 | `entities`（`entity_type = 'paper'`） |
| `workflows` | v22 | `entities`（`entity_type = 'workflow'`） |
| `repo_modules_legacy` | v23 | `repo_modules`（新 schema） |
