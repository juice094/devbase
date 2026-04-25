# Contributing to devbase

> 欢迎！devbase 当前为单人维护项目（Bus Factor = 1），你的贡献至关重要。

## 快速开始

### 环境要求

- **Rust**: 1.94.1+（`rustc --version`）
- **OS**: Windows 10/11（主要开发平台），Linux/macOS 社区支持
- **可选**: Python 3.10+（用于 embedding provider）

```powershell
# 克隆
git clone https://github.com/juice094/devbase.git
cd devbase

# 构建
cargo build --release

# 运行测试
cargo test --all-targets

# 代码检查
cargo clippy --all-targets -D warnings
cargo fmt --check
```

### 首次运行

```powershell
# 扫描当前目录的 Git 仓库
cargo run -- scan . --register

# 启动 TUI
cargo run -- tui

# 启动 MCP Server（stdio 模式）
cargo run -- mcp
```

---

## 项目结构

| 目录 | 职责 | 关键文件 |
|------|------|---------|
| `src/main.rs` | CLI 入口 | 所有 subcommand 路由 |
| `src/lib.rs` | 库导出 | 29 个顶级模块 |
| `src/mcp/` | MCP 工具实现 | `tools/` 34 个 tool handler |
| `src/registry/` | SQLite 存储 | `migrate.rs`  schema 迁移 |
| `src/skill_runtime/` | Skill 运行时 | `parser.rs`, `executor.rs`, `dependency.rs` |
| `src/tui/` | 终端界面 | `state.rs`, `render/` |
| `docs/` | 架构文档 | `architecture/pre-split-evaluation.md` |
| `tools/` | 外部工具 | `embedding-provider/skills.py` |
| `vault/` | 笔记模板 | PARA 结构模板 |

> 详细架构决策见 [`ARCHITECTURE.md`](ARCHITECTURE.md)，Agent 上下文约定见 [`AGENTS.md`](AGENTS.md)。

---

## 如何添加 MCP Tool

1. 在 `src/mcp/tools/` 新建或修改对应模块
2. 实现 `McpTool` trait（`name()`, `description()`, `handle()`）
3. 在 `src/mcp/tools/mod.rs` 注册 tool
4. 在 `src/mcp/mod.rs` 的 `handle_request` 中路由
5. **必须**添加单元测试到 `src/mcp/tests.rs`
6. 更新 `README.md` 中的 Tool 矩阵
7. 更新 `AGENTS.md` 中的工具计数

**原则**: 所有状态变更操作必须幂等（`ON CONFLICT ... DO UPDATE`）。

---

## 如何添加 Skill

1. 在 `skills/` 或外部 git 仓库创建 `SKILL.md`
2. 遵循 [Skill 规范](AGENTS.md) 编写 frontmatter（`id`, `name`, `version`, `dependencies`）
3. 入口脚本支持: `py`, `sh`, `ps1`, `js`, 二进制
4. 本地测试: `cargo run -- skill run <skill-id> -- <args>`
5. 发布: `cargo run -- skill publish`（创建 git tag 并推送）

---

## Schema 迁移规范

**绝对禁止**直接修改现有表的列定义。必须遵循：

1. 在 `src/registry/migrate.rs` 新增版本判断块
2. 使用 `ALTER TABLE ... ADD COLUMN`（SQLite 限制）
3. 升级前自动调用 `backup::auto_backup_before_migration()`
4. 在 `src/registry/test_helpers.rs` 的 `SCHEMA_DDL` 同步更新
5. 更新 `AGENTS.md` 的 Schema 版本号
6. 更新 `registry/migrate.rs` 的 `CURRENT_SCHEMA_VERSION`

---

## 子代理协作安全

> **教训**: 多个子代理在同一 Git 工作目录并行执行会导致分支混乱。

- **串行优先**: 子代理任务必须串行，每次 commit 后切回 main
- **目录隔离**: 若必须并行，使用独立 `git clone` 临时目录
- **编译检查**: 任何子代理返回前必须通过 `cargo test --lib`

---

## 提交规范

```
feat: 新功能
fix: Bug 修复
docs: 文档更新
refactor: 重构（无行为变更）
test: 测试相关
chore: 构建/工具链
```

**必须**通过 CI 检查:
- `cargo test --all-targets` — 全绿
- `cargo clippy --all-targets -D warnings` — 零警告
- `cargo fmt --check` — 已格式化

---

## 路线图

见 [`AGENTS.md`](AGENTS.md) 的 "历史 Waves" 和 "当前粗粒度待办"。
