# devbase AppContext 职责拆分设计文档

> **版本**: v0.15.0-P2  
> **日期**: 2026-05-08  
> **范围**: `src/storage.rs`、`src/clients.rs` 及 6 个 Client trait 实现  
> **目标**: 消除 `AppContext` 违反 SRP 的集中式 trait 实现，降低 `storage.rs` 耦合度，提升单元测试可行性。

---

## 一、现状诊断

### 1.1 规模与耦合

`src/storage.rs` 当前 **860 行**，其中约 **432 行**（50%）为 6 个 Client trait 的 `impl` 块：

| Trait | 方法数 | 所在行号 | 依赖的核心模块 |
|-------|--------|----------|----------------|
| `ScanClient` | 1 | 309–317 | `scan::run_json` |
| `HealthClient` | 1 | 319–341 | `health::run_json`、`health::refresh_env_cache` |
| `SyncClient` | 1 | 343–353 | `sync::run_json` |
| `DigestClient` | 1 | 355–361 | `digest::generate_daily_digest` |
| `KnowledgeClient` | 4 | 363–408 | `knowledge_engine::run_index`、`registry::knowledge::*` |
| `RegistryClient` | 11 | 410–741 | `registry::repo`、`registry::knowledge`、`registry::metrics`、`registry::health`、`dependency_graph`、`registry::call_graph` |

这些 `impl` 块与 `AppContext` 本体（字段定义、构造函数、连接池管理、`EnvVersionCache` 管理）共存于同一文件，导致：

1. **单一文件职责过载**：`storage.rs` 同时承担"存储后端抽象"、"应用上下文生命周期管理"、"6 个业务领域 Client 实现"。
2. **变更放大效应**：修改 `RegistryClient` 的查询逻辑（如 `query_code_symbols`）会触发 `storage.rs` 的重新编译，间接影响所有依赖 `StorageBackend` 的模块。
3. **单元测试阻碍**：`AppContext` 的 `pool` 和 `env_cache` 为私有字段，测试只能通过 `with_storage()` 构造完整上下文；而业务逻辑与上下文构造代码混在一起，导致 mock 成本过高。
4. **内联 SQL 泄漏**：`RegistryClient` 的 `query_code_symbols` 和 `query_dead_code` 直接在 `storage.rs` 中拼接 SQL，绕过了 `registry` 子模块的封装边界。

### 1.2 当前 AppContext 方法清单（按职责分组）

#### A. 基础设施生命周期（应保留在 `storage.rs`）
- `with_defaults() -> Result<Self>`
- `with_storage(storage) -> Result<Self>`
- `build_pool(path) -> Result<Pool<...>>` (private)
- `conn() -> Result<PooledConnection<...>>`
- `conn_mut() -> Result<PooledConnection<...>>` （与 `conn()` 完全等价，API 冗余）
- `pool() -> Pool<...>`
- `env_cache() -> Result<EnvVersionCache>`
- `set_env_cache(cache) -> Result<()>`

#### B. 扫描职责 (`ScanClient`)
- `scan_directory(path, register) -> Future<Result<Value>>`

#### C. 健康职责 (`HealthClient`)
- `check_health(detail) -> Future<Result<Value>>`（含 `env_cache` 刷新逻辑）

#### D. 同步职责 (`SyncClient`)
- `sync_repos(dry_run, filter_tags) -> Future<Result<Value>>`

#### E. 摘要职责 (`DigestClient`)
- `generate_daily_digest() -> Result<Value>`

#### F. 知识引擎职责 (`KnowledgeClient`)
- `run_index(path) -> Result<Value>`
- `save_note(repo_id, text, author) -> Result<Value>`
- `save_summary(repo_id, desc, author) -> Result<Value>`
- `get_paper(arxiv_id) -> Result<Value>`

#### G. 注册表职责 (`RegistryClient`)
- `list_repos(filter) -> Result<Value>`
- `get_repo(repo_id) -> Result<Value>`
- `list_modules(repo_id) -> Result<Value>`
- `save_paper(paper) -> Result<Value>`
- `save_experiment(exp) -> Result<Value>`
- `list_code_metrics() -> Result<Value>`
- `get_code_metrics(repo_id) -> Result<Value>`
- `get_health(repo_id) -> Result<Value>`
- `query_call_graph(repo_id, callee, caller, file, limit) -> Result<Value>`
- `query_dependencies(repo_id, direction, relation_type) -> Result<Value>`
- `query_code_symbols(repo_id, name, symbol_type, file, limit) -> Result<Value>`
- `query_dead_code(repo_id, include_pub, limit) -> Result<Value>`

---

## 二、拆分方案

### 2.1 核心原则

