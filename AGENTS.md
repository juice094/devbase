# Agent 环境指引

## 项目概述

`devbase` 是本地优先的开发者工作区与知识库管理器。当前处于 **架构重构稳定期**（Sprint 2 已结束，技术债务清理完成）。

- **技术栈**：Rust 2024, SQLite, tokio, ratatui, git2, reqwest, tantivy
- **Registry DB**：`%LOCALAPPDATA%\devbase\registry.db`（Schema v6，已移除 `ai_queries`/`agri_observations`）
- **MCP Server**：仅 stdio 传输（SSE 已移除，遵循 MCP 2026 规范）
- **当前测试**：44 passed / 0 failed / 2 ignored
- **编译状态**：0 warnings（devbase），clarity-core 外部 warnings 6 个

## 关键约定

1. **文件操作**：读取用 `ReadFile`，搜索用 `Grep`/`Glob`，修改用 `StrReplaceFile`，整文件重写用 `WriteFile`
2. **Shell**：Windows PowerShell；用 `;` 分隔命令
3. **Git**：提交前 `cargo test` 必须全绿；commit message 遵循 `feat/fix/docs/refactor(scope): description`
4. **Schema 迁移**：`PRAGMA user_version` 安全升级；升级前自动调用 `backup::auto_backup_before_migration()`

## 架构变更记录（2026-04-15）

| 变更项 | 旧状态 | 新状态 | Commit |
|--------|--------|--------|--------|
| MCP Transport | stdio + SSE 双传输 | 仅 stdio | `7bf2625` |
| Sync ASYNC 死锁 | Sequential fallback (FIXME) | `spawn_blocking` + 真正并发 | `7bf2625` |
| Registry Schema | v5（12 张表） | v6（9 张表，删除 ai_queries/agri_observations） | `7bf2625` |
| clarity-core | 路径依赖 | 内联移除，纯 reqwest | `7bf2625` |
| Search 框架 | 无 | tantivy bm25 框架（待集成到 Query） | `7bf2625` |

## Sprint 2 历史任务

| 顺序 | 任务 | 状态 | Commit |
|------|------|------|--------|
| 1 | `McpTool::invoke_stream()` trait 扩展 | ✅ 完成 | `df3a908` |
| 2 | SSE handler 流式适配 (`_stream: true`) | ❌ 已移除（SSE deprecated） | `7bf2625` |
| 3 | CLI pagination (`--limit` / `--page`) | ✅ 完成 | `4716faf` |
| 4 | `devkit_health`/`devkit_query` 流式集成 | ❌ 已移除（非标实现） | `7bf2625` |
| 5 | `.syncdone` 文件标记 | ✅ 完成 | `5efde13` |
| 6 | `agri_observations` schema | ❌ 已移除（零使用） | `7bf2625` |
| 7 | Daemon 内置 SSE Server | ❌ 已移除 | `7bf2625` |

## 跨项目接口

- **clarity-core**：已解除路径依赖。devbase 不再被 clarity-core 调用，LLM 能力内联为纯 reqwest
- **syncthing-rust**：`.syncdone` 标记格式已对齐
- **agri-paper**：`agri_observations` 表已删除，后续若需农业数据集成请重新评估 Schema 设计

## 禁止事项

- 不得修改 `dev\third_party\*` 外部仓库
- 不得在没有迁移逻辑的情况下修改 registry schema
- 不得引入新的已 deprecated 协议（如 SSE）
