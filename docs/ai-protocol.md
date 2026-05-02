# AI Protocol State — devbase

> 跨架构状态同步锚点。CLI/Web/Claw 会话启动时应优先读取此文件恢复上下文。

## 当前架构快照

- **版本**：v0.15.0 (`main@e8860ba`)
- **测试**：418 passed / 0 failed / 5 ignored（`search::test_index_is_empty` Tantivy writer 显式 drop 加固；`embedding::test_candle_provider_encode` 取消 ignore）
- **编译**：0 errors，0 warnings
- **Registry God Object**：生产代码业务逻辑已全部消除，`WorkspaceRegistry` 为纯向后兼容门面
- **Workspace 拆分**：6 个零耦合模块已提取为独立 crate（`crates/` 目录）
- **千行文件治理**：6/6 完成，最大文件降至 950 行
- **Embedding 闭环**：Phase 3 完成，`local-embedding` 默认启用，candle `all-MiniLM-L6-v2` CPU 实时推理
- **索引黑名单**：`semantic_index` / `scan` / `TUI` 统一排除 `target/` / `.venv/` / `node_modules/` 等 9 个目录
- **增量索引**：Phase 4 完成，Git diff + 工作区变更检测，无变更 0.10s / 单文件变更 0.63s / 全量 ~15-25s（Sprint A rayon 并行 embedding）
- **Tantivy-SQLite 一致性**：Sprint B 完成，启动时一致性扫描 + `orphan_tantivy_docs` 懒清理
- **Agent 状态接口**：Sprint C 完成，`devbase status [--json]` + `DevkitStatusTool` + `DevkitIndexStreamTool` + `tools/call` streaming 支持
- **测试覆盖**：新增 `git_diff.rs` 8 个单元测试（commit/工作区/untracked/删除/空变更/空仓库）

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

- **日期**：2026-05-02
- **架构**：CLI
- **交付**：Sprint A — v28 migration 三维 embedding 主键 + rayon 并行化 + 性能基准达标
- **Commit**：`dfdc1cc`
- **关键决策**：
  - `code_embeddings` 主键从 `(repo_id, symbol_name)` 扩展为 `(repo_id, file_path, symbol_name)`，消除同名不同文件 symbol 的 embedding 共享问题
  - `rayon::par_iter` 并行生成 embeddings，全量索引从 130s 降至 ~15-25s（<60s 目标达成）
  - v28 迁移安全策略：检测旧表 → 创建新表 + `''` 填充迁移 + 原子 RENAME，零数据丢失
  - `generate_and_save_embeddings` 统一使用 `ON CONFLICT DO UPDATE`，消除同名 symbol UNIQUE constraint 失败

- **日期**：2026-05-02（续）
- **架构**：CLI
- **交付**：Sprint B — Tantivy-SQLite 双写一致性（启动扫描 + 懒清理）
- **Commit**：`dcbe256`
- **关键决策**：
  - `orphan_tantivy_docs` 表记录 Tantivy 有但 SQLite `entities` 无的孤儿文档
  - `AppContext::with_defaults()` 启动时自动扫描并修复一致性（`repair_tantivy_consistency`）
  - `run_index` 中对孤儿文档自动 `delete_repo_doc` + `commit_writer` 后清除 orphan 记录
  - 扫描双向清理：检测新孤儿 + 清除已恢复的过时孤儿标记

- **日期**：2026-05-02（续续）
- **架构**：CLI
- **交付**：Sprint C — Agent 状态接口 + MCP Streaming
- **Commit**：`e8860ba`
- **关键决策**：
  - `IndexState` 状态机（Fresh/Stale/Missing/Unknown），`detect_changes` 重构为 `get_repo_index_state`
  - `devbase status [--json]` CLI 命令，JSON 输出可被 Python `json.loads` 解析
  - `DevkitStatusTool` MCP 工具：100ms 内返回 repo 索引状态
  - `DevkitIndexStreamTool` + `handle_request` `stream: true` 支持：stdio 传输层流式事件
  - 45 个 MCP 工具（原 43 + 2 新增）

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

- **日期**：2026-04-26（千行文件治理 Batch 3/6 + Repository Phase 2）
- **架构**：CLI
- **交付**：`mcp/tools/repo.rs` 拆分 5 文件 + `registry/migrate.rs` 26 个独立迁移 + `commands/simple.rs` 拆分 5 文件 + `knowledge_engine.rs` / `semantic_index.rs` / `tui/state.rs` 拆分
- **Commit**：`091d444`

- **日期**：2026-04-26（千行文件治理收尾 + binary compat 修复）
- **架构**：CLI
- **交付**：`tui/state.rs` 1293→330+6 子模块 + `commands/simple.rs` re-export 修复 binary build
- **Commit**：`1a937c8`

- **日期**：2026-04-26（Phase 3 Embedding 闭环 + 索引黑名单）
- **架构**：CLI
- **交付**：
  - `Cargo.toml`：`local-embedding` 默认启用（candle CPU 推理）
  - `mcp/tools/search.rs`：`hybrid_search` / `semantic_search` 零参数自动生成 embedding
  - `knowledge_engine/index.rs`：`index` 流程自动生成 symbol embeddings（1201/1201 distinct names = 100%）
  - `semantic_index/mod.rs` / `scan.rs` / `config.rs`：9 个默认排除目录
  - `search_sync.rs`：TUI 搜索黑名单扩展
- **Commit**：`00c6765`
- **关键决策**：
  - embedding 按 `symbol_name` 去重存储，同名不同文件 symbol 共享 embedding
  - `generate_query_embedding` 失败时自动降级到纯 keyword search（AI 无感知）
  - MCP stdio 通过 Python subprocess 端到端验证通过：1.91s 返回 5 个融合结果

- **日期**：2026-04-26（Phase 4 增量索引）
- **架构**：CLI
- **交付**：
  - `semantic_index/git_diff.rs`：commit-to-commit + index-to-workdir 双模式变更检测
  - `registry/migrations/v27_repo_index_state.rs`：`repo_index_state` 表记录每个 repo 最后索引 hash
  - `semantic_index/persist.rs`：`delete_symbols_for_files` / `save_symbols_incremental` / `save_calls_incremental`
  - `knowledge_engine/index.rs`：`run_index` 集成 `detect_changes` → 增量分支 / 全量回退策略
  - `index.rs`：`save_symbol_embeddings_incremental` 增量更新 embedding（ON CONFLICT UPDATE）
- **Commit**：`cdceff3`
- **关键决策**：
  - 增量回退策略：非 Git / 首次索引 / 变更文件 >100 / diff 失败 → 全量索引
  - `diff_since` 同时检测已提交差异和未提交工作区修改（`Untracked` 视为 Added）
  - 无变更时 HEAD hash 不变，但工作区 clean → `Already up-to-date` 0.10s

- **当前计划**
- **文档**：`docs/plans/ai-native-storage-plan.md`
- **目标**：Saga 协调器（或 P0 Workspace crate 剥离）
- **验收**：Kimi CLI 作为 AI 用户的视角评价
