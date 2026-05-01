# 架构审计报告：千行文件治理与 Repository 层推进

> 日期：2026-04-26  
> 基线：`main@7f07d1e` (v0.14.0+)  
> 审计范围：`src/` 全部 Rust 源码

---

## 一、度量快照

```
文件                        行数    structs  impls   fns
src/mcp/tools/repo.rs       2249    25       25      61
src/registry/migrate.rs     1321    0        1       8
src/tui/state.rs            1293    0        1       6
src/semantic_index.rs       1184    5        2       51
src/commands/simple.rs      1093    1        0       19
src/knowledge_engine.rs     1049    1        0       36
```

**统计**：共 6 个千行级文件，合计 **8189 行**，占 `src/` 总行数约 **31%**。

---

## 二、逐文件诊断

### 2.1 `mcp/tools/repo.rs` (2249 行) — 最严重的上帝文件

**症状**：
- 单一文件承载 **24 个 MCP 工具**（`devkit_scan` → `devkit_cross_repo_search`）
- 每新增一个工具，编译单元重新编译 2249 行
- 文件内职责横跨：仓库发现、健康检查、知识管理、代码分析、向量搜索、外部 API 调用

**耦合度**：
- `crate::` 引用 23 处（已优化，原始 35）
- 但仍直接耦合 `registry::WorkspaceRegistry`（4 处搜索方法）
- 直接耦合 `health::`, `search::`, `arxiv::`, `oplog_analytics::`

**拆分方案**（按功能域分组）：

| 新文件 | 包含的工具 | 预估行数 |
|--------|-----------|---------|
| `repo_management.rs` | scan, health, sync, index, query_repos, nl_query | ~350 |
| `knowledge.rs` | note, digest, paper_index, experiment_log, knowledge_report | ~400 |
| `code_analysis.rs` | code_metrics, module_graph, code_symbols, dependency_graph, call_graph, dead_code | ~450 |
| `search.rs` | semantic_search, hybrid_search, embedding_store, embedding_search, related_symbols, cross_repo_search | ~400 |
| `external.rs` | github_info, arxiv_fetch | ~250 |
| `repo.rs`（保留） | 公共 trait 和辅助函数 | ~200 |

**收益**：
- 增量编译：修改单个工具只需重编 200-450 行
- 代码所有权：不同功能域可由不同开发者维护
- 测试隔离：每个文件可独立单元测试

---

### 2.2 `registry/migrate.rs` (1321 行) — 历史债务累积

**症状**：
- 包含 v1 → v26 共 26 版迁移逻辑
- 每版迁移函数内嵌大量 `ALTER TABLE`、`json_extract` 兼容代码
- 新增迁移（v27）需继续膨胀此文件

**拆分方案**：
```
registry/
  migrate.rs          → 仅保留迁移协调器 (run_migrations, CURRENT_SCHEMA_VERSION)
  migrations/
    mod.rs            → 注册表，枚举所有版本迁移
    v01_initial.rs
    v02_add_tags.rs
    ...
    v26_denormalize.rs
```

**收益**：
- 历史迁移只读化，新开发无需触碰旧代码
- 便于未来删除超旧迁移（如 v1-v10 可归档为 `migrations/_archive/`）

---

### 2.3 `semantic_index.rs` (1184 行) — 核心算法单块化

**症状**：
- 51 个函数挤在 2 个 impl 块中
- 混合职责：索引构建、向量量化、Tantivy 读写、并行调度、错误恢复
- 之前 P0 优化（`std::thread::scope`）进一步增加了此文件复杂度

**拆分方案**：
```
semantic_index/
  mod.rs              → 公共类型、入口函数 index_repo_full
  indexer.rs          → 文件遍历、符号提取、批量插入
  embedding.rs        → embedding_to_bytes, cosine_similarity, 向量运算
  tantivy_ops.rs      → Tantivy IndexWriter/IndexReader 封装
  parallel.rs         → thread scope、work stealing、栈大小配置
```

---

### 2.4 `commands/simple.rs` (1093 行) — CLI 分发器残余

