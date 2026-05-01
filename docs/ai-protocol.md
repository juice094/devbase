# AI Protocol State — devbase

> 跨架构状态同步锚点。CLI/Web/Claw 会话启动时应优先读取此文件恢复上下文。

## 当前架构快照

- **版本**：v0.14.0
- **测试**：455 workspace passed / 0 failed / 5 ignored；分布：devbase 406 + symbol-links 4 + sync-protocol 12 + core-types 3 + syncthing-client 2 + vault-frontmatter 5 + vault-wikilink 5
- **编译**：0 errors，1 unused import warning（SortMode）
- **Registry God Object**：生产代码业务逻辑已全部消除，`WorkspaceRegistry` 为纯向后兼容门面
- **Workspace 拆分**：3 个零耦合模块已提取为独立 crate（`crates/` 目录）

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

## 已提取的 Workspace Crate

| Crate | 来源模块 | 行数 | 测试 | 内部耦合 | Commit |
|-------|---------|------|------|---------|--------|
| `devbase-symbol-links` | `src/symbol_links.rs` | 280 | 4 | 0 `crate::` refs | `7eb139d` |
| `devbase-sync-protocol` | `src/sync_protocol.rs` | 279 | 12 | 0 `crate::` refs | `7eb139d` |
| `devbase-core-types` | `src/core/node.rs` | 128 | 3 | 0 `crate::` refs | `7eb139d` |
| `devbase-syncthing-client` | `src/syncthing_client.rs` | 85 | 2 | 0 `crate::` refs | `066b18d` |
| `devbase-vault-frontmatter` | `src/vault/frontmatter.rs` | 175 | 5 | 0 `crate::` refs | `066b18d` |
| `devbase-vault-wikilink` | `src/vault/wikilink.rs` | 130 | 5 | 0 `crate::` refs | `066b18d` |

> 向后兼容：原 `src/` 路径改为 `pub use <crate>::*;` 重新导出，API 不变。

## 模块耦合健康度地图（按 `crate::` 引用数）

| 等级 | 标准 | 代表模块 | 行动 |
|------|------|---------|------|
| 🟢 健康 | 0-3 个 `crate::` refs | `syncthing_client`, `embedding`, `semantic_index`, `search`, `registry/health`, `registry/metrics`, `registry/workspace`, `registry/entity`, `registry/relation`, `vault/frontmatter`, `vault/wikilink`, `workflow/interpolate`, `workflow/model`, `skill_runtime/parser` | **下一轮拆分候选** |
| 🟡 亚健康 | 4-15 个 refs | `scan`, `sync`, `query`, `health`, `vault/indexer`, `workflow/state`, `skill_runtime/discover` | 需 trait 化解耦后拆分 |
| 🔴 不健康 | >15 个 refs | `mcp/tools/repo` (41), `knowledge_engine` (33), `skill_runtime/executor` (15), `workflow/executor` (10) | 需架构重构 |

## 待办（按优先级）

> 完整规划见 [`docs/ROADMAP.md`](ROADMAP.md)。此处仅保留架构层面的关键决策锚点。

1. **P0 — Workspace 扩展**：提取 🟢 健康模块为独立 crate。目标：workspace 成员达到 8-10 个。验收：`cargo check --workspace` 0 errors。
2. **P1 — MCP trait 化**：`mcp/tools/repo.rs` 有 41 个 `crate::` 引用（从 70 降下）。已定义 `ScanClient`/`HealthClient`/`SyncClient`/`KnowledgeClient`/`RegistryClient`/`DigestClient` trait，`AppContext` 统一实现。验收：<50 ✅，下一步 <30。
3. **P2 — registry 子模块清洁**：`health`, `metrics`, `workspace`, `entity`, `relation` 已零耦合，消除所有 `crate::` 引用使其达到"随时可提取"状态。
4. **P3 — migrate.rs 拆解**：按 schema 版本切分为 `migrate/v16.rs` 等。需 Claw 架构支持。

**技术债务**（清偿中）：
- Tantivy+SQLite 双写一致性（无事务协调）→ 评估补偿机制或 FTS5 替代
- tree-sitter 编译成本（~15-20s）→ ccache 或 grammar 预编译
- Feature flags 扩展（mcp 待评估）
- `SortMode` unused import（1 warning）

## 模式约束

- **提取模式**：`impl WorkspaceRegistry { pub fn method(...) }` → 模块级 `pub fn method(...)` + `impl WorkspaceRegistry { pub fn method(...) { method(...) } }` facade
- **调用路径**：内部调用统一使用 `crate::registry::<module>::<fn>`
- **模块可见性**：提取后的子模块必须在 `registry.rs` 中声明为 `pub mod <module>;`

## 最近一次 AI 会话

- **日期**：2026-05-01
- **架构**：CLI
- **交付**：Workspace 骨架搭建 + 3 个零耦合模块提取 + 全模块耦合地图扫描
- **Commit**：`7eb139d`（workspace 拆分）
- **关键决策**：采用"强叙事+弱绑定"分发策略；以 `crate::` 引用数作为耦合健康度金标准

- **日期**：2026-05-01（续）
- **架构**：CLI
- **交付**：Batch 3 — MCP trait 化完成。`clients.rs` 定义 6 个 client trait；`AppContext` 统一实现；`repo.rs` `crate::` 引用 68→41；测试 405 passed（1 flaky）
- **关键决策**：`HealthClient`/`SyncClient` 因 `rusqlite::Connection` 非 `Send`，去掉 future `+ Send` bound；`ScanClient` 保留 `+ Send`（`&Pool` 是 `Send`）

- **日期**：2026-05-01（续续）
- **架构**：CLI
- **交付**：CLI-MCP 断层补全（metrics/module-graph/call-graph/dependency-graph/code-symbols/dead-code/github-info）+ simple.rs 去耦合（63→54）+ repo.rs 查询提取（42→36）+ RegistryClient trait 扩展
- **Commit**：`92039d2`..`090b4b8`

- **日期**：2026-05-01（P0 性能修复）
- **架构**：CLI
- **交付**：`index_repo_full` 并行化（2.4x 加速，83.7s→35.0s）+ `list_repos` NULL 崩溃修复
- **Commit**：`c7dbacd`
- **关键决策**：`std::thread::scope` + 4MB 线程栈；`DEVBASE_INDEX_THREADS` 环境变量调优

- **当前计划**
- **文档**：`docs/plans/ai-native-storage-plan.md`
- **目标**：AI-Native Storage Engine Refactor（Phase 1-4）
- **验收**：Kimi CLI 作为 AI 用户的视角评价
