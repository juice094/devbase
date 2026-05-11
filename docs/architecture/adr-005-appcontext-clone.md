# ADR-005: AppContext Clone for Async Context Propagation

- **状态**: accepted
- **日期**: 2026-05-11
- **作者**: devbase 架构优化会话

## 上下文

MCP 工具频繁使用 `tokio::task::spawn_blocking` 执行 I/O 密集型操作（Tantivy 索引、SQLite 查询、文件系统遍历）。此前 `AppContext` 未实现 `Clone`，导致：
- 闭包内无法调用 `ctx.list_vault_notes()` 等 trait 方法
- 被迫在 `spawn_blocking` 外获取 `conn` 再 move 进闭包，增加生命周期复杂度
- `VaultClient`、`WorkflowClient` 等 trait 难以在异步上下文中使用

## 决策

将 `AppContext.env_cache` 从 `std::sync::Mutex<EnvVersionCache>` 改为 `Arc<std::sync::Mutex<EnvVersionCache>>`，并为 `AppContext` 添加 `#[derive(Clone)]`。

## 后果

- **正面**: `spawn_blocking` 闭包内可直接 `ctx.clone()` 后调用任意 trait 方法；统一 async/sync 边界处理模式
- **负面**: `Clone` 后多个上下文共享同一 `Mutex`，并发修改 env_cache 的竞争概率微增（当前仅 daemon 定期刷新，可忽略）
- **风险**: 未来若向 `AppContext` 添加非 Clone 字段，需回退到显式字段 clone 模式

## 备选方案

| 方案 | 不选原因 |
|------|---------|
| 为每个 trait 定义无状态 Impl (ZST) | `RegistryClient` 等方法需 `conn`，无状态 impl 需传入 `Connection`，违反 T11 红线 |
| 使用 `Arc<AppContext>` 包装 | 增加一层间接，所有调用点需改为 `arc.ctx.method()`，改动面过大 |
| 将 `Pool` 单独 clone move 进闭包 | 已在使用，但无法调用 `DigestClient` 等需要 config/i18n 的 trait 方法 |

## 相关决策

- 依赖：ADR-004（trait 化后，clone 成为 spawn_blocking 中使用 trait 的基础设施）