1. **AppContext 保持为依赖容器**：不拆分字段，不引入 6 个新的 Service 结构体以避免调用方大面积重构。
2. **`impl` 块按职责归属迁移**：将 `impl XxxClient for AppContext` 从 `storage.rs` 剪切到对应的业务模块文件中。Rust 允许在任意模块中为外部类型实现外部 trait（orphan rules 在此不适用，因为 `AppContext` 和 trait 均定义在当前 crate）。
3. **trait 定义不动**：`src/clients.rs` 保持为 MCP tool 与业务模块之间的稳定契约。
4. **分阶段实施**：Phase 1 为纯物理迁移（零行为变更），Phase 2 为逻辑下沉（提取内联 SQL 到 `registry` 子模块）。

### 2.2 Phase 1：文件级 `impl` 块迁移（零行为变更）

| Trait | 原位置 | 目标位置 | 理由 |
|-------|--------|----------|------|
| `ScanClient` | `storage.rs:309` | `scan.rs` | 直接代理 `scan::run_json`，归属扫描模块。 |
| `HealthClient` | `storage.rs:319` | `health.rs` | 直接代理 `health::run_json` 并管理 `env_cache`，归属健康模块。 |
| `SyncClient` | `storage.rs:343` | `sync.rs` | 直接代理 `sync::run_json`，归属同步模块。 |
| `DigestClient` | `storage.rs:355` | `digest.rs` | 直接代理 `digest::generate_daily_digest`，归属摘要模块。 |
| `KnowledgeClient` | `storage.rs:363` | `knowledge_engine/mod.rs` | 混合调用 `knowledge_engine::run_index` 与 `registry::knowledge`，归属知识引擎模块。 |
| `RegistryClient` | `storage.rs:410` | `registry.rs` | 大量调用 `registry::*` 子模块，归属注册表模块。 |

**迁移后 `storage.rs` 的预期规模**：

- `EnvVersionCache` + `StorageBackend` / `DefaultStorageBackend` / `TempStorageBackend`：~105 行
- `AppContext` 本体（字段 + 构造函数 + `conn/pool/env_cache` 方法）：~105 行
- `repair_tantivy_consistency` / `repair_tantivy_consistency_at`：~90 行
- `#[cfg(test)]` 测试：~75 行
- **总计约 375 行**，较当前 **860 行** 减少 **56%**。

### 2.3 Phase 2：逻辑下沉与封装修复

Phase 1 迁移后，`RegistryClient` 的实现（~330 行）中会暴露两个问题：

1. `query_code_symbols` 和 `query_dead_code` 直接在 `impl` 块中拼接 SQL，未复用 `registry::code_symbols` / `registry::dead_code` 子模块。
2. `RegistryClient` 成为 `registry.rs` 中最臃肿的部分，而该文件原本以数据类型定义为主。

**建议的 Phase 2 动作**：

| 方法 | 当前实现方式 | Phase 2 下沉目标 |
|------|--------------|------------------|
| `query_code_symbols` | 内联 SQL (`SELECT ... FROM code_symbols`) | 新增 `registry::code_symbols::query_code_symbols(conn, ...)` 函数 |
| `query_dead_code` | 内联 SQL (`SELECT ... FROM code_symbols cs ...`) | 新增 `registry::dead_code::query_dead_code(conn, ...)` 函数 |
| `query_call_graph` | 调用 `registry::call_graph::query_call_edges` | ✅ 已下沉，无需改动 |
| `query_dependencies` | 调用 `dependency_graph::*` | ✅ 已下沉，无需改动 |

下沉后，`impl RegistryClient for AppContext` 将退化为**纯代理层**（类似 `ScanClient`），每个方法仅做：
1. `let conn = self.conn()?;`
2. 调用对应子模块函数；
3. 包装 `serde_json::Value` 返回。

这进一步使 `RegistryClient` 实现具备可测试性：业务逻辑可在 `registry` 子模块中直接以 `rusqlite::Connection` 为参数进行单元测试，无需构造完整 `AppContext`。

### 2.4 模块结构图（迁移后）

```
src/
├── clients.rs              # trait 定义（不变）
├── storage.rs              # AppContext 本体 + StorageBackend（~375 行）
├── scan.rs                 # + impl ScanClient for AppContext
├── health.rs               # + impl HealthClient for AppContext
├── sync.rs                 # + impl SyncClient for AppContext
├── digest.rs               # + impl DigestClient for AppContext
├── knowledge_engine/
│   └── mod.rs              # + impl KnowledgeClient for AppContext
├── registry.rs             # + impl RegistryClient for AppContext（Phase 2 后退化）
│   ├── code_symbols.rs     # Phase 2: 新增 query_code_symbols(conn, ...)
│   ├── dead_code.rs        # Phase 2: 新增 query_dead_code(conn, ...)
│   └── ...
```

---

## 三、向后兼容策略

### 3.1 API 兼容性

