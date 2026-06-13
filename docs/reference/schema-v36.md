# Schema v36 参考

> **当前版本**：v36
> **数据库**：`%LOCALAPPDATA%\devbase\registry.db`（SQLite，WAL 模式）
> **外键策略**：`PRAGMA foreign_keys = ON`
> **单一事实来源**：`src/registry/test_helpers.rs` 的 `SCHEMA_DDL` 与 `src/registry/migrate.rs` 的迁移逻辑原子同步

---

## 迁移历史摘要

| 版本 | 关键变更 |
|------|----------|
| v1 | 初始 schema：`repos`、`repo_tags`、`repo_remotes` |
| v5 | `repo_modules`（旧版） |
| v7 | `repo_summaries`（README 摘要 + 关键词） |
| v9 | `code_symbols`（AST 符号） |
| v10 | `code_call_graph`（调用边） |
| v11 | `code_embeddings`（语义向量） |
| v12 | `oplog` 结构化：`event_type`、`duration_ms`、`event_version` |
| v13 | `code_symbol_links`（符号概念链接） |
| v14-v15 | `skills` 表与 `dependencies` 支持 |
| v16 | **统一实体模型**：`entity_types`、`entities`、`relations` |
| v17 | `workflow_executions` 工作流执行 |
| v18 | `known_limits` L3 风险层 |
| v19 | `knowledge_meta` L4 元认知层 |
| v20 | Flat ID namespace：移除 `repo:/skill:` 前缀 |
| v21 | **删除 `repos` 表**；`entities` 成为唯一真相源 |
| v22 | 删除 `vault_notes`/`papers`/`workflows`；重建 `repo_modules` |
| v23 | 清理 `repo_modules_legacy`；`init_db_at` 末尾 drop `repos` 幽灵表 |
| v24 | 激活 `relations` 表：唯一索引 + `repo_relations` → `relations` 迁移 |
| v25 | `agent_symbol_reads` Agent 符号读取行为信号 |
| v26 | `entities` 反规范化：新增 `language`、`workspace_type`、`data_tier`、`last_synced_at`、`stars` 列 |
| v27 | `repo_index_state` 索引状态追踪 |
| v28 | `code_embeddings` 3D 主键 `(repo_id, file_path, symbol_name)` |
| v29 | `orphan_tantivy_docs` Tantivy 孤儿文档补偿日志 |
| v30 | `code_symbols.attributes` 属性列（`#[test]` 等） |
| v31 | `agent_contexts` Agent 会话上下文 |
| v32 | `context_entity_links` 上下文 ↔ 实体多对多关联 |
| v33 | `workflow_executions.context_id` 工作流-会话绑定 |
| v34 | `agent_memories` 向量存储：`embedding`、`embedding_model`、`indexed_at` |
| v35 | `skills_fts` FTS5 虚拟表 + 同步触发器 |
| v36 | `sync_sources` / `sync_log` 可插拔外部 Skill 源 |

---

## 核心实体

### `entity_types` — 实体类型定义

```sql
CREATE TABLE entity_types (
    name            TEXT PRIMARY KEY,
    schema_json     TEXT NOT NULL,
    description     TEXT,
    created_at      TEXT NOT NULL
);
```

内置类型：`repo`、`skill`、`paper`、`vault_note`、`workflow`

### `entities` — 统一实体存储（唯一真相源）

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
    updated_at      TEXT NOT NULL,
    language        TEXT,
    discovered_at   TEXT,
    workspace_type  TEXT DEFAULT 'git',
    data_tier       TEXT DEFAULT 'private',
    last_synced_at  TEXT,
    stars           INTEGER
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
CREATE UNIQUE INDEX idx_relations_unique ON relations(from_entity_id, to_entity_id, relation_type);
```

---

## 仓库相关

### `repo_tags` — 仓库标签多对多

```sql
CREATE TABLE repo_tags (
    repo_id TEXT NOT NULL,
    tag     TEXT NOT NULL,
    PRIMARY KEY (repo_id, tag)
);
CREATE INDEX idx_repo_tags_tag ON repo_tags(tag);
```

### `repo_remotes` — 仓库远程信息

```sql
CREATE TABLE repo_remotes (
    repo_id       TEXT NOT NULL,
    remote_name   TEXT NOT NULL,
    upstream_url  TEXT,
    default_branch TEXT,
    last_sync     TEXT,
    PRIMARY KEY (repo_id, remote_name)
);
```

### `repo_health` — Git 健康状态

```sql
CREATE TABLE repo_health (
    repo_id   TEXT PRIMARY KEY,
    status    TEXT,
    ahead     INTEGER DEFAULT 0,
    behind    INTEGER DEFAULT 0,
    checked_at TEXT
);
```

### `repo_stars_cache` / `repo_stars_history` — Stars 缓存与历史

```sql
CREATE TABLE repo_stars_cache (
    repo_id    TEXT PRIMARY KEY,
    stars      INTEGER,
    fetched_at TEXT
);

