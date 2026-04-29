# devbase 项目现状与工程模糊点 · 2026-04-29

> 本文件由 AI Agent 基于代码库实际状态生成，供与分析师/架构师讨论使用。
> 覆盖范围：devbase v0.12.0-alpha (commit `f6dd5f0`)

---

## 一、当前交付状态

### 1.1 已完成的重大重构（Phase 2 全量交付）

| Stage | 内容 | 状态 |
|-------|------|------|
| A | `ENTITY_TYPE_*` 常量提取（10 文件 ~25 处替换） | ✅ |
| B | 通用 `upsert_entity` 抽象 + `upsert_entity_for_repo` / `sync_skill_to_entities` | ✅ |
| C | vault/paper/workflow dual-write 到 `entities` 表 | ✅ |
| C+ | `scan.exclude_paths` 配置 + sync 阶段二次过滤 | ✅ |
| D | `list_workflows` / `list_papers` / `list_vault_notes` 迁移到 `entities` + `json_extract` | ✅ |
| E | 删除 `vault_notes` / `papers` / `workflows` 孤儿表；Schema v22 迁移 | ✅ |
| — | `.devbase-ignore` 目录级排除 | ✅ |
| — | **Managed-Gate Fail-Safe Defaults**（sync 默认仅操作管理标签仓库） | ✅ |

### 1.2 数据层现状

- **Schema v22**：`entities` 表为唯一数据源，`skills` 保留独立表（仅因 `embedding` BLOB 字段）
- **已删除表**：`repos` (v0.11.3)、`vault_notes`/`papers`/`workflows` (v0.12.0-alpha Stage E)
- **JOIN 表**：`repo_tags`、`repo_remotes`、`repo_health`、`code_metrics`、`repo_summaries`、`code_symbol_links`、`workspace_snapshots`、`oplog`、`skills`、`skill_executions`、`known_limits`、`knowledge_meta`...

### 1.3 测试状态

- **Unit tests**：379 passed / 0 failed / 4 ignored（单线程 `--test-threads=1`）
- **Flaky tests**：2 个（`search::tests::test_search_repos`、`search::tests::test_search_vault`）在多线程并发下偶发失败，单独运行通过。根因未定位。
- **Integration tests**：`tests/cli.rs` 9 passed
- **CI**：Windows-only GitHub Actions，`--test-threads=4`

### 1.4 版本与分支

- **当前版本**：v0.12.0-alpha（未打 tag）
- **main 分支**：领先 origin/main 10 commits（`5468a35..f6dd5f0`）
- **最近 tag**：v0.11.3

---

## 二、工程模糊点（Engineering Ambiguities）

### 2.1 架构决策层

#### A1. SQLite FTS5 vs Tantivy 的取舍（未决策）

**现状**：当前使用 Tantivy 做全文检索 + SQLite 做元数据存储。双写无事务协调。

**模糊点**：
- Tantivy 带来 ~15-20s 编译成本（tree-sitter grammar 预编译问题）
- 双写一致性无补偿机制；索引与 DB 可能漂移
- SQLite FTS5 是否能完全替代 Tantivy？功能差距（BM25、多字段权重、前缀搜索）未评估
- 如果保留 Tantivy，是否需要设计 `sync_index_to_db()` 回滚/两阶段提交？

**需决策**：评估 FTS5 能力边界 → 决定替代或增强一致性机制。

#### A2. Feature Flags 不完整（🟡 中风险）

**现状**：`tui` 和 `watch` 为 optional feature，`mcp` 仍为默认依赖。

**模糊点**：
- `--no-default-features` 编译通过，但核心功能（sync/scan/registry）仍依赖全部默认 crate
- `mcp` 模块（37 tools）是否应拆分为独立 feature？这会显著减少无 AI 集成场景的二进制体积
- `search` 模块（Tantivy 依赖）是否应 feature-gate？

**需决策**：feature 拆分策略与最小可运行二进制定义。

#### A3. `skills` 表的独立存在理由（🟡 中风险）

**现状**：`skills` 是唯一未迁移到 `entities` 的表，原因是 `embedding` BLOB 字段。

**模糊点**：
- `entities` 表的 `metadata` JSON 列能否容纳 BLOB（通过 base64/hex 编码）？
- 如果 `skills` 也迁移到 `entities`，整个数据模型将完全统一，但可能牺牲查询性能
- `embedding` 的 768-dim f32 数组 (~3KB/skill) 在 JSON 中的序列化/反序列化开销是否可接受？

**需决策**：`skills` 最终归宿 — 保持独立表 vs 统一 entities。

---

### 2.2 安全与可靠性

#### B1. Sync Fail-Safe Defaults 的用户体验代价（🟡 中风险）

**现状**：Managed-gate 已落地。`devbase sync` 默认仅操作带管理标签的仓库。

**模糊点**：
- 现有用户数据库中大量 `"discovered"` 标签的仓库将**静默停止同步**，用户可能误以为 sync 坏了
- 提示消息 `hint_unmanaged_repos` 仅在 `tasks.is_empty()` 时打印；如果部分仓库有管理标签、部分没有，未管理仓库**完全静默跳过**，用户无从知晓
- 没有批量 `tag` 命令；用户需要逐个 `devbase tag <repo> managed`，操作成本高
- `--filter-tags` 可以绕过 gate，但这要求用户知道哪些旧标签存在

