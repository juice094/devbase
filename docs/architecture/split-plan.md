# devbase 大文件拆分方案

> 研究范围：`src/mcp/tools.rs`（1336 行，19 个 tool）、`src/registry/core.rs`（1253 行，schema + CRUD + 测试）
> 目标：提供可执行的拆分计划，零运行时行为变更。

---

## 1. 当前问题分析

### 1.1 `mcp/tools.rs` — 功能域混杂
- **规模**：1336 行，包含 19 个 `McpTool` 实现 + 4 个私有辅助函数。
- **问题**：
  - Repo 管理、Vault 笔记、Knowledge 查询、Project Context 四类工具平铺在同一文件。
  - 新增工具需要大量滚动定位，Code Review 时 diff 难以聚焦。
  - 辅助函数（`nl_filter_repos`、`parse_github_repo` 等）与调用者相距甚远，可读性差。

### 1.2 `registry/core.rs` — 职责过载
- **规模**：1253 行，按职责可粗分为：
  - Schema 迁移与初始化：`db_path` / `workspace_dir` / `init_db`（~540 行）
  - Repo CRUD：`list_repos`、`save_repo`、`update_repo_*` 等（~250 行）
  - Vault CRUD：`save_vault_note`、`list_vault_notes`、`delete_vault_note`（~80 行）
  - Link CRUD：`get_linked_repos`、`get_linked_vaults` 等（~55 行）
  - 测试辅助：`init_in_memory`、`SCHEMA_DDL`（~190 行）
  - 单元测试：`mod tests`（~130 行）
- **问题**：
  - Schema 迁移代码“写一次、极少改动”，却夹在频繁迭代的 CRUD 中间。
  - 测试代码与生产代码耦合在同一文件，增加编译单元体积。
  - 已有 `registry/{health,knowledge,metrics,workspace}.rs` 证明“按实体拆 `impl WorkspaceRegistry`”是该项目的既有风格，`core.rs` 是唯一的例外。

---

## 2. `mcp/tools.rs` 拆分方案

### 2.1 前提约束
Rust 不允许同名的 `tools.rs` 与 `tools/` 目录并存。因此必须：
1. **删除** `src/mcp/tools.rs`
2. 新建目录 `src/mcp/tools/`，并在其中创建 `mod.rs` 作为统一入口

`src/mcp/mod.rs` 中已有 `pub mod tools;`（第 416 行），它会自动解析到 `tools/mod.rs`，**无需修改调用方**。

### 2.2 工具分类

| 域 | 数量 | Tool 名称 |
|---|---|---|
| **Repo** | 13 | `scan`, `health`, `sync`, `index`, `note`, `digest`, `paper_index`, `experiment_log`, `github_info`, `code_metrics`, `module_graph`, `query_repos`, `natural_language_query` |
| **Vault** | 4 | `vault_search`, `vault_read`, `vault_write`, `vault_backlinks` |
| **Knowledge** | 1 | `query` |
| **Context** | 1 | `project_context` |

### 2.3 文件结构

```
src/mcp/
├── mod.rs
└── tools/
    ├── mod.rs          # 统一入口，re-export 所有 tool struct
    ├── repo.rs         # ~950 行：Repo 域 13 tool + 4 helper
    ├── vault.rs        # ~210 行：Vault 域 4 tool
    ├── query.rs        # ~40 行：Knowledge 域 1 tool
    └── context.rs      # ~140 行：Context 域 1 tool
```

### 2.4 各文件职责

#### `tools/mod.rs`
```rust
pub use repo::*;
pub use vault::*;
pub use query::*;
pub use context::*;
```
- 保持 `mcp/mod.rs` 中 `pub use tools::*;` 的语义不变。
- 不暴露私有辅助函数（`parse_github_repo`、`nl_filter_repos` 等仍保持 `repo.rs` 私有）。

#### `tools/repo.rs`（~950 行）
包含以下 Tool 及辅助函数：
- `DevkitScanTool`
- `DevkitHealthTool`
- `DevkitSyncTool`
- `DevkitIndexTool`
- `DevkitNoteTool`
- `DevkitDigestTool`
- `DevkitPaperIndexTool`
- `DevkitExperimentLogTool`
- `DevkitGithubInfoTool`
- `DevkitCodeMetricsTool`
- `DevkitModuleGraphTool`
- `DevkitQueryReposTool`
- `DevkitNaturalLanguageQueryTool`
- 辅助函数：`parse_github_repo`、`nl_filter_repos`、`parse_stars_condition`、`extract_tag_from_query`

> 建议：由于 `repo.rs` 仍接近 1000 行，若未来继续增加 repo 类 tool，可进一步拆分为 `repo_ops.rs`（写/扫描类）与 `repo_query.rs`（查询类），但当前阶段一次拆到 4 个子模块已足够。

