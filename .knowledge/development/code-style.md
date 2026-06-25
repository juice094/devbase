---
type: StyleGuide
title: devbase 代码风格与提交规范
description: rustfmt 配置、Conventional Commits、源文件头、架构红线。
timestamp: 2026-06-25T11:15:50Z
tags: [development, style, commits, guidelines]
---

# devbase 代码风格与提交规范

## 格式化

`rustfmt.toml`：

```toml
edition = "2024"
max_width = 100
chain_width = 80
fn_call_width = 80
struct_lit_width = 30
array_width = 80
reorder_imports = true
```

## 提交规范（Conventional Commits）

```
feat:     新功能
fix:      Bug 修复
docs:     文档更新
refactor: 重构（无行为变更）
test:     测试相关
chore:    构建/工具链
perf:     性能优化
```

示例：

```
feat(mcp): add devkit_skill_validate tool
```

## 源文件头

新增源文件应在顶部包含 SPDX 许可证头：

```rust
// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (c) 2026 juice094
```

> 历史文件可能仍使用 `MIT` SPDX 头，新文件统一使用 AGPL。

## 工具使用约定

- **读文件**：优先使用 `Read` 工具；不要直接用 `cat`/`head`。
- **搜索**：优先使用 `Grep`/`Glob`；不要直接用 shell `grep`/`find`。
- **小修改**：使用 `Edit`。
- **整文件/新建**：使用 `Write`。
- **多文件操作/构建/测试**：使用 `Bash`。

## 架构红线速查

完整清单见 [.knowledge/architecture/invariants.md](../architecture/invariants.md)。

- G5：生产代码禁止 `unwrap()` / `expect()` / `panic!()`。
- G1：禁止新增全局硬编码路径，走 `StorageBackend` / `AppContext`。
- G4：`main.rs` 不得超过 1000 行。
- G3：`SCHEMA_DDL` 与 `migrate.rs` 必须同步。
