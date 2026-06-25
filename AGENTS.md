# Agent 环境指引

> 本文件面向不了解项目的 AI coding agent。它汇总了 `devbase` 的架构、构建、测试、安全与开发约定。请在修改代码前先阅读本文件，并遵循其中的红线与检查清单。

## 1. 项目概览

**devbase**（v0.20.1）是一个本地优先的开发者工作空间数据库与知识库管理器。它把代码仓库、笔记（Vault）、Skill 与工作流编译成 AI 可推理的结构化情境，核心职责是：

- **感知**：扫描 Git 仓库、分析代码结构、解析 Vault 笔记。
- **编码**：把原始资产转化为统一实体模型（`Node`/`Edge`）、图谱关系、向量索引。
- **持久化**：本地 SQLite Registry + Tantivy 全文/符号索引 + 文件系统 Workspace。
- **检索**：通过 MCP（Model Context Protocol）向 AI 客户端暴露 71 个工具。

项目主页：`https://github.com/juice094/devbase`  
许可证：AGPL-3.0-or-later（双许可，商业使用需联系作者）。

### 当前关键指标（基于仓库实际内容）

| 指标 | 数值 |
|------|------|
| 版本 | `0.20.1` |
| Rust Edition | `2024`（要求 rustc 1.95+） |
| Registry Schema | `v36`（`src/registry/migrate.rs`） |
| MCP Tools | **71** 个（`src/mcp/tools/*.rs` 中 `pub struct Devkit*Tool`） |
| 测试函数 | **616** 个（`cargo test --workspace -- --list`） |
| Ignored 测试 | 6 个（`#[ignore]` 在 `src/`） |
| Workspace Crates | **12** 个（`crates/` 目录） |
| `src/main.rs` 行数 | 833 行（RF-4 限界 1000 行内） |
| Clippy | `-D warnings` / CI `-W warnings` |
| 生产代码 `unwrap` | 0（架构红线 RF-6） |

> 注意：仓库内原有文档可能写到“18/19 个 crate”“495 tests”“main.rs 515 行”，那是历史快照；本文件以当前文件系统与 `cargo test --workspace -- --list` 的实际输出为准。

## 2. 技术栈与依赖

| 用途 | 技术/库 |
|------|---------|
| CLI / 子命令 | `clap` derive |
| 异步运行时 | `tokio`（rt-multi-thread, macros, process, io-util, io-std, sync） |
| 数据库 | `rusqlite` + `r2d2`/`r2d2_sqlite`（WAL 模式，bundled） |
| 全文/符号检索 | `tantivy` |
| Git 操作 | `git2` |
| 代码解析 | `tree-sitter` + 可选 grammar（rust/python/typescript/go） |
| 终端 UI | `ratatui` + `crossterm`（feature `tui`） |
| 文件监控 | `notify`（feature `watch`） |
| HTTP | `reqwest` / `ureq`（embedding crate） |
| 序列化 | `serde`/`serde_json`/`serde_yaml` + `toml` |
| 日志 | `tracing`/`tracing-subscriber` |
| 哈希/并行 | `blake3`、`rayon`、`crossbeam-channel` |
| 构建/测试辅助 | `tempfile`、`assert_cmd`、`predicates`、`criterion`（bench） |

### Cargo Features

```toml
default = ["tui", "mcp", "lang-rust", "lang-python", "lang-js-ts", "lang-go"]
```

- `tui`：终端仪表盘。
- `mcp`：MCP Server。
- `lang-*`：tree-sitter 语言支持。
- `embedding`：启用 `devbase-embedding` crate（Candle/Ollama），**不在 default**，需显式 `--features embedding`。
- `greptimedb`：可选 GreptimeDB 写入。
- `watch`：目录监控（由 `tui` 间接启用）。

## 3. 仓库布局