#### `tools/vault.rs`（~210 行）
- `DevkitVaultSearchTool`
- `DevkitVaultReadTool`
- `DevkitVaultWriteTool`
- `DevkitVaultBacklinksTool`

#### `tools/query.rs`（~40 行）
- `DevkitQueryTool`（调用 `crate::query::run_json`，纯知识库查询）

#### `tools/context.rs`（~140 行）
- `DevkitProjectContextTool`（跨 repo + vault + assets 的聚合工具）

### 2.5 编译影响评估
- **零 API 变更**：`mcp/mod.rs` 的 `McpToolEnum` 通过 `pub use tools::*;` 引入所有 struct，拆分后枚举定义无需修改。
- **同 crate 内拆分**：不产生新的 crate 边界，对编译时间无实质影响；增量编译粒度略有提升。
- **风险点**：必须严格遵循“先删 `tools.rs`、再建 `tools/`”的顺序，否则触发 `E0761`。

---

## 3. `registry/core.rs` 拆分方案

### 3.1 前提约束
`src/registry.rs` 已与 `src/registry/` 子目录共存，因此只需在 `registry.rs` 中调整 `mod` 声明，**无需文件系统重命名**。

### 3.2 文件结构

```
src/registry/
├── migrate.rs         # schema 迁移 + init_db
├── repo.rs            # Repo CRUD + 原有单元测试
├── vault.rs           # Vault note CRUD
├── links.rs           # vault_repo_links CRUD
├── test_helpers.rs    # #[cfg(test)] init_in_memory + SCHEMA_DDL
├── health.rs          # 已有，不动
├── knowledge.rs       # 已有，不动
├── metrics.rs         # 已有，不动
├── workspace.rs       # 已有，不动
└── (删除 core.rs)
```

`src/registry.rs` 的变更：
```rust
// 删除此行
// mod core;

// 替换为
mod migrate;
mod repo;
mod vault;
mod links;
#[cfg(test)]
mod test_helpers;
```

### 3.3 各文件职责与内容映射

#### `registry/migrate.rs`（~540 行）
从 `core.rs` 迁移：
- `db_path()`
- `workspace_dir()`（含 sample `repos.toml` 初始化）
- `init_db()` — 包含所有 `CREATE TABLE / ALTER TABLE / PRAGMA user_version`、legacy migration（`repos_legacy` → `repos` + `repo_remotes`）、v1–v8 版本升级逻辑。

> 建议：未来若 schema 版本继续增加，可考虑将每个版本的升级逻辑提取为独立私有函数（如 `migrate_v1_to_v2`），但本次拆分不触及函数内部结构，仅做文件级搬运。

#### `registry/repo.rs`（~470 行）
从 `core.rs` 迁移：
- `collect_repos_from_stmt`（私有辅助）
- `list_repos`、`list_repos_stale_health`、`list_repos_need_index`
- `save_repo`
- `update_repo_language`、`update_repo_tier`、`update_repo_workspace_type`、`update_repo_last_synced_at`
- `list_workspaces_by_tier`
- 原有 `mod tests`（第 1123–1253 行）全部移入本文件底部

文件顶部保留 `use super::*;`，与现有 `metrics.rs`、`health.rs` 等保持一致。

#### `registry/vault.rs`（~80 行）
从 `core.rs` 迁移：
- `save_vault_note`
- `list_vault_notes`
- `delete_vault_note`

#### `registry/links.rs`（~55 行）
从 `core.rs` 迁移：
- `get_linked_repos`
- `get_linked_vaults`
- `get_linked_vault_notes`
- `get_linked_repos_full`

#### `registry/test_helpers.rs`（~190 行，`#[cfg(test)]`）
从 `core.rs` 迁移：
- `impl WorkspaceRegistry { pub fn init_in_memory() ... }`
- `const SCHEMA_DDL`

### 3.4 对现有调用者的影响

**零影响**。原因如下：

1. **跨模块 `impl` 块合法**：Rust 允许多个文件对同一类型写 `impl WorkspaceRegistry`，只要类型在作用域内。已有 `registry/{health,knowledge,metrics,workspace}.rs` 均采用此模式。
2. **无直接引用 `core` 模块**：全代码库搜索 `registry::core` 无任何命中；所有调用均通过 `crate::registry::WorkspaceRegistry::method(...)`。
3. **测试辅助可见性**：`digest.rs`、`vault/scanner.rs`、`test_utils.rs` 中的 `#[cfg(test)]` 代码调用 `WorkspaceRegistry::init_in_memory()`。将 `test_helpers.rs` 声明为 `#[cfg(test)] mod test_helpers;` 后，其 `impl WorkspaceRegistry` 仍属于 crate 的 test target，调用方无需修改。