- **`clients.rs` trait 签名**：零变更。所有 MCP tool（`mcp/tools/repo.rs`、`knowledge.rs`、`external.rs`、`code_analysis.rs` 等）和 CLI command（`commands/analysis.rs` 等）的调用代码**无需任何修改**。
- **`AppContext` 公共字段与方法**：`storage`、`config`、`i18n`、`conn()`、`pool()`、`env_cache()`、`set_env_cache()` 保持公开且语义不变。
- **`AppContext::with_defaults` / `with_storage`**：构造函数逻辑不变，启动一致性检查（`repair_tantivy_consistency`）保留。

### 3.2 唯一破坏性变更（建议纳入 v0.15.0）

- **`AppContext::conn_mut()`**：与 `conn()` 完全等价（`&mut self` 并未改变 `pool.get()` 的不可变借用语义），存在误导性。建议 **移除** 或标记为 `#[deprecated]`。
  - 影响面：全局搜索显示仅 `commands/` 和 `mcp/tools/` 中极少数代码可能使用。经 `grep` 验证，当前调用方均使用 `ctx.conn()` 或直接 `ctx.pool()`，因此实际影响接近零。

### 3.3 编译隔离

- 迁移后，`scan.rs` 修改不再导致依赖 `storage.rs` 的模块重新编译（因为 `ScanClient` 实现已离开 `storage.rs`）。
- `registry.rs` 中的 `impl RegistryClient` 修改不再影响 `StorageBackend` 的编译单元。

---

## 四、估计工作量和风险

### 4.1 涉及文件数量

| 阶段 | 变更文件数 | 新增文件数 | 说明 |
|------|------------|------------|------|
| Phase 1 | 7 | 0 | `storage.rs` + 6 个目标模块文件剪切粘贴 |
| Phase 2 | 3 | 0 | `registry.rs`、`registry/code_symbols.rs`、`registry/dead_code.rs` 提取函数 |

### 4.2 工作量估计

- **Phase 1**：**0.5–1 人天**。纯代码迁移，无逻辑变更。主要工作为调整各目标文件顶部的 `use` 语句（引入 `AppContext`、`clients::XxxClient`、`serde_json` 等）。
- **Phase 2**：**1–1.5 人天**。需将 SQL 逻辑封装为带参数的纯函数，并补充单元测试（使用 `WorkspaceRegistry::init_in_memory()` 构造内存数据库）。
- **回归测试**：`cargo test` 全量通过 + `cargo clippy` 零警告。由于零行为变更，现有测试即回归测试。

### 4.3 风险矩阵

| 风险项 | 概率 | 影响 | 缓解措施 |
|--------|------|------|----------|
| `use` 语句循环依赖 | 低 | 中 | 迁移前检查各模块已有的 `use crate::storage` 引用；`AppContext` 本体不依赖任何 Client trait，天然避免循环。 |
| `RegistryClient` 内联 SQL 提取时引入行为偏差 | 中 | 低 | 提取前后对比 SQL 字符串完全一致；对 `query_code_symbols` / `query_dead_code` 补充 in-memory SQLite 单元测试。 |
| 私有字段访问权限 | 低 | 低 | `impl` 块中的代码仅使用 `self.conn()`、`self.pool()`、`self.config`、`self.i18n`、`self.env_cache()`，均为公共 API，跨模块访问无障碍。 |
| 编译时间回退 | 极低 | 低 | 实际上编译时间应略微下降（`storage.rs` 编译单元变小，增量编译更细）。 |

---

## 五、Hard Veto 兼容性检查

| Veto 项 | 检查结果 | 说明 |
|---------|----------|------|
| 禁止闭源 / 云端强制 / 数据外泄 | ✅ 兼容 | 纯内部文件移动，无外部依赖引入。 |
| 禁止 Docker / RAG(Qdrant) / GUI(Electron) | ✅ 兼容 | 不涉及。 |
| 禁止项目广度 > 5 核心工具 | ✅ 兼容 | 不新增核心工具或 crate。 |
| 本地 LLM 优先 | ✅ 兼容 | 不涉及模型变更。 |
| Rust 核心模块不可外包给子 Agent | ⚠️ 注意 | 本方案为**设计文档与纯代码迁移**，实际执行建议由人类或 `coder` 子代理在本地完成；若使用子代理，应在人类复核后执行 `git diff`。 |
| 禁止永久删除文件 | ✅ 兼容 | 仅剪切代码，原 `storage.rs` 保留大量代码，不产生废弃文件。 |

---

## 六、执行建议

1. **立即执行 Phase 1**：无风险、高回报，直接解决 `storage.rs` 臃肿问题。
2. **Phase 2 与 v0.15.0 其他 P2 工作并行**：在需要修改 `query_code_symbols` 或 `query_dead_code` 逻辑时顺手提取，不单独立项。
3. **删除 `conn_mut()`**：在 Phase 1 中一并删除，因为 v0.15.0 已允许 minor breaking changes（且实际调用方为零）。
