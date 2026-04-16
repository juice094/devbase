# devbase

devbase is a **local-first knowledge base manager** for developer workspaces and AI agent memory.

开发者工作区数据库与知识库管理器。

## Memory Sovereignty

devbase tracks not only Git repositories, but any workspace you treat as knowledge. Data stays local by default; you decide what—if anything—leaves your machine.

## 功能

- **仓库扫描与注册**：自动发现本地 Git 仓库并持久化到 SQLite 数据库
- **GitHub 同步**：批量 fetch/pull 注册仓库的远程更新，支持并发与超时控制
- **健康检查**：追踪每个仓库的 dirty / ahead / behind 状态
- **知识日报**：基于仓库健康状态和摘要生成每日简报
- **TUI 交互界面**：基于 ratatui 的终端交互
- **MCP / Daemon / Syncthing 桥接**：面向 AI 工具链和自动化工作流的扩展能力

## 快速开始

```bash
# 扫描并注册当前目录下的 Git 仓库
cargo run -- scan . --register

# 批量同步全部仓库（fetch-only）
cargo run -- sync

# 批量同步并输出 JSON
cargo run -- sync --json

# 生成知识日报
cargo run -- digest

# 启动 TUI
cargo run -- tui
```

## 依赖

- Rust 2024 edition
- SQLite (bundled via `rusqlite`)
- 可选：`clarity-core`（用于 LLM 驱动的仓库摘要生成）

## 配置

配置文件位于：
- Windows: `%APPDATA%\devbase\config.toml`

## 许可证

MIT