```
devbase/
├── Cargo.toml              # 主包 + workspace 定义（members = ["crates/*"]）
├── rustfmt.toml            # 格式化配置
├── mcp.json                # MCP 客户端配置示例
├── .github/workflows/      # CI（check/test/fmt/clippy/audit/invariant）+ Release
├── .githooks/pre-commit    # 提交前 fmt + clippy
├── .cargo/config.toml      # RUST_TEST_THREADS=1
├── src/
│   ├── main.rs             # CLI 入口，仅做命令分发（833 行）
│   ├── lib.rs              # 导出 30+ 模块，条件编译 mcp/tui/watch
│   ├── commands/           # CLI 子命令实现
│   ├── core/               # 原子类型：Node / Edge / NodeType
│   ├── registry/           # SQLite Registry：schema、迁移、实体、关系、健康
│   ├── repository/         # Git 仓库实体抽象
│   ├── search/             # Tantivy 索引、混合检索（BM25 + 向量）
│   ├── semantic_index/     # 语义索引持久化
│   ├── skill_runtime/      # Skill 发现、安装、执行、评分、发布
│   ├── workflow/           # YAML 工作流：解析、校验、调度、执行
│   ├── vault/              # PARA 笔记系统、双向链接、BFS 图、历史
│   ├── mcp/                # MCP Server + 71 个 tools
│   ├── tui/                # ratatui 仪表盘（render/ + state/）
│   ├── sync/               # 仓库同步编排与策略
│   ├── storage.rs          # StorageBackend trait + AppContext（依赖注入容器）
│   ├── config.rs           # 配置与凭证模板
│   ├── i18n/               # 国际化（zh_cn / en）
│   └── ...
├── crates/                 # 12 个独立 workspace crate
│   ├── devbase-core-types
│   ├── devbase-registry
│   ├── devbase-embedding
│   ├── devbase-skill-runtime-types
│   ├── devbase-skill-runtime-parser
│   ├── devbase-symbol-links
│   ├── devbase-sync-protocol
│   ├── devbase-syncthing-client
│   ├── devbase-vault-frontmatter
│   ├── devbase-vault-wikilink
│   ├── devbase-workflow-interpolate
│   └── devbase-workflow-model
├── tests/
│   └── cli.rs              # 11 个集成测试
├── benches/
│   ├── registry_bench.rs
│   ├── semantic_index.rs
│   └── vault_bench.rs
├── skills/                 # 示例 Skill（embed-repo / knowledge-report / search-workspace）
├── scripts/
│   ├── install.ps1 / install.sh
│   ├── devbase-claude.ps1
│   └── invariant-checks/run-checks.ps1
└── docs/                   # 架构文档、ADR、RFC、指南
```

### Workspace Crates 职责

| Crate | 说明 |
|-------|------|
| `devbase-core-types` | 统一实体模型 `Node`/`Edge`/`NodeType`，零内部耦合 |
| `devbase-registry` | SQLite Registry 操作；内部子模块：entity/health/metrics/call_graph/code_symbols/dead_code/relation/workspace |
| `devbase-embedding` | Embedding 生成协议；Candle/Ollama backend、`cosine_similarity` |
| `devbase-skill-runtime-types` | Skill Runtime 类型与枚举 |
| `devbase-skill-runtime-parser` | `SKILL.md` frontmatter 解析 |
| `devbase-symbol-links` | 代码符号链接生成（相似签名、共位关系） |
| `devbase-sync-protocol` | 目录同步协议与版本向量 |
| `devbase-syncthing-client` | Syncthing REST API 客户端 |
| `devbase-vault-frontmatter` | Vault 笔记 frontmatter 解析 |
| `devbase-vault-wikilink` | `[[wiki-link]]` / `[[note#anchor]]` 解析与解析 |
| `devbase-workflow-interpolate` | 工作流变量插值 |
| `devbase-workflow-model` | YAML Workflow 定义类型 |

## 4. 构建、运行与测试

### 环境要求

- **Rust 1.95.0+**
- 主要开发/CI 平台：**Windows**（Linux/macOS 社区支持）
- 可选：`sccache` 可显著加速 tree-sitter grammar 的 C 编译（见 `CONTRIBUTING.md`）

### 常用命令

```powershell
# 构建
cargo build --release

# 本地快速体验
cargo run -- scan . --register
cargo run -- tui
cargo run -- mcp

# 测试（与 CI 一致）
cargo test --all-targets
cargo test --workspace -- --test-threads=4

# 静态检查
cargo clippy --all-targets -D warnings
cargo fmt --check

# 审计
cargo audit

# 架构不变量检查（CI 的 invariant job）
scripts/invariant-checks/run-checks.ps1
```

### 测试策略

- **单元测试**：分布在 `src/**/tests.rs` 与 `#[cfg(test)]` 块中。
- **集成测试**：`tests/cli.rs`，使用 `assert_cmd` + `tempfile`，通过 `DEVBASE_DATA_DIR` 隔离数据目录。
- **Crate 测试**：每个 `crates/*/src/*.rs` 自带测试。
- **Bench**：`criterion` 驱动的 `benches/registry_bench.rs`、`benches/semantic_index.rs`、`benches/vault_bench.rs`。
- **测试隔离**：
  - 所有 IO 测试使用 `TempDir` 与 `StorageBackend` 注入，禁止直接写 `%LOCALAPPDATA%`。
  - `.cargo/config.toml` 默认 `RUST_TEST_THREADS=1`；CI 使用 `--test-threads=4`。
  - `git2` 测试必须显式 `Signature::now("Test", "test@example.com")` 与 `repo.set_head("refs/heads/main")`。