**需决策**：
- 是否添加 `--list-unmanaged` 子命令或 `health` 中标记未管理仓库？
- 是否支持批量 `devbase tag --all managed`？
- 是否在首次 sync 遇到未管理仓库时打印每个被跳过仓库的列表？

#### B2. Flaky 测试根因（🟡 中风险）

**现状**：`test_search_repos` 和 `test_search_vault` 在多线程下偶发失败。

**模糊点**：
- 失败是否与 Tantivy 索引的并发写入/读取有关？
- 是否与 `i18n::init()` 全局状态竞争有关？（`i18n` 使用 `OnceLock`，但测试间可能串扰）
- 是否与临时目录清理竞争有关？
- 单线程通过说明是竞态条件，而非逻辑错误

**需决策**：是否投入时间定位根因，还是暂时用 `--test-threads=1` 规避？

---

### 2.3 代码质量与可维护性

#### C1. `main.rs` 上帝文件残余（🟢 低风险）

**现状**：`main.rs` 515 行，已拆分为 `commands/simple.rs` + `commands/skill.rs` + `commands/workflow.rs` + `commands/limit.rs`。

**模糊点**：
- CLI 命令枚举 `Commands`（22 个变体）仍在 `main.rs` 中定义，导致 `main.rs` 仍需了解所有子命令的参数结构
- 是否应将 `Commands` 枚举也下沉到 `commands/` 模块？
- `main.rs` 的 `AppContext` 初始化和错误处理逻辑 ~80 行，是否应提取为 `app.rs`？

**需决策**：`main.rs` 的"足够干净"阈值是多少？当前 515 行是否已可接受？

#### C2. `init_db()` 全局路径（🟡 中风险）

**现状**：`AppContext` 已集成到全部 22 个 commands 模块，`db_path`/`workspace_dir`/`index_path`/`backup_dir` 已统一。

**模糊点**：
- `backup_dir`、`db_path`、`index_path` 仍有 3 处全局路径硬编码（grandfathered）
- `StorageBackend` trait 已奠基但尚未完全替代直接路径访问
- 新增功能是否可能无意中引入第 4 处全局路径？

**需决策**：`StorageBackend` trait 的完全迁移优先级。这是否阻碍 v0.12.0 发布？

---

### 2.4 产品化与发布

#### D1. v0.12.0 发布标准（🔴 高风险）

**现状**：v0.12.0-alpha 已累积 10 个 commits，包含 breaking change（managed-gate）。

**模糊点**：
- 是否需要 CHANGELOG.md？当前只有 commit message 历史
- 是否需要 integration test 加固（`tests/cli.rs` 仅 9 个测试）？
- managed-gate 的 breaking change 是否需要迁移脚本或发布公告？
- 版本 tag 策略：alpha → rc → release？还是直接 v0.12.0？

**需决策**：发布 checklist 与 minimum viable release criteria。

#### D2. 跨项目依赖状态（🟢 低风险）

**现状**：AGENTS.md 记录依赖图为 `clarity ← devbase ← syncthing-rust`。但 `clarity` 和 `devbase` 之间的路径依赖已解除。

**模糊点**：
- `devbase` 的 `syncthing_push` 命令是否仍在使用？是否应标记为 deprecated？
- `devbase` 的 `.syncdone` 标记格式与 `syncthing-rust` 的对齐状态是否仍需要维护？

---

## 三、需要人类裁决的决策清单

| # | 决策 | 影响 | 建议讨论角色 |
|---|------|------|-------------|
| 1 | Tantivy 去留 | 编译时间、一致性、功能边界 | 架构师 |
| 2 | v0.12.0 发布标准 | 用户感知、breaking change 沟通 | PM / 分析师 |
| 3 | Sync 未管理仓库的可见性 | 用户体验、安全与便利平衡 | PM / 分析师 |
| 4 | `skills` 表统一迁移 | 数据模型纯度 vs 性能 | 架构师 |
| 5 | Flaky test 修复投入 | 开发时间 vs CI 稳定性 | 工程负责人 |
| 6 | Feature flags 完整拆分 | 二进制体积、模块化 | 架构师 |

---

## 四、客观度量数据

```
代码规模：
  $ tokei src/
  Rust: ~18,000 LOC (含注释)
  
测试覆盖：
  $ cargo test --lib -- --test-threads=1
  test result: ok. 379 passed; 0 failed; 4 ignored

编译：
  $ cargo build --release
  ~45s (clean build, Windows, Rust 1.94.1)

依赖：
  $ cargo tree | wc -l
  ~450 个 crate (含 dev-dependencies)
  
已知漏洞：
  $ cargo audit
  1 informational: RUSTSEC-2020-0163 (tokei upstream, not exploitable in devbase usage)
```

---

*生成时间：2026-04-29*
*基于 commit：f6dd5f0*
