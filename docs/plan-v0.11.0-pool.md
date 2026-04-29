# v0.11.0 Phase 0.5: Pool 接入与 init_db() 根治

## 目标
- 将 `AppContext` 中的裸 `Connection` 替换为 `r2d2_sqlite::Pool`
- 消灭所有非构造函数/定义处的 `init_db()` 调用
- 解决 `spawn_blocking` / `thread::spawn` 闭包中的 `Connection` `Send` 问题

## 方案选择（最小依赖原则）

| 方案 | 依赖复杂度 | 总改动量 | 重复劳动 |
|------|-----------|---------|---------|
| A: 逐个清理叶子模块 | 高（20+模块分散依赖） | ~50处 | 高（Pool接入后需二次修改签名） |
| B: 中心节点 Pool 化 | 低（`AppContext` 单一节点） | ~80处 | 无 |

**裁决：方案 B**

理由：`AppContext` 是连接层唯一中心节点。修改 `AppContext` 后，所有模块的 `init_db()` 替换模式一致（`ctx.conn()?`），无需逐个修改函数签名，避免二次返工。

## 实施步骤

### Step 1: `AppContext` Pool 化
- `storage.rs`: `conn` → `pool`
- `conn()` / `conn_mut()` 返回 `Result<PooledConnection<SqliteConnectionManager>>`
- `with_defaults()` / `with_storage()` 创建 `Pool`
- 影响：约 22 处现有 `ctx.conn()` / `ctx.conn_mut()` 需加 `?`

### Step 2: 批量适配现有调用点
- `commands/*.rs`: `ctx.conn()` → `ctx.conn()?`, `ctx.conn_mut()` → `let mut conn = ctx.conn()?;`
- `mcp/tools/skill.rs` / `known_limit.rs`: 同上
- `tui/state.rs`: 同上

### Step 3: 深层模块 `init_db()` 替换
- `scan.rs`, `health.rs`, `sync.rs`, `backup.rs`, `query.rs`, `knowledge_engine.rs`
- `skill_runtime/*`, `workflow/*`, `vault/*`, `daemon.rs`
- 同步上下文中的直接替换为 `ctx.conn()?`
- `spawn_blocking` 闭包中的也替换为 `ctx.conn()?`（Pool 是 `Send`）

### Step 4: 编译修复与测试
- `cargo check` → `cargo test`
- 提交