---

## 4. 实施步骤建议

### 阶段一：`registry/core.rs` 拆分（低风险，先做）

1. 新建 `src/registry/migrate.rs`、`repo.rs`、`vault.rs`、`links.rs`、`test_helpers.rs`。
2. 按第 3.3 节的映射，将 `core.rs` 的代码块剪切到对应新文件。
3. 在每个新文件顶部添加 `use super::*;`。
4. 修改 `src/registry.rs`：删除 `mod core;`，添加新的 `mod` 声明（含 `#[cfg(test)] mod test_helpers;`）。
5. 删除 `src/registry/core.rs`。
6. 执行 `cargo check` → `cargo test`。
7. 提交 commit。

### 阶段二：`mcp/tools.rs` 拆分（次低风险，后做）

1. **备份** `src/mcp/tools.rs` 内容（或直接依赖 git）。
2. **删除** `src/mcp/tools.rs`。
3. 新建目录 `src/mcp/tools/`，创建 `mod.rs`。
4. 在 `mod.rs` 中写入 `pub use repo::*; pub use vault::*; pub use query::*; pub use context::*;`。
5. 创建 `repo.rs`、`vault.rs`、`query.rs`、`context.rs`，按第 2.4 节填充内容。
6. 每个子模块顶部保留必要的 `use`（如 `use crate::mcp::McpTool;`、`use anyhow::Context;` 等）。
7. 执行 `cargo check` → `cargo test`。
8. 提交 commit。

### 并行性
- 两次拆分互不依赖，可分两天完成。
- **不建议**在同一次 commit 中同时修改两个大文件，以便出问题时能快速回滚。

---

## 5. 风险评估

| 风险项 | 可能性 | 影响 | 缓解措施 |
|--------|--------|------|----------|
| **`tools.rs` 与 `tools/` 目录冲突**（Rust E0761） | 高（若操作顺序错误） | 编译阻塞 | **必须先删除 `tools.rs`，再创建 `tools/`**。建议在 git 中分两步：先 `git rm src/mcp/tools.rs`，再 `git add src/mcp/tools/`。 |
| 搬运后遗漏 `use super::*;` 或必要的 crate 导入 | 中 | 编译错误 | 每个新文件顶部显式添加 `use super::*;`，并对照原文件保留 `anyhow::Context`、`rusqlite::OptionalExtension` 等导入。 |
| `SCHEMA_DDL` / `init_in_memory` 在测试中不可见 | 低 | 测试编译失败 | 确保 `registry.rs` 中使用 `#[cfg(test)] mod test_helpers;`，且 `init_in_memory` 保持 `pub`。 |
| `McpToolEnum` 缺少某个 tool 变体 | 低 | 编译错误 | `McpToolEnum` 在 `mcp/mod.rs` 中定义，只要 `tools/mod.rs` re-export 了所有 struct，枚举无需改动。 |
| 合并冲突 | 中 | 需手动解决 | 拆分完成后，多人同时修改同一功能域的概率显著降低（例如改 vault 工具只需改 `vault.rs`）。 |
| `test_tools_list` 硬编码 19 个 tool 名称 | 低 | 测试语义不变 | `mcp/tests.rs` 第 29 行断言 `tools.len() == 19`，只要 `build_server()` 注册逻辑不变，该测试继续通过。 |

---

## 附录：关键数据速查

### `mcp/tools.rs`（1336 行）拆分后分布

| 文件 | 预估行数 | 包含 Tool / 函数 |
|------|----------|------------------|
| `tools/repo.rs` | ~950 | 13 个 repo tool + `parse_github_repo` / `nl_filter_repos` / `parse_stars_condition` / `extract_tag_from_query` |
| `tools/vault.rs` | ~210 | `vault_search`, `vault_read`, `vault_write`, `vault_backlinks` |
| `tools/query.rs` | ~40 | `query` |
| `tools/context.rs` | ~140 | `project_context` |

### `registry/core.rs`（1253 行）拆分后分布

| 文件 | 预估行数 | 包含内容 |
|------|----------|----------|
| `migrate.rs` | ~540 | `db_path`, `workspace_dir`, `init_db`（schema + 全量迁移） |
| `repo.rs` | ~470 | Repo CRUD + 原有 `mod tests` |
| `vault.rs` | ~80 | Vault note CRUD |
| `links.rs` | ~55 | `vault_repo_links` CRUD |
| `test_helpers.rs` | ~190 | `init_in_memory`, `SCHEMA_DDL`（`#[cfg(test)]`） |
