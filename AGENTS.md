# Agent 环境指引

## 项目概述

`devbase` 是本地优先的开发者工作区与知识库管理器。当前处于 **v0.2.0**，技术债务已清理，架构具备可持续演进能力。

- **技术栈**：Rust 2024, SQLite, tokio, ratatui, git2, reqwest, tantivy
- **Registry DB**：`%LOCALAPPDATA%\devbase\registry.db`（轻量索引，用户本地，永不进入版本控制）
- **Workspace**：`%LOCALAPPDATA%\devbase\workspace/` —— 文件系统 = source of truth
  - `vault/` —— PARA 结构：00-Inbox, 01-Projects, 02-Areas, 03-Resources, 04-Archives, 99-Meta
  - `assets/` —— 二进制资源
- **MCP Server**：stdio 传输，17 个 tools（含 3 个 vault tools）；配置见 `mcp.json`
- **统一节点模型**：`core::node::{Node, NodeType, Edge}` —— GitRepo / VaultNote / Asset / ExternalLink
- **当前测试**：157 passed / 0 failed / 2 ignored
- **编译状态**：0 warnings / 0 vulnerabilities（`cargo audit` 干净，除上游 `tokei` 的 `RUSTSEC-2020-0163`）

## 关键约定

1. **文件操作**：读取用 `ReadFile`，搜索用 `Grep`/`Glob`，修改用 `StrReplaceFile`，整文件重写用 `WriteFile`
2. **Shell**：Windows PowerShell；用 `;` 分隔命令
3. **Git**：提交前必须通过 `cargo test --all-targets` + `cargo clippy --all-targets -D warnings` + `cargo fmt --check`
4. **Schema 迁移**：`PRAGMA user_version` 安全升级；升级前自动调用 `backup::auto_backup_before_migration()`

## 安全原则

### 本地优先（Local-First）

- **Registry DB** 始终存储在用户的本地配置目录（`dirs::config_dir()/devbase/`），绝不向远程传输
- **代码内容** 不会被上传到任何云端服务（除非用户显式配置 GitHub token 用于 stars 查询）
- **MCP Server** 仅通过 stdio 本地进程通信，不暴露网络端口

### 凭证管理

- GitHub token、LLM API key 存储在本地 `config.toml` 中
- `config.toml` 位于用户配置目录，**不在项目工作目录**，因此不会被意外 `git commit`
- 默认配置模板中的 token 字段使用占位符 `<YOUR_GITHUB_PAT>`，避免真实 token 格式泄露
- `.gitignore` 已覆盖 `*.db`、`.devbase/`、`.env*`、`*.local.toml`

### 审计与备份

- 所有 `scan`/`sync`/`health` 操作自动写入 OpLog（SQLite `oplog` 表）
- Schema 迁移前自动生成 `backup-YYYYMMDD-HHMMSS.db` 快照
- Registry 支持 `export`/`import` 用于用户自主备份

## 架构状态（Wave 5 完成）

| 维度 | 状态 |
|------|------|
| 代码质量 | `rustfmt.toml` + `cargo fmt` + `clippy -D warnings` 全绿 |
| 模块拆分 | `sync`→5 子模块 / `registry`→7 子模块 / `mcp` 测试分离 |
| 库/二进制 | `src/lib.rs` 导出全部 22 个模块；`src/main.rs` 仅 CLI 入口 |
| TUI 架构 | `render/` 6 子模块 + `theme.rs` Design Token + `layout.rs` 响应式引擎 |
| CI/CD | `.github/workflows/ci.yml`：check / test / fmt / clippy on Windows |
| 依赖安全 | `cargo audit` 0 漏洞（除上游 `tokei` 的 `RUSTSEC-2020-0163`） |

## 历史 Waves

| Wave | 主题 | 关键产出 | Commit |
|------|------|---------|--------|
| 1 | 代码质量 | `rustfmt.toml`, clippy 清零 | `4efcd58` |
| 2 | 模块拆分 | `sync/`, `registry/`, `mcp/tests.rs` | `4efcd58` |
| 3 | 工程化 | `src/lib.rs`, CI workflow, `main.rs` 简化 | `4efcd58` |
| 4 | 依赖/审计 | `notify` 8.2.0, `tokei` 14.0.0 | `4efcd58` |
| 5 | TUI 美学与工程学 | 主题系统, Tabs, Help Overlay, Render 拆分 | `6b9be88` |

## 敏感文件清单（禁止提交）

| 文件/模式 | 原因 | .gitignore 覆盖 |
|-----------|------|----------------|
| `*.db` | SQLite 数据库含用户仓库元数据 | ✅ |
| `.devbase/` | 本地 sync 标记和工作区状态 | ✅ |
| `*.log` | 可能含路径或错误堆栈信息 | ✅ |
| `.env*` | 环境变量和 secrets | ✅ |
| `*.local.toml` | 本地覆盖配置 | ✅ |
| `target/` | 构建产物 | ✅ |

## 跨项目接口

- **clarity-core**：已解除路径依赖。devbase 不再被 clarity-core 调用，LLM 能力内联为纯 reqwest
- **syncthing-rust**：`.syncdone` 标记格式已对齐

## 禁止事项

- 不得修改 `dev\third_party\*` 外部仓库
- 不得在没有迁移逻辑的情况下修改 registry schema
- 不得引入已 deprecated 的协议
- **不得在任何源码文件中硬编码真实 token、api_key 或密码**（包括注释和测试数据）
