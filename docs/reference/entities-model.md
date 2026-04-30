# 统一实体模型

> **引入版本**：Schema v16  
> **核心表**：`entity_types` + `entities` + `relations`  
> **目标**：用三张表替代此前分散的 `repos`/`vault_notes`/`papers`/`workflows` 等独立表，实现概念统一、查询统一、扩展统一。

---

## 设计动机

旧 schema 中每新增一个概念就要新增一张表：

```sql
-- 旧方式（v15 及之前）
CREATE TABLE repos (...);
CREATE TABLE vault_notes (...);
CREATE TABLE papers (...);
CREATE TABLE workflows (...);
-- 新增 Memory？→ 再加 CREATE TABLE memories (...)
```

问题：
- **查询碎片化**：查 repo 用 `SELECT * FROM repos`，查 paper 用 `SELECT * FROM papers`，无法统一过滤
- **关系无法通用**：repo 依赖 repo、paper 引用 paper、note 链接 repo —— 每种关系都要单独建表
- **扩展成本高**：新增概念需要改 schema + 写 migration + 改 DAO 层

新方式（v16+）：

```sql
-- 所有对象统一存储
CREATE TABLE entities (
    id          TEXT PRIMARY KEY,
    entity_type TEXT NOT NULL,  -- 'repo' | 'skill' | 'paper' | 'vault_note' | 'workflow' | 自定义
    name        TEXT NOT NULL,
    source_url  TEXT,
    local_path  TEXT,
    metadata    TEXT,           -- JSON，动态字段由 entity_type.schema_json 定义
    ...
);

-- 所有关系统一存储
CREATE TABLE relations (
    from_entity_id TEXT NOT NULL,
    to_entity_id   TEXT NOT NULL,
    relation_type  TEXT NOT NULL, -- 'depends_on' | 'inspired_by' | 'links_to' | 自定义
    confidence     REAL DEFAULT 1.0,
    ...
);
```

---

## 核心查询模式

### 1. 按类型列出实体

```sql
SELECT e.id, e.name, e.local_path, json_extract(e.metadata, '$.language')
FROM entities e
WHERE e.entity_type = 'repo'
ORDER BY e.name;
```

### 2. 获取实体的完整元数据

```sql
SELECT e.id, e.name, e.metadata
FROM entities e
WHERE e.id = 'devbase';
-- metadata: {"language":"Rust","workspace_type":"git","stars":42,...}
```

### 3. 获取实体的关联对象

```sql
-- 获取与 devbase 相关的所有 vault notes
SELECT e.id, e.name
FROM entities e
JOIN relations r ON e.id = r.to_entity_id
WHERE r.from_entity_id = 'devbase'
  AND r.relation_type = 'has_note'
  AND e.entity_type = 'vault_note';
```

### 4. 反向链接（谁引用了我）

```sql
-- 哪些 repo 依赖 devbase？
SELECT e.id, e.name
FROM entities e
JOIN relations r ON e.id = r.from_entity_id
WHERE r.to_entity_id = 'devbase'
  AND r.relation_type = 'depends_on';
```

### 5. 标签过滤 + 全文搜索

```sql
-- 查找 tag 含 "rust" 的 repo
SELECT e.id, e.name
FROM entities e
JOIN repo_tags t ON e.id = t.repo_id
WHERE e.entity_type = 'repo'
  AND t.tag = 'rust';
```

---

## 双轨制过渡期（v16–v22）

v16 引入 `entities` 表后，`repos` 表仍存在了一段时间（dual-write → read-switch → drop）：

| 阶段 | 写入 | 读取 | 时间 |
|------|------|------|------|
| Dual-write | `repos` + `entities` | `repos` | v16–v20 |
| Read-switch | `entities` | `entities` | v20–v21 |
| Drop legacy | `entities` only | `entities` | v21+ |

v21 正式 `DROP TABLE repos`，所有关联表的 FK 约束也一并移除。

---

## 扩展自定义实体类型

```sql
-- 1. 定义新类型
INSERT INTO entity_types (name, schema_json, description, created_at)
VALUES (
    'dataset',
    '{"fields":[{"name":"format","type":"string"},{"name":"size_bytes","type":"integer"}]}',
    'Machine learning dataset',
    '2026-04-30T00:00:00Z'
);

-- 2. 插入实例
INSERT INTO entities (id, entity_type, name, local_path, metadata, created_at, updated_at)
VALUES (
    'imagenet-1k',
    'dataset',
    'ImageNet 1K',
    '/data/imagenet',
    '{"format":"folder","size_bytes":150000000000}',
    '2026-04-30T00:00:00Z',
    '2026-04-30T00:00:00Z'
);

-- 3. 建立关系（dataset 被 paper 引用）
INSERT INTO relations (id, from_entity_id, to_entity_id, relation_type, created_at)
VALUES ('r1', 'paper-resnet', 'imagenet-1k', 'uses', '2026-04-30T00:00:00Z');
```

---

## 相关文档

- [`schema-v23.md`](schema-v23.md) — 完整数据库表结构
- [`context-compiler.md`](../architecture/context-compiler.md) — 情境编译器架构定义
