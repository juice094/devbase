# AI Protocol State — devbase

> 跨架构状态同步锚点。CLI/Web/Claw 会话启动时应优先读取此文件恢复上下文。

## 当前架构快照

- **版本**：v0.14.0
- **测试**：437 lib passed / 0 failed / 5 ignored；7 bin + 11 integration passed
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
2. **P2 — 测试覆盖率**：已补充 ~47 个 smoke tests 覆盖全部零测试文件。所有含逻辑的文件均已具备测试模块。
3. **P3 — 12 个巨石文件 >500L**：持续瘦身。

## 模式约束

- **提取模式**：`impl WorkspaceRegistry { pub fn method(...) }` → 模块级 `pub fn method(...)` + `impl WorkspaceRegistry { pub fn method(...) { method(...) } }` facade
- **调用路径**：内部调用统一使用 `crate::registry::<module>::<fn>`
- **模块可见性**：提取后的子模块必须在 `registry.rs` 中声明为 `pub mod <module>;`

## 最近一次 AI 会话

- **日期**：2026-04-26 / 2026-04-27
- **架构**：CLI
- **交付**：Batch 1-3 完成 + MCP NDJSON 修复 + 33 个新增 smoke tests
- **Commit**：`dfc43d4`（重构）、`2def21c`（文档）、`095f074`（MCP 修复）、`08b0f1b`（测试 batch 1）、`9e660a8`（测试 batch 2）、`07e21ca`（测试 batch 3）
