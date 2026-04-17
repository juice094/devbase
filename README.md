# devbase

devbase is a **local-first knowledge base manager** for developer workspaces and AI agent memory.

开发者工作区数据库与知识库管理器。

## Memory Sovereignty

devbase tracks not only Git repositories, but any workspace you treat as knowledge. Data stays local by default; you decide what—if anything—leaves your machine.

## 功能

- **工作区扫描与注册**：自动发现 Git 仓库 **以及** `SOUL.md` / `MEMORY.md` / `.devbase` 标记的非 Git 工作区，持久化到 SQLite
- **GitHub 同步**：批量 fetch/pull 注册仓库的远程更新，支持并发、超时控制、Safe Sync 预检（dirty/diverged/protected 自动跳过）
- **健康检查**：追踪 Git 仓库的 dirty / ahead / behind，以及非 Git 工作区的 blake3 哈希快照变更检测
- **知识日报**：基于仓库健康状态和摘要生成每日简报
- **TUI 交互界面**：基于 ratatui 的终端交互；支持本地/远程 commit 对比、按标签聚类排序
- **MCP Server**：stdio 与 SSE 双传输模式，暴露 `devkit_scan`/`devkit_health`/`devkit_sync`/`devkit_query` 等工具
- **Registry 备份**：`export`/`import`/`backups`/`clean`，schema 迁移前自动快照
- **操作日志 (OpLog)**：`scan`/`sync`/`health` 自动记录，可追溯审计
- **数据分级**：`public` / `cooperative` / `private` 三级，控制同步边界

## 快速开始

```bash
# 扫描并注册当前目录下的所有工作区（Git + 非 Git）
cargo run -- scan . --register

# 批量同步全部仓库（fetch-only）
cargo run -- sync

# 查看工作区健康状态
cargo run -- health --detail

# 生成知识日报
cargo run -- digest

# 启动 TUI
cargo run -- tui

# 查看操作日志
cargo run -- oplog --limit 20

# 导出 registry 备份
cargo run -- registry export --format json

# 启动 MCP SSE Server
cargo run -- mcp --transport sse --port 3001
```

## 非 Git 工作区支持

devbase 不仅管理 Git 仓库，也支持任意被标记的目录：

| 标记文件 | 工作区类型 | 变更检测 |
|---------|-----------|---------|
| `.git/` | `git` | `git2` ahead/behind/dirty |
| `SOUL.md` / `.claude/` | `openclaw` | blake3 哈希快照 |
| `MEMORY.md` / `.devbase` | `generic` | blake3 哈希快照 |

## 依赖

- Rust 2024 edition
- SQLite (bundled via `rusqlite`)
- 可选：`clarity-core`（用于 LLM 驱动的仓库摘要生成）

## 配置

配置文件位于：
- Windows: `%LOCALAPPDATA%\devbase\config.toml`

## 许可证

MIT