CREATE TABLE repo_stars_history (
    repo_id    TEXT,
    stars      INTEGER,
    fetched_at TEXT
);
CREATE INDEX idx_stars_history_repo ON repo_stars_history(repo_id, fetched_at);
```

### `repo_summaries` — 仓库摘要

```sql
CREATE TABLE repo_summaries (
    repo_id     TEXT PRIMARY KEY,
    summary     TEXT,
    keywords    TEXT,
    generated_at TEXT
);
```

### `repo_modules` — 模块结构（cargo metadata）

```sql
CREATE TABLE repo_modules (
    repo_id      TEXT,
    module_name  TEXT,
    module_type  TEXT,
    module_path  TEXT,
    PRIMARY KEY (repo_id, module_name)
);
```

### `repo_relations` — 仓库间关系（遗留，逐步迁移至 `relations`）

```sql
CREATE TABLE repo_relations (
    from_repo_id  TEXT NOT NULL,
    to_repo_id    TEXT NOT NULL,
    relation_type TEXT NOT NULL,
    confidence    REAL DEFAULT 0.0,
    discovered_at TEXT NOT NULL,
    PRIMARY KEY (from_repo_id, to_repo_id, relation_type)
);
```

### `repo_code_metrics` — 代码统计

```sql
CREATE TABLE repo_code_metrics (
    repo_id           TEXT PRIMARY KEY,
    total_lines       INTEGER,
    source_lines      INTEGER,
    test_lines        INTEGER,
    comment_lines     INTEGER,
    file_count        INTEGER,
    language_breakdown TEXT,
    updated_at        TEXT
);
```

### `repo_index_state` — 索引提交状态

```sql
CREATE TABLE repo_index_state (
    repo_id          TEXT PRIMARY KEY,
    last_commit_hash TEXT,
    indexed_at       DATETIME DEFAULT current_timestamp
);
```

---

## 代码分析

### `code_symbols` — AST 符号

```sql
CREATE TABLE code_symbols (
    repo_id     TEXT NOT NULL,
    file_path   TEXT NOT NULL,
    symbol_type TEXT NOT NULL,
    name        TEXT NOT NULL,
    line_start  INTEGER,
    line_end    INTEGER,
    signature   TEXT,
    attributes  TEXT,
    PRIMARY KEY (repo_id, file_path, name)
);
CREATE INDEX idx_code_symbols_repo ON code_symbols(repo_id);
CREATE INDEX idx_code_symbols_name ON code_symbols(name);
CREATE INDEX idx_code_symbols_type ON code_symbols(symbol_type);
```

### `code_call_graph` — 调用图

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
CREATE INDEX idx_call_graph_caller ON code_call_graph(repo_id, caller_file, caller_symbol);
```

### `code_embeddings` — 语义向量

```sql
CREATE TABLE code_embeddings (
    repo_id      TEXT NOT NULL,
    file_path    TEXT NOT NULL,
    symbol_name  TEXT NOT NULL,
    embedding    BLOB NOT NULL,
    generated_at TEXT NOT NULL,
    PRIMARY KEY (repo_id, file_path, symbol_name)
);
```