- **网络相关测试**：`crates/devbase-embedding` 中 Candle 测试会下载模型，离线环境会失败；CI/在线环境需保证网络可达。

### 提交前必须通过

```powershell
cargo test --all-targets
cargo clippy --all-targets -D warnings
cargo fmt --check
```

仓库已配置 `.githooks/pre-commit` 执行 `cargo fmt --check` 与 `cargo clippy --all-targets -- -D warnings`。

## 5. 代码风格与约定

### 格式化

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
```

### 源文件头

新增源文件应在顶部包含 SPDX 许可证头（项目主许可证为 AGPL-3.0-or-later）：

```rust
// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (c) 2026 juice094
```

> 注意：仓库内部分历史文件仍使用 `MIT` SPDX 头，新文件统一使用 AGPL。

### 工具使用约定

- **读文件**：优先使用 `Read` 工具；不要直接用 `cat`/`head`。
- **搜索**：优先使用 `Grep`/`Glob`；不要直接用 shell `grep`/`find`。
- **小修改**：使用 `Edit`（按原文件内容精确替换）。
- **整文件/新建**：使用 `Write`。
- **多文件操作/构建/测试**：使用 `Bash`。

### 添加 MCP Tool 的标准路径

1. 在 `src/mcp/tools/` 新建模块。
2. 实现 `McpTool` trait（`name()`、`schema()`、`invoke()`，可选 `invoke_stream()`）。
3. 在 `src/mcp/tools/mod.rs` 注册并 `pub use`。
4. 在 `src/mcp/mod.rs` 的 `McpToolEnum` / 路由中加入该工具。
5. 在 `src/mcp/tests.rs` 添加单元测试。
6. 更新 `README.md` Tool 矩阵与 `AGENTS.md` 工具计数。

**核心原则**：所有状态变更操作必须幂等（`ON CONFLICT ... DO UPDATE`）。

## 6. 数据存储、Schema 与迁移

### 存储位置

默认使用用户本地数据目录（可通过 `DEVBASE_DATA_DIR` 覆盖）：

```
%LOCALAPPDATA%/devbase/          # Windows
~/.local/share/devbase/          # Linux
~/Library/Application Support/devbase/  # macOS
```

目录内容：

```
devbase/
├── registry.db          # SQLite Registry（WAL 模式）
├── registry.db-wal
├── search_index/        # Tantivy 全文索引
├── symbol_index/        # Tantivy 代码符号索引
├── backups/             # 自动备份
└── workspace/
    ├── vault/           # PARA 笔记（00-Inbox, 01-Projects, ...）
    └── assets/          # 二进制资源
```

### Schema 单一事实来源

- `src/registry/migrate.rs`：当前 Schema DDL + 迁移逻辑。
- `src/registry/migrations/v*.rs`：v01 到 v36 的增量迁移脚本。
- `src/registry/test_helpers.rs`：`SCHEMA_DDL` 必须与 `migrate.rs` 保持原子同步。
- `CURRENT_SCHEMA_VERSION = 36`。

### Schema 迁移规范

1. 在 `migrate.rs` 新增版本判断块，使用 `ALTER TABLE ... ADD COLUMN`（SQLite 限制）。
2. 升级前必须调用 `backup::auto_backup_before_migration()` 生成 `backup-YYYYMMDD-HHMMSS.db`。
3. 同步更新 `test_helpers.rs` 的 `SCHEMA_DDL`。
4. 更新 `AGENTS.md` 的 Schema 版本号与 `CURRENT_SCHEMA_VERSION`。

**禁止**：直接修改现有表的列定义；不得在无迁移逻辑的情况下修改 registry schema。

## 7. 架构红线（Architecture Guardrails）

违反任意一条 = **HALT**，转交人类裁决或回滚。完整清单与检测脚本见 `docs/architecture/invariants.md`。

### RF-1：依赖注入优于全局状态

- 禁止新增 `dirs::data_local_dir()` / `std::env::var_os` 硬编码路径。
- 所有 IO 边界路径通过参数、构造函数或 `StorageBackend` trait 注入。
- 例外（Grandfathered）：`backup_dir`、`db_path`、`index_path` 在重构前不得新增第 4 处。

Fitness function：

```bash
grep -rn "dirs::data_local_dir\|std::env::var_os\|std::env::var(\"LOCALAPPDATA\"" src/ \
  | grep -v "backup.rs\|migrate.rs\|search.rs"
