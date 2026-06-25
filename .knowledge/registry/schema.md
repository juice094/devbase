---
type: SchemaDefinition
title: devbase Registry Schema 总览
description: SQLite Registry 当前 Schema v36 的核心表与职责说明。
timestamp: 2026-06-25T11:15:50Z
tags: [registry, schema, sqlite]
---

# devbase Registry Schema 总览

> **当前版本**：v36（见 `src/registry/migrate.rs` `CURRENT_SCHEMA_VERSION`）
> **单一事实来源**：`src/registry/migrate.rs`
> **测试同步**：`src/registry/test_helpers.rs` 的 `SCHEMA_DDL`

## 核心表

| 表名 | 职责 | 主要模块 |
|------|------|----------|
| `repos` | Git/非 Git 工作区元数据 | `scan`, `registry::repo` |
| `repo_tags` | 仓库标签多对多关系 | `registry::repo` |
| `repo_remotes` | 仓库远程地址 | `scan`, `registry::repo` |
| `code_symbols` | 代码符号（函数/结构体/枚举等） | `semantic_index`, `registry::code_symbols` |
| `code_embeddings` | 符号/文本的 embedding BLOB | `embedding`, `search/hybrid` |
| `code_call_graph` | 符号间调用关系 | `semantic_index`, `registry::call_graph` |
| `code_symbol_links` | 符号相似性/共位关系 | `symbol_links`, `registry::links` |
| `vault_notes` | Vault Markdown 笔记 | `vault/scanner`, `registry::vault` |
| `oplog` | 操作审计日志 | 所有 scan/sync/health 操作 |
| `papers` | 学术文献元数据 | `arxiv`, `mcp/tools/repo` |
| `experiments` | 实验记录 | `mcp/tools/repo` |
| `skills` | Skill 元数据 | `skill_runtime::registry` |
| `skill_executions` | Skill 执行历史 | `skill_runtime::executor`, `scoring` |
| `entities` | 统一实体模型实例 | `registry::entity` |
| `entity_types` | 可扩展的实体类型定义 | `registry::entity` |
| `relations` | 实体间有向关系 | `registry::relation` |
| `workflows` | 工作流定义 | `workflow::state` |
| `workflow_executions` | 工作流执行记录 | `workflow::state` |

## 统一实体模型（Schema v16+）

```sql
CREATE TABLE entity_types (
    name         TEXT PRIMARY KEY,
    schema_json  TEXT NOT NULL,
    description  TEXT,
    created_at   TEXT NOT NULL
);

CREATE TABLE entities (
    id           TEXT PRIMARY KEY,
    entity_type  TEXT NOT NULL REFERENCES entity_types(name),
    name         TEXT NOT NULL,
    source_url   TEXT,
    local_path   TEXT,
    metadata     TEXT,
    content_hash TEXT,
    created_at   TEXT NOT NULL,
    updated_at   TEXT NOT NULL
);

CREATE TABLE relations (
    id             TEXT PRIMARY KEY,
    from_entity_id TEXT NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
    to_entity_id   TEXT NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
    relation_type  TEXT NOT NULL,
    metadata       TEXT,
    confidence     REAL NOT NULL DEFAULT 1.0,
    created_at     TEXT NOT NULL
);
```

## 存储位置

默认使用用户本地数据目录（可通过 `DEVBASE_DATA_DIR` 覆盖）：

- Windows：`%LOCALAPPDATA%/devbase/`
- Linux：`~/.local/share/devbase/`
- macOS：`~/Library/Application Support/devbase/`

目录内容：

```text
devbase/
├── registry.db          # SQLite Registry（WAL 模式）
├── registry.db-wal
├── search_index/        # Tantivy 全文索引
├── symbol_index/        # Tantivy 代码符号索引
├── backups/             # 自动备份
└── workspace/
    ├── vault/           # PARA 笔记
    └── assets/          # 二进制资源
```