**症状**：
- 19 个函数，主要是 `run_xxx()` 子命令入口
- 虽已提取 registry 查询到独立模块，但命令 dispatch 表仍在膨胀
- `crate::` 引用 54 处（原始 63）

**拆分方案**：
- 已完成大部分提取工作（`registry/{call_graph,code_symbols,dead_code}.rs`）
- **剩余**：将 CLI 子命令按功能域拆分为 `commands/{repo,search,code,knowledge,system}.rs`
- `simple.rs` 只保留 `main()` dispatch 表（目标 < 200 行）

---

### 2.5 `knowledge_engine.rs` (1049 行) — 职责边界模糊

**症状**：
- 36 个函数，仅 1 struct
- 可能混合：知识图谱构建、摘要生成、标签推荐、报告合成

**建议**：
- 先绘制函数调用图，识别自然聚类
- 再拆分为 `knowledge_engine/{graph,summary,report,tag}.rs`

---

### 2.6 `tui/state.rs` (1293 行) — 状态机臃肿

**症状**：
- 6 个函数，1293 行 → 平均每函数 215 行
- 大量 match 分支处理键位映射、弹窗状态、异步结果回调

**建议**：
- 按 TUI 视图拆分：`tui/state/{list,detail,popup,async}.rs`
- 或使用状态模式（State Pattern）替代巨型 match

---

## 三、优先级矩阵

| 优先级 | 文件 | 拆分难度 | 编译收益 | 维护收益 | 推荐动作 |
|--------|------|---------|---------|---------|---------|
| P0 | `mcp/tools/repo.rs` | 中 | 高 | 极高 | **立即执行** — 按功能域拆 5 文件 |
| P1 | `registry/migrate.rs` | 低 | 低 | 高 | v27 之前完成迁移 |
| P1 | `semantic_index.rs` | 高 | 中 | 高 | 与 Phase 3 (Embedding 闭环) 同步 |
| P2 | `commands/simple.rs` | 低 | 中 | 中 | 已部分完成，收尾即可 |
| P2 | `knowledge_engine.rs` | 中 | 低 | 中 | 延后到 Phase 4 |
| P3 | `tui/state.rs` | 中 | 低 | 中 | 延后到 TUI 重构专项 |

---

## 四、Repository 层推进状态

当前已完成的 Repository（7 个）：

```
repository/
  mod.rs           → Repository trait
  repo.rs          → RepoRepository        (从 registry/repo.rs 提取)
  knowledge.rs     → KnowledgeRepository   (从 registry/knowledge.rs 提取)
  symbol.rs        → SymbolRepository      (从 registry/{code_symbols,call_graph,dead_code}.rs 提取)
  search.rs        → SearchRepository      (骨架，TODO 4 方法)
  dependency.rs    → DependencyRepository  (从 dependency_graph.rs 提取)
  health.rs        → HealthRepository      (从 registry/health.rs 提取)
  workspace.rs     → WorkspaceRepository   (从 registry/workspace.rs 提取)
```

**剩余阻塞**：
- `SearchRepository` 需填充 4 个方法（semantic/hybrid/related/cross_repo）
- 完成后 `mcp/tools/repo.rs` 中 `crate::registry::WorkspaceRegistry` 引用可清零

---

## 五、下一步建议

### 方案 A：继续 Phase 2 收尾（推荐，低风险）
1. 填充 `SearchRepository` 4 个搜索方法
2. `repo.rs` `crate::` 引用降至 < 20
3. 提交 v0.14.1

### 方案 B：千行文件治理（P0：`mcp/tools/repo.rs` 拆分）
1. 按功能域拆分为 5 个文件
2. 同步更新 `mcp/mod.rs` 中的工具注册表
3. 预期工作量：1-2 小时，零行为变更

### 方案 C：并行推进（推荐，最大化收益）
- **子代理 A**：`mcp/tools/repo.rs` 拆分（方案 B）
- **子代理 B**：`SearchRepository` 填充（方案 A）
- **子代理 C**：`registry/migrate.rs` 迁移提取（P1）

---

*报告生成完毕。等待人类裁决。*
