---
type: Policy
title: devbase Registry Schema 迁移策略
description: 修改 SQLite Schema 必须遵循的安全流程与检查清单。
timestamp: 2026-06-25T11:15:50Z
tags: [registry, schema, migration, policy]
---

# devbase Registry Schema 迁移策略

> 违反本策略的 Schema 变更必须回滚。

## 变更流程

```text
1. 在 migrate.rs 新增版本判断块
        ↓
2. 使用 ALTER TABLE ... ADD COLUMN（SQLite 限制）
        ↓
3. 升级前调用 backup::auto_backup_before_migration()
        ↓
4. 同步更新 test_helpers.rs 的 SCHEMA_DDL
        ↓
5. 同步更新 AGENTS.md / .knowledge/ 中的 schema_version
        ↓
6. 跑 cargo test --all-targets
```

## 检查清单

- [ ] `CURRENT_SCHEMA_VERSION` 已递增。
- [ ] 新增/修改的表在 `migrate.rs` 的 `CREATE TABLE IF NOT EXISTS` 中同步。
- [ ] 新增字段通过 `ALTER TABLE ... ADD COLUMN` 或表重建实现。
- [ ] `backup::auto_backup_before_migration()` 在迁移开始前调用。
- [ ] `test_helpers.rs` 的 `SCHEMA_DDL` 与 `migrate.rs` 一致。
- [ ] `oplog_analytics.rs` 中相关的表存在性检查已更新。
- [ ] MCP tool schema 兼容性已审查（G4）。
- [ ] `.knowledge/index.md` 和 `AGENTS.md` 的 `schema_version` 已更新。

## 禁止事项

- 禁止直接修改现有表的列定义。
- 禁止在无迁移逻辑的情况下修改 registry schema。
- 禁止 breaking change 通过修改现有 MCP tool schema 实现（必须新增 tool）。