# 预期输出：空
```

### RF-2：测试密封性（Hermetic Testing）

- 测试禁止修改全局进程状态（`std::env::set_var`、`static mut`、全局文件系统句柄）。
- 文件系统测试使用 `tempfile` + `StorageBackend` 注入。
- Tantivy / SQLite 文件系统测试必须串行化。
- R2.1 禁止 `DEVBASE_DATA_DIR` 全局注入；R2.2 Windows 路径双端 `dunce::canonicalize`；R2.3 `git2` 测试显式身份与分支。

Fitness function：

```bash
cargo test --test-threads=16
```

### RF-3：Schema 单一事实来源

- `SCHEMA_DDL` 与 `migrate.rs` 必须原子同步。
- CI 运行 `test_in_memory_schema_version` + schema 结构比对。

### RF-4：二进制入口限界

- `main.rs` 不得超过 1000 行；当前 833 行。
- 新增 CLI 命令必须拆分到 `src/commands/` 子模块。

### RF-5：无循环依赖

- 禁止模块间双向 `use crate::` 引用。

### RF-6：生产代码无 panic

- 生产代码禁止 `unwrap()` / `expect()` / `panic!()`（测试代码除外）。
- 状态：当前生产代码 unwrap 计数为 0。

Fitness function：

```bash
for f in $(find src -name "*.rs"); do
  test_line=$(grep -n "#\[cfg(test)\]" "$f" | head -1 | cut -d: -f1)
  if echo "$f" | grep -qE "tests?\.rs$|_test\.rs$|/tests/"; then continue; fi
  if [ -n "$test_line" ]; then
    head -n "$((test_line - 1))" "$f" | grep -n "\.unwrap()"
  else
    grep -n "\.unwrap()" "$f"
  fi