### `code_symbol_links` — 符号概念链接

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
CREATE INDEX idx_symbol_links_source ON code_symbol_links(source_repo, source_symbol);
CREATE INDEX idx_symbol_links_target ON code_symbol_links(target_repo, target_symbol);
```

---

## Vault

### `vault_repo_links` — Vault 笔记 ↔ 仓库关联

```sql
CREATE TABLE vault_repo_links (
    vault_id TEXT NOT NULL,
    repo_id  TEXT NOT NULL,
    PRIMARY KEY (vault_id, repo_id)
);
```

---

## 学习痕迹与学术

### `ai_discoveries` — AI 发现

```sql
CREATE TABLE ai_discoveries (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    repo_id       TEXT,
    discovery_type TEXT,
    description   TEXT,
    confidence    REAL DEFAULT 0.0,
    timestamp     TEXT NOT NULL
);
```

### `repo_notes` — 仓库 AI 笔记

```sql
CREATE TABLE repo_notes (
    id        INTEGER PRIMARY KEY AUTOINCREMENT,
    repo_id   TEXT NOT NULL,
    note_text TEXT NOT NULL,
    author    TEXT DEFAULT 'ai',
    timestamp TEXT NOT NULL
);
```

### `experiments` — 实验记录

```sql
CREATE TABLE experiments (
    id                TEXT PRIMARY KEY,
    repo_id           TEXT,
    paper_id          TEXT,
    config_json       TEXT,
    result_path       TEXT,
    git_commit        TEXT,
    syncthing_folder_id TEXT,
    status            TEXT,
    timestamp         TEXT NOT NULL
);
CREATE INDEX idx_experiments_repo ON experiments(repo_id);
CREATE INDEX idx_experiments_paper ON experiments(paper_id);
```

---

## 工作区与审计

### `workspace_snapshots` — 非 Git 工作区快照

```sql
CREATE TABLE workspace_snapshots (
    repo_id    TEXT PRIMARY KEY,
    file_hash  TEXT NOT NULL,
    checked_at TEXT NOT NULL
);
```

### `oplog` — 不可变操作日志

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

---

## Skill 系统

### `skills` — Skill 元数据

```sql
CREATE TABLE skills (
    id             TEXT PRIMARY KEY,
    name           TEXT NOT NULL,
    version        TEXT NOT NULL,
    description    TEXT NOT NULL,
    author         TEXT,
    tags           TEXT,
    entry_script   TEXT,
    skill_type     TEXT NOT NULL DEFAULT 'custom',
    local_path     TEXT NOT NULL,
    inputs_schema  TEXT,
    outputs_schema TEXT,
    dependencies   TEXT,
    embedding      BLOB,
    installed_at   TEXT NOT NULL,
    updated_at     TEXT NOT NULL,
    last_used_at   TEXT,
    category       TEXT,
    success_rate   REAL,
    usage_count    INTEGER DEFAULT 0,
    rating         REAL
);
CREATE INDEX idx_skills_type ON skills(skill_type);
```

### `skills_fts` — Skill 全文搜索（FTS5 虚拟表）

```sql
CREATE VIRTUAL TABLE skills_fts USING fts5(
    name,
    description,
    tags,
    category,
    content='skills',
    content_rowid='rowid',
    tokenize='unicode61'
);
```

同步触发器：

```sql
CREATE TRIGGER skills_fts_ai AFTER INSERT ON skills BEGIN
    INSERT INTO skills_fts(rowid, name, description, tags, category)
    VALUES (new.rowid, new.name, new.description, new.tags, new.category);
END;

CREATE TRIGGER skills_fts_ad AFTER DELETE ON skills BEGIN
    INSERT INTO skills_fts(skills_fts, rowid, name, description, tags, category)
    VALUES ('delete', old.rowid, old.name, old.description, old.tags, old.category);
END;

CREATE TRIGGER skills_fts_au AFTER UPDATE ON skills BEGIN
    INSERT INTO skills_fts(skills_fts, rowid, name, description, tags, category)
    VALUES ('delete', old.rowid, old.name, old.description, old.tags, old.category);
    INSERT INTO skills_fts(rowid, name, description, tags, category)
    VALUES (new.rowid, new.name, new.description, new.tags, new.category);
