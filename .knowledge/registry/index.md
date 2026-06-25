---
type: ConceptIndex
title: devbase Registry 概念索引
description: SQLite Registry 相关概念：Schema、迁移、实体关系、操作规范。
timestamp: 2026-06-25T11:15:50Z
tags: [registry, sqlite, schema, index]
---

# devbase Registry 概念索引

## 概述

Registry 是 devbase 的核心数据结构，使用 SQLite（WAL 模式）持久化所有被管理仓库、笔记、Skill、工作流、代码符号等元数据。

## 关键概念

| 概念 | 文档 | 说明 |
|------|------|------|
| Schema 总览 | [schema.md](./schema.md) | 当前 Schema v36 的核心表与职责 |
| 迁移策略 | [migration-policy.md](./migration-policy.md) | 如何安全地修改 Schema |

## 快速定位

- Schema 定义：`src/registry/migrate.rs`
- 迁移脚本：`src/registry/migrations/v*.rs`
- 测试同步：`src/registry/test_helpers.rs` 的 `SCHEMA_DDL`
- 当前版本常量：`src/registry/migrate.rs` `CURRENT_SCHEMA_VERSION = 36`

## 核心约定

1. 所有 Registry 写入必须留下 OpLog 审计痕迹。
2. Schema 迁移前必须自动生成快照 `backup-YYYYMMDD-HHMMSS.db`。
3. 禁止直接修改现有表的列定义；必须通过迁移脚本。