done
# 预期输出：空
```

### RF-7：Workspace 拆分约束

- 新增模块若对 devbase 内部其他模块的 `crate::` 引用超过 5 个，禁止提取为 workspace crate。
- 已提取 crate 的重新导出文件（如 `src/symbol_links.rs`）顶部标有 `RE-EXPORT ONLY`，禁止添加新代码。
- 子 crate 依赖版本必须与 workspace 统一。

### 关键分层不变量（G/T）

| 编号 | 规则 |
|------|------|
| G1 | `registry::WorkspaceRegistry` 不得依赖 Tier 4+ 模块 |
| G3 | 所有状态变更 MCP tool 必须幂等 |
| G4 | Breaking change 只能通过新增 tool 实现，不修改现有 schema |
| G5 | 生产代码不得新增 `unwrap`/`expect`（RF-6） |
| T11 | `mcp/tools/*` 不得直接调用 `rusqlite::Connection`，必须通过 registry 封装（已知例外：`repo.rs`、`repo/nl_query.rs`、`brief.rs`、`impact.rs`） |
| T12 | `tui/render/*` 是纯消费者层，禁止写入 registry |

CI 通过 `scripts/invariant-checks/run-checks.ps1` 检测 G5 / T11 / T12 / README+Cargo.toml 完整性。

## 8. 安全与隐私原则

### 本地优先（Local-First）

- Registry DB 只存在用户本地配置目录，**不向远程传输**。
- 代码内容默认不上云（除非用户显式配置 GitHub token 用于 stars 查询）。
- MCP Server 仅通过 **stdio** 本地进程通信，不暴露网络端口。

### 客户端无关（Client-Agnostic）

- 允许：向通用目录输出数据；实现标准协议（MCP）。
- 禁止：核心能力硬编码特定客户端路径/API/配置；核心能力可用性依赖某个客户端是否安装。
- `scripts/claude/`、`docs/clients/` 属于适配示例，不归入核心版本控制。

### 凭证管理

- GitHub token、LLM API key 存储在本地 `config.toml`（用户配置目录，**不在项目工作目录**）。
- 模板使用占位符 `<YOUR_GITHUB_PAT>`，禁止在源码/注释/测试数据中硬编码真实凭证。
- `.gitignore` 已覆盖 `*.db`、`.devbase/`、`.env*`、`*.local.toml`。

### 审计与备份

- 所有 `scan`/`sync`/`health` 操作自动写入 OpLog（SQLite `oplog` 表）。
- Schema 迁移前自动生成 `backup-YYYYMMDD-HHMMSS.db` 快照。
- Registry 支持 `export`/`import` 用于用户自主备份。

## 9. CLI / MCP / TUI 能力速览

### 顶层 CLI 命令

| 分组 | 命令 |
|------|------|
| 仓库管理 | `scan`、`health`、`status`、`sync`、`query`、`index`、`tag`、`meta`、`repo` |
| 代码分析 | `metrics`、`module-graph`、`call-graph`、`dependency-graph`、`code-symbols`、`dead-code`、`github-info` |
| 知识/Vault | `digest`、`knowledge-report`、`oplog`、`vault`、`ontology` |
| Skill / Workflow | `skill`、`workflow` |
| 系统 | `tui`（feature `tui`）、`mcp`（feature `mcp`）、`daemon`、`watch`（feature `watch`）、`syncthing-push`、`skill-sync`、`limit`、`registry`、`clean`、`version` |

### MCP Server

- 启动：`devbase mcp`
- 传输：**stdio only**
- 工具示例：`devkit_scan`、`devkit_health`、`devkit_sync`、`devkit_query`、`devkit_index`、`devkit_vault_search`、`devkit_skill_run`、`devkit_workflow_run`、`devkit_session_recall`、`devkit_project_brief` 等共 71 个。
- 客户端配置示例见 `mcp.json`：

```json
{
  "mcpServers": {
    "devbase": { "command": "devbase", "args": ["mcp"] }
  }
}
```

### TUI

- 启动：`devbase tui`
- 基于 `ratatui` 的异步事件循环，支持跨仓库导航、安全同步预览、标签聚类、搜索。

## 10. CI/CD 与发布

### CI（`.github/workflows/ci.yml`）

在 Windows runner 上执行：

1. `cargo check --all-targets`
2. `cargo test --lib --tests --bins --examples -- --test-threads=4 --nocapture`
3. `cargo fmt --check`
4. `cargo clippy --all-targets --verbose -- -W warnings`
5. `cargo audit`
6. `scripts/invariant-checks/run-checks.ps1`

### Release（`.github/workflows/release.yml`）

- 触发：推送 `v*` tag。
- 构建 Windows x64 zip 与 Linux x64 tar.gz，附带 `README.md`、`LICENSE`、`CHANGELOG.md`。
- 上传至 GitHub Release。

### 安装脚本

- Windows：`scripts/install.ps1`
- Linux/macOS：`scripts/install.sh`
- Claude Code 启动器：`scripts/devbase-claude.ps1`

## 11. Skill 与工作流

### Skill

- 元数据：目录下的 `SKILL.md`，frontmatter 必须包含 `id`、`name`、`version`、`description`，可选 `dependencies`。
- 入口脚本支持 `py`、`sh`、`ps1`、`js` 或二进制。
- 命令：`skill discover`、`skill run`、`skill install`、`skill publish`、`skill sync`。
- 评分：`success_rate`、`usage_count`、`rating`（0-5）。
- 跨客户端 sync：`skill sync <OUTPUT_DIR> --target <TARGET>...`，支持 `all` / `clarity` / `kimicli` / `claude-code` / `codex` / `claw`。详见 `vault/99-Meta/skillopt-devbase-integration.md`。

### Workflow

- YAML 定义，5 种 step 类型：`skill`、`subworkflow`、`parallel`、`condition`、`loop`。
- 拓扑调度 + batch 并行执行。
- 规范见 `docs/architecture/workflow-dsl.md`。

## 12. 禁止事项

- 不得修改 `dev/third_party/*` 外部仓库。
- 不得在没有迁移逻辑的情况下修改 registry schema。
- 不得引入已 deprecated 的协议。
- **不得在主仓库引入 Spark/Flink 依赖**（研究性质代码必须置于独立仓库）。
- **不得在任何源码文件中硬编码真实 token、api_key 或密码**（包括注释和测试数据）。

## 13. 参考文档

| 文档 | 内容 |
|------|------|
| `README.md` | 项目简介、快速开始、技术栈 |
| `CONTRIBUTING.md` | 贡献指南、构建加速、代码规范、Skill/MCP 添加路径 |
| `docs/architecture/overview.md` | 三层架构、技术决策记录 |
| `docs/architecture/invariants.md` | 完整不变量清单（G/T） |
| `docs/architecture/workflow-dsl.md` | Workflow DSL 规范 |
| `docs/architecture/workspace-as-schema.md` | 统一实体模型 |
| `docs/guides/mcp-integration-guide.md` | MCP 集成指南 |
| `docs/README.md` | 完整文档导航 |
| `docs/ROADMAP.md` | 历史 Waves、功能路线图与讨论 |
| `CHANGELOG.md` | 版本变更日志 |

---

*本文件应随项目结构、Schema 版本、工具数量、测试数量等变更同步更新。*