END;
```

### `skill_executions` — Skill 执行历史

```sql
CREATE TABLE skill_executions (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    skill_id    TEXT NOT NULL REFERENCES skills(id) ON DELETE CASCADE,
    args        TEXT,
    status      TEXT NOT NULL,
    stdout      TEXT,
    stderr      TEXT,
    exit_code   INTEGER,
    started_at  TEXT NOT NULL,
    finished_at TEXT,
    duration_ms INTEGER
);
```

### `sync_sources` / `sync_log` — 外部 Skill 源

```sql
CREATE TABLE sync_sources (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    name        TEXT NOT NULL UNIQUE,
    url         TEXT NOT NULL,
    source_type TEXT NOT NULL DEFAULT 'github',
    enabled     INTEGER NOT NULL DEFAULT 1,
    last_sync_at TEXT,
    created_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE sync_log (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    source_name    TEXT NOT NULL,
    status         TEXT NOT NULL,
    skills_added   INTEGER NOT NULL DEFAULT 0,
    skills_updated INTEGER NOT NULL DEFAULT 0,
    error_message  TEXT,
    started_at     TEXT NOT NULL DEFAULT (datetime('now')),
    finished_at    TEXT
);
```

---

## Agent Context 与会话

### `agent_contexts` — Agent 会话上下文

```sql
CREATE TABLE agent_contexts (
    id         TEXT PRIMARY KEY,
    name       TEXT NOT NULL,
    intent     TEXT,
    status     TEXT DEFAULT 'active',
    created_at DATETIME DEFAULT current_timestamp,
    updated_at DATETIME DEFAULT current_timestamp
);
```

### `agent_memories` — 结构化记忆 + 向量

```sql
CREATE TABLE agent_memories (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    context_id      TEXT NOT NULL,
    memory_type     TEXT NOT NULL,
    content         TEXT NOT NULL,
    created_at      DATETIME DEFAULT current_timestamp,
    embedding       BLOB,
    embedding_model TEXT,
    indexed_at      DATETIME,
    FOREIGN KEY (context_id) REFERENCES agent_contexts(id) ON DELETE CASCADE
);
CREATE INDEX idx_agent_memories_context ON agent_memories(context_id);
CREATE INDEX idx_agent_memories_embedding ON agent_memories(context_id, indexed_at) WHERE embedding IS NOT NULL;
```

### `context_entity_links` — 上下文 ↔ 实体关联

```sql
CREATE TABLE context_entity_links (
    context_id TEXT NOT NULL,
    entity_id  TEXT NOT NULL,
    link_type  TEXT NOT NULL DEFAULT 'linked',
    created_at TEXT NOT NULL,
    PRIMARY KEY (context_id, entity_id, link_type)
);
CREATE INDEX idx_context_links_entity ON context_entity_links(entity_id);
```

### `agent_symbol_reads` — 符号读取行为信号

```sql
CREATE TABLE agent_symbol_reads (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    repo_id     TEXT NOT NULL,
    symbol_name TEXT NOT NULL,
    read_at     TEXT NOT NULL,
    context     TEXT
);
CREATE INDEX idx_agent_reads_symbol ON agent_symbol_reads(repo_id, symbol_name);
CREATE INDEX idx_agent_reads_time ON agent_symbol_reads(read_at DESC);
```

### `workflow_executions` — 工作流执行

```sql
CREATE TABLE workflow_executions (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    workflow_id  TEXT NOT NULL,
    inputs_json  TEXT,
    status       TEXT NOT NULL,
    current_step TEXT,
    started_at   TEXT NOT NULL,
    finished_at  TEXT,
    duration_ms  INTEGER,
    context_id   TEXT
);
CREATE INDEX idx_workflow_execs_context ON workflow_executions(context_id);
```

---

## 风险与元认知

### `known_limits` — L3 风险层（Hard Veto / Known Bug）

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
CREATE INDEX idx_known_limits_category ON known_limits(category);
CREATE INDEX idx_known_limits_mitigated ON known_limits(mitigated);
```

### `knowledge_meta` — L4 元认知层

```sql
CREATE TABLE knowledge_meta (
    id              TEXT PRIMARY KEY,
    target_level    INTEGER NOT NULL,
    target_id       TEXT NOT NULL,
    correction_type TEXT,
    correction_json TEXT,
    confidence      REAL DEFAULT 0.0,
    created_at      TEXT NOT NULL
);
CREATE INDEX idx_knowledge_meta_target ON knowledge_meta(target_level, target_id);
```

---

## 索引一致性

### `orphan_tantivy_docs` — Tantivy 孤儿文档补偿日志

```sql
CREATE TABLE orphan_tantivy_docs (
    repo_id     TEXT PRIMARY KEY,
    detected_at DATETIME DEFAULT current_timestamp
);
```

---

## Schema 变更规范

1. **禁止直接修改现有表的列定义**。SQLite 仅支持有限的 `ALTER TABLE`。
2. 新增版本必须在 `src/registry/migrations/v##_*.rs` 中实现，并在 `src/registry/migrations/mod.rs` 注册。
3. 升级前自动调用 `backup::auto_backup_before_migration()` 生成 `backup-YYYYMMDD-HHMMSS.db`。
4. `src/registry/test_helpers.rs` 的 `SCHEMA_DDL` 必须与 `migrate.rs` 的当前 DDL 原子同步（RF-3）。
5. 更新 `AGENTS.md` 与本文档的 Schema 版本号。

---

*本文件应与 `src/registry/migrate.rs` 的 `CURRENT_SCHEMA_VERSION` 保持同步。*
