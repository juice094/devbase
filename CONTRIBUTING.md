# Contributing to devbase

> 欢迎！无论你是修复一个 typo、提交一个 bug 报告、还是添加一个新的 MCP Tool，你的贡献都让这个项目更好。
>
> devbase 当前为单人维护项目（Bus Factor = 1），你的参与至关重要。

## 项目健康指标

| 指标 | 状态 |
|:---|:---|
| 版本 | v0.8.0 |
| 测试 | 267 passed / 0 failed / 3 ignored |
| Clippy | `-D warnings` 全绿 |
| 生产代码 unwrap | 0 |
| 许可证 | MIT |

---

## 5 分钟上手

### 环境要求

- **Rust**: 1.94.1+（`rustc --version`）
- **OS**: Windows 10/11（主要开发平台），Linux/macOS 社区支持
- **可选**: Python 3.10+（embedding provider）

```powershell
git clone https://github.com/juice094/devbase.git
cd devbase
cargo build --release
cargo test --all-targets
cargo clippy --all-targets -D warnings
cargo fmt --check
```

### 首次体验

```powershell
# 扫描当前目录的 Git 仓库
cargo run -- scan . --register

# 启动 TUI
cargo run -- tui

# 启动 MCP Server
cargo run -- mcp
```

---

## 我想贡献...（决策矩阵）

| 你想做什么 | 入口 | 关键文件 | 必读 |
|:---|:---|:---|:---|
| **报告 bug** | [New Issue](https://github.com/juice094/devbase/issues/new) | — | 本文件 "提交规范" 节 |
| **修复 bug** | 查看 [open issues](https://github.com/juice094/devbase/issues) | `src/` 对应模块 | 下方 "代码规范" |
| **添加 MCP Tool** | `src/mcp/tools/` 新建模块 | `src/mcp/tools/mod.rs`, `src/mcp/mod.rs` | [AGENTS.md](AGENTS.md) "MCP 工具幂等性" |
| **添加 Skill** | `skills/` 或外部 git 仓库 | `SKILL.md` 规范 | [AGENTS.md](AGENTS.md) "Skill 规范" |
| **改进文档** | 直接编辑 `.md` 文件 | `README.md`, `AGENTS.md` | — |
| **重构 / 性能优化** | 先开 Issue 讨论 | — | [ARCHITECTURE.md](ARCHITECTURE.md) |

### 添加 MCP Tool 的标准路径

1. 在 `src/mcp/tools/` 新建模块
2. 实现 `McpTool` trait（`name()`, `description()`, `handle()`）
3. 在 `src/mcp/tools/mod.rs` 注册
4. 在 `src/mcp/mod.rs` 的 `handle_request` 中路由
5. **必须**添加单元测试到 `src/mcp/tests.rs`
6. 更新 `README.md` Tool 矩阵
7. 更新 `AGENTS.md` 工具计数

> **核心原则**: 所有状态变更操作必须幂等（`ON CONFLICT ... DO UPDATE`）。

### 添加 Skill 的标准路径

1. 在 `skills/` 或外部 git 仓库创建 `SKILL.md`
2. 遵循 frontmatter 规范（`id`, `name`, `version`, `dependencies`）
3. 入口脚本支持: `py`, `sh`, `ps1`, `js`, 二进制
4. 本地测试: `cargo run -- skill run <skill-id> -- <args>`
5. 发布: `cargo run -- skill publish`

---

## 代码规范（检查清单）

提交 PR 前，请确认以下检查项：

- [ ] `cargo test --all-targets` — 全绿
- [ ] `cargo clippy --all-targets -D warnings` — 零警告
- [ ] `cargo fmt --check` — 已格式化
- [ ] 新增代码无生产环境 `unwrap`（测试代码除外）
- [ ] Schema 变更已更新 `src/registry/migrate.rs` 和 `SCHEMA_DDL`
- [ ] 新增 Tool 已添加测试和文档

### 提交规范（Conventional Commits）

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

devkit_skill_validate checks SKILL.md frontmatter and entry_script
existence before registration. Returns structured validation report.

- Add SkillValidator struct in src/mcp/tools/skill_validate.rs
- Register in tools/mod.rs and mcp/mod.rs
- Add unit tests for valid/invalid/missing-frontmatter cases
```

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

> ⚠️ **教训**：多个子代理在同一 Git 工作目录并行执行会导致严重的分支混乱。

| 规则 | 说明 |
|:---|:---|
| **串行优先** | 子代理任务必须串行，每次 commit 后切回 main |
| **目录隔离** | 若必须并行，使用独立 `git clone` 临时目录 |
| **编译检查** | 任何子代理返回前必须通过 `cargo test --lib` |
| **禁止共享** | 多个 Agent 绝不能同时操作同一个 `.git` 目录 |

---

## 架构参考

| 文档 | 内容 |
|:---|:---|
| [`ARCHITECTURE.md`](ARCHITECTURE.md) | 三层架构、技术决策记录、模块边界 |
| [`AGENTS.md`](AGENTS.md) | 安全原则、上下文机制、Schema 迁移规范、历史 Waves |
| [`docs/architecture/`](docs/architecture/) | 预拆分评估、Workflow DSL 规范、统一实体模型 |
| [`docs/research/`](docs/research/) | 竞品分析、Embedding 策略 |

---

## 获取帮助

- **Bug 报告**: [GitHub Issues](https://github.com/juice094/devbase/issues/new)
- **功能讨论**: 先开 Issue 描述使用场景，再讨论实现方案
- **实时交流**: Issue 评论区（维护者每日查看）

感谢你的贡献！🦀
