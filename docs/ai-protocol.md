# AI Protocol State — devbase

> 跨架构状态同步锚点。CLI/Web/Claw 会话启动时应优先读取此文件恢复上下文。

## 当前架构快照

- **版本**：v0.14.0
- **测试**：397 lib passed / 0 failed / 5 ignored；11 CLI passed
- **编译**：0 warnings
- **Registry God Object**：生产代码业务逻辑已全部消除，`WorkspaceRegistry` 为纯向后兼容门面

## 已完成的子模块提取（v0.15 重构）

| 模块 | 方法数 | Commit |
|------|--------|--------|
| `registry/entity.rs` | 3 | 前置批次 |
| `registry/relation.rs` | 3 | 前置批次 |
| `registry/repo.rs` | 9 | Batch 1 |
| `registry/vault.rs` | 4 | Batch 1 |
| `registry/workspace.rs` | 5 | Batch 1 |
| `registry/health.rs` | 5 | Batch 2 |
| `registry/metrics.rs` | 3 | Batch 2 |
| `registry/links.rs` | 4 | Batch 3 |
| `registry/known_limits.rs` | 6 | Batch 3 |
| `registry/knowledge_meta.rs` | 4 | Batch 3 |
| `registry/knowledge.rs` | 20 | Batch 3 |

## 剩余 God Object 表面

| 文件 | 行数 | 状态 | 阻塞原因 |
|------|------|------|---------|
| `registry/migrate.rs` | 1273 | ⏳ 待拆分 | 巨石文件，含 schema 迁移 + DDL + 数据转换逻辑；需 Claw 架构支持多轮持久化拆分 |
| `registry/test_helpers.rs` | 394 | ✅ 保留 | 纯测试基础设施，`init_in_memory()` / `seed_test_repo()` 等辅助方法 |

## 待办（按优先级）

1. **P1 — migrate.rs 拆解**：待 Claw 架构就绪后推进。计划按 schema 版本或职责切分为子模块（如 `migrate/v16.rs`、`migrate/v17.rs` 等），每次改动需保证 `test_in_memory_schema_version` 通过。
2. **P2 — 测试覆盖率**：20 个零测试文件（`commands/simple.rs` 647L、`sync/tasks.rs` 622L 等）待补充 smoke tests。
3. **P3 — 12 个巨石文件 >500L**：持续瘦身。

## 模式约束

- **提取模式**：`impl WorkspaceRegistry { pub fn method(...) }` → 模块级 `pub fn method(...)` + `impl WorkspaceRegistry { pub fn method(...) { method(...) } }` facade
- **调用路径**：内部调用统一使用 `crate::registry::<module>::<fn>`
- **模块可见性**：提取后的子模块必须在 `registry.rs` 中声明为 `pub mod <module>;`

## 最近一次 AI 会话

- **日期**：2026-04-26
- **架构**：CLI
- **交付**：Batch 1-3 完成 + 文档同步
- **Commit**：`dfc43d4`（重构）、`2def21c`（文档）
