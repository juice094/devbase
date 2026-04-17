# Agent 环境指引

## 项目概述

`devbase` 是本地优先的开发者工作区与知识库管理器。当前处于 **Sprint 2（Phase 2）**。

- **技术栈**：Rust 2024, SQLite, tokio, ratatui, axum, git2
- **Registry DB**：`%LOCALAPPDATA%\devbase\registry.db`（Schema v4）
- **MCP Server**：stdio + SSE 双传输
- **当前测试**：42 passed / 0 failed / 2 ignored

## 关键约定

1. **文件操作**：读取用 `ReadFile`，搜索用 `Grep`/`Glob`，修改用 `StrReplaceFile`，整文件重写用 `WriteFile`
2. **Shell**：Windows PowerShell；用 `;` 分隔命令
3. **Git**：提交前 `cargo test` 必须全绿；commit message 遵循 `feat/fix/docs/refactor(scope): description`；Sign-off 必须包含用户邮箱
4. **Schema 迁移**：新增表必须在 `init_db()` 中做 `CREATE TABLE IF NOT EXISTS` + `PRAGMA user_version` 安全升级；升级前自动调用 `backup::auto_backup_before_migration()`

## Sprint 2 优先级

| 顺序 | 任务 | 状态 |
|------|------|------|
| 1 | `McpTool::invoke_stream()` trait 扩展 | 🔴 当前执行 |
| 2 | `agri_observations` schema + `devkit_agri_query` | 🟡 等 agri-paper PR |
| 3 | SSE handler 流式适配 + CLI pagination | 🟡 等 W1 完成 |
| 4 | Daemon 内置 SSE Server | 🟡 Sprint 2 后半 |

## 跨项目接口

- **上游 clarity-core**：通过 MCP 调用 devbase；流式响应仅限 SSE transport
- **下游 syncthing-rust**：`.syncdone` 标记格式已对齐；待其暴露 `FolderStatus::Idle` REST endpoint
- **下游 agri-paper**：农业 tag 命名空间 `agri:*` 已接受；等 DDL PR

## 禁止事项

- 不得修改 `dev\third_party\*` 外部仓库
- 不得删除已有 MCP tool（保持 backward compatible）
- 不得在没有迁移逻辑的情况下修改 registry schema
