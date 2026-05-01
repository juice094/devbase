# Plan: AI-Native Storage Engine Refactor

> **协议版本**: V3.1-EP-O  
> **生效范围**: devbase v0.14.0+  
> **验收视角**: Kimi CLI 作为 AI 用户  
> **起草日期**: 2026-05-01  

---

## 一、项目背景

当前 devbase 作为 AI Agent 的知识底座，存在三层架构债务：

1. **Embedding 管道悬空**: `candle-*` 依赖已声明（Sprint 14），但 gated behind `local-embedding` optional feature。AI 调用 `hybrid_search` 时必须外部提供 `query_embedding`，破坏了 RAG 流水线的端到端自主性。
2. **上帝模块**: `mcp/tools/repo.rs` 2247 行，36 个 `crate::` 引用，同时包揽 HTTP、SQL、Tantivy、tree-sitter、JSON 序列化。
3. **存储三体问题**: SQLite + Tantivy + 文件系统无事务协调，`index` 时可能 Tantivy 已 commit 但 SQLite 回滚，导致 AI 检索到幽灵文档。

---

## 二、验收标准（Kimi CLI 作为 AI 用户的视角）

验收不是"编译通过"或"测试全绿"，而是 AI Agent 实际使用 devbase 时的体验指标。

### 2.1 端到端延迟（Latency）

| 场景 | 当前 | 目标 | 测量方式 |
|------|------|------|---------|
| `devbase index <repo>`（~22K LOC） | 35.0s | **< 20s** | `Measure-Command` |
| `devkit_hybrid_search`（含 embedding 生成） | N/A（需外部向量） | **< 2s** | MCP tool invoke 计时 |
| `devkit_code_symbols` 查询 | < 1s | < 1s（保持） | MCP tool invoke 计时 |

### 2.2 工具自主性（Autonomy）

AI 调用工具时，**不应需要手动干预或外部数据**。

| 工具 | 当前状态 | 目标 |
|------|---------|------|
| `devkit_hybrid_search` | AI 必须提供 `query_embedding` | AI 只提供 `query_text`，系统自动生成 embedding |
| `devkit_semantic_search` | 同上 | 同上 |
| `devkit_github_info` | ✅ 已自治 | 保持 |

### 2.3 上下文效率（Context Efficiency）

MCP 工具返回的 JSON 应在 AI 的上下文窗口中"高信噪比"。

| 工具 | 当前平均 token | 目标 | 优化手段 |
|------|---------------|------|---------|
| `devkit_project_context` | ~2000-4000 | **< 1000** | Context Compression 层 |
| `devkit_query_repos` | ~1500 | **< 500** | 分页 + 字段裁剪 |
| `devkit_code_symbols` | ~800 | < 800（保持） | — |

### 2.4 调用成功率（Reliability）

7 日运行期内，MCP 工具调用成功率 > 99%。

| 已知失败模式 | 根因 | 修复方案 |
|-------------|------|---------|
| `list_repos` NULL 崩溃 | `discovered_at` JSON 缺失 | Schema 硬化（Phase 1） |
| Tantivy 文件锁竞态 | Windows 多测试线程 | 测试隔离（已有 `SEARCH_TEST_LOCK`） |
| `index` 栈溢出（大仓库） | tree-sitter 递归耗栈 | 4MB 线程栈（已修复） |

### 2.5 错误可恢复性（Resilience）

工具失败时返回结构化错误，AI 能自动选择降级路径。

```json
{
  "success": false,
  "error": "embedding model not loaded",
  "fallback": "keyword_search",
  "fallback_tool": "devkit_natural_language_query"
}
```

---

## 三、分阶段实施路线图

### Phase 1: Schema 硬化（1 周）

**目标**: 消除 NULL 漂移，为 Repository 层奠定严格的类型基础。

**任务清单**:
- [ ] `discovered_at` 从 `metadata` JSON 提升为 `entities` 独立列，类型 `DATETIME NOT NULL DEFAULT current_timestamp`
- [ ] `workspace_type`、`data_tier` 提升为独立列，`NOT NULL DEFAULT 'git'/'private'`
- [ ] `language` 已有独立列，清理残留的 `json_extract(metadata, '$.language')` 查询
- [ ] 编写 `migrate/v15.rs`：迁移旧数据（`UPDATE entities SET discovered_at = COALESCE(...) ...`）
- [ ] 所有 `json_extract` 查询改为直接列引用或 `COALESCE`
- [ ] 删除 `entities.metadata` JSON 中的冗余字段（保留真正动态的字段如 `custom_attrs`）

**验收标准**:
- `cargo test --lib` 全过
- `devbase index`（不带参数，全量索引）不再因 NULL 崩溃
- `crate::` 引用数：无硬性目标，但 `registry/repo.rs` 的 SQL 应简化

**子代理任务**:
- 子代理 A: `migrate/v15.rs` + schema 变更 + 数据迁移脚本
- 子代理 B: 清理所有 `json_extract` 查询路径（`repo.rs`, `knowledge.rs`, `knowledge_engine.rs` 等）

---

### Phase 2: Repository 层提取（2 周）

**目标**: 消除上帝模块，将 `mcp/tools/repo.rs` 的 SQL 逻辑迁移到独立的 Repository 层。

**新建模块**:
```
src/
  repository/
    mod.rs
    repo_repository.rs      # repo CRUD + list/query
    symbol_repository.rs    # code_symbols + call_graph
    search_repository.rs    # Tantivy 操作封装
    embed_repository.rs     # embedding 生成（Phase 3 填充）
    summary_repository.rs   # repo_summaries + knowledge
    health_repository.rs    # repo_health
```

**任务清单**:
- [ ] 定义 `Repository` trait: `fn conn(&self) -> &rusqlite::Connection`
- [ ] `RepoRepository`: 提取 `repo.rs` 中的 SQL（`list_repos`, `save_repo`, `update_repo_language` 等）
- [ ] `SymbolRepository`: 提取 `code_symbols.rs`, `call_graph.rs`, `dead_code.rs` 的 SQL
- [ ] `SearchRepository`: 封装 Tantivy `IndexWriter` / `IndexReader` 操作，提供 `add_repo_doc`, `delete_repo_doc`, `commit`
- [ ] `SummaryRepository`: 提取 `knowledge.rs` 的 `save_summary`, `save_modules`, `list_modules`
- [ ] `mcp/tools/repo.rs` 只保留 `McpTool` 的 JSON 包装，所有业务调用转给 Repository 层

**验收标准**:
- `mcp/tools/repo.rs` `crate::` 引用数: 36 → **< 25**
- `mcp/tools/repo.rs` 行数: 2247 → **< 1500**
- 编译 0 errors，测试全过

**子代理任务**:
- 子代理 A: `repository/` 骨架 + `RepoRepository` + `SummaryRepository`
- 子代理 B: `SymbolRepository` + `SearchRepository`
- 子代理 C: 重写 `mcp/tools/repo.rs`，使用 Repository 层

---

### Phase 3: Embedding 闭环（Sprint 14 完成，1-2 周）

**目标**: 默认启用 `local-embedding`，AI 调用 `hybrid_search` 时零参数自动生成 embedding。

**技术栈**:
- `candle-core` + `candle-nn` + `candle-transformers`
- `tokenizers` (HuggingFace)
- `hf-hub` (模型自动下载)
- 模型: `sentence-transformers/all-MiniLM-L6-v2` (384 维，~80MB，CPU 实时)

**任务清单**:
- [ ] 将 `local-embedding` 从 optional feature 提升为 **默认 feature**
- [ ] 新建 `src/embedding/` 模块:
  - `model.rs`: 加载 all-MiniLM-L6-v2，提供 `encode(text: &str) -> Vec<f32>`
  - `cache.rs`: LRU 缓存（查询 text → embedding），避免重复计算
  - `batch.rs`: 批量编码接口（用于 `index` 时批量生成 symbol embedding）
- [ ] 修改 `hybrid_search` MCP 工具:
  - 如果 `query_embedding` 未提供，自动调用 `embedding::encode(query_text)`
  - 如果模型加载失败，返回结构化错误 + `fallback: "keyword_search"`
- [ ] 修改 `semantic_search` MCP 工具，同样逻辑
- [ ] 在 `index` 流程中，为每个 `code_symbol` 生成 embedding 并存储到 SQLite（新增 `symbol_embeddings` 表）

**新增表**:
```sql
CREATE TABLE symbol_embeddings (
    repo_id TEXT NOT NULL,
    symbol_name TEXT NOT NULL,
    embedding BLOB NOT NULL,  -- 384 x f32 = 1536 bytes
    PRIMARY KEY (repo_id, symbol_name)
);
```

**验收标准**:
- `devbase` 默认编译包含 embedding 功能（`cargo build` 自动链接 candle）
- `hybrid_search` 工具：AI 只传 `query_text`，系统 2s 内返回结果
- 首次模型下载后，后续查询 < 500ms（LRU cache）
- 模型加载失败时，工具返回 `fallback` 字段，AI 自动降级到 `nl_query`

**子代理任务**:
- 子代理 A: `embedding/` 模块 + candle 模型加载 + `encode()`
- 子代理 B: `symbol_embeddings` 表 + index 流程集成
- 子代理 C: `hybrid_search` / `semantic_search` 零参数改造

---

### Phase 4: 增量索引 + 事务协调（2 周）

**目标**: `index` 从全量重建变为增量更新；跨存储操作原子化。

**任务清单**:
- [ ] **Git Watcher**: 比较 `HEAD` 与上一次 index 的 commit hash（存储在 `repo_index_state` 表）
  - 变更文件列表: `git diff --name-only HEAD~1 HEAD`
  - 新增/修改的文件: 重新解析 + 更新 embedding
  - 删除的文件: 从 SQLite 和 Tantivy 中删除对应 symbol
- [ ] **Tantivy 增量**: 使用 `delete_term` 删除旧文档，只 add 变更文档
- [ ] **SQLite 增量**: 使用 `INSERT OR REPLACE` / `DELETE` 批量更新变更 symbol
- [ ] **Saga 协调器**:
  - 新建 `saga_log` 表: `(id, repo_id, step, status, created_at)`
  - `index` 流程拆分为 steps: `parse → embed → sqlite_save → tantivy_save → commit`
  - 每步完成前记录 saga_log；崩溃后从 saga_log 恢复或补偿

**新增表**:
```sql
CREATE TABLE repo_index_state (
    repo_id TEXT PRIMARY KEY,
    last_commit_hash TEXT,
    indexed_at DATETIME
);

CREATE TABLE saga_log (
    id TEXT PRIMARY KEY,
    repo_id TEXT NOT NULL,
    step TEXT NOT NULL,
    status TEXT NOT NULL,  -- 'pending', 'done', 'compensating'
    payload TEXT,          -- JSON
    created_at DATETIME DEFAULT current_timestamp
);
```

**验收标准**:
- `devbase index <repo>` 第二次运行时: **< 5s**（只处理变更文件）
- 全量索引后 Tantivy 和 SQLite 的 symbol 数量一致（一致性校验）
- 中断后重跑 `index` 能正确恢复，无重复数据

**子代理任务**:
- 子代理 A: Git Watcher + `repo_index_state` + 变更检测
- 子代理 B: Tantivy 增量删除/添加 + SQLite 批量更新
- 子代理 C: Saga 协调器 + 补偿逻辑

---

## 四、AI 用户验收测试脚本

Phase 3 完成后，由 Kimi CLI 实际执行以下 MCP 调用，评价体验：

```json
// Test 1: 零参数 hybrid_search
{"tool": "devkit_hybrid_search", "args": {"repo_id": "devbase", "query_text": "error handling in sync module"}}
// 期望: 返回 symbols，无需 query_embedding，耗时 < 2s

// Test 2: 上下文压缩
{"tool": "devkit_project_context", "args": {"project": "devbase"}}
// 期望: JSON < 1000 tokens，包含关键模块摘要而非原始数据 dump

// Test 3: 增量索引后一致性
{"tool": "devkit_code_symbols", "args": {"repo_id": "devbase", "symbol_type": "function"}}
{"tool": "devkit_hybrid_search", "args": {"repo_id": "devbase", "query_text": "function"}}
// 期望: 两个工具返回的 function 数量一致（±5%）

// Test 4: 错误降级
{"tool": "devkit_hybrid_search", "args": {"repo_id": "unknown_repo", "query_text": "test"}}
// 期望: 返回 {"success": false, "error": "...", "fallback_tool": "devkit_natural_language_query"}
```

---

## 五、风险与回滚策略

| 风险 | 影响 | 缓解措施 |
|------|------|---------|
| candle 模型下载失败（网络/HF 不可达） | Phase 3 阻塞 | 内嵌模型到 repo（`models/all-MiniLM-L6-v2`），或支持 `ort` ONNX 运行时 |
| schema 迁移损坏旧数据 | Phase 1 高风险 | 迁移前自动备份 SQLite；迁移脚本幂等（`IF NOT EXISTS`） |
| Repository 层引入循环依赖 | Phase 2 架构风险 | 严格单向依赖: `repository → registry → storage`，禁止反向 |
| 增量索引漏文件 | Phase 4 数据完整性 | `repo_index_state` 存储文件 mtime + size 校验，Git diff 为辅 |
| candle 编译时间剧增 | 开发体验 | 使用 `local-embedding` feature gate 保持可选，仅默认启用 |

---

## 六、子代理路由契约

| 阶段 | 子代理类型 | 任务 | 输入文件 | 输出文件 |
|------|-----------|------|---------|---------|
| Phase 1 | coder | schema 硬化 + 迁移 | `registry/repo.rs`, `registry/migrate.rs` | `migrate/v15.rs`, `registry/repo.rs` |
| Phase 2 | explore + coder | Repository 层提取 | `mcp/tools/repo.rs` | `repository/*.rs`, `mcp/tools/repo.rs` |
| Phase 3 | coder | Embedding 管道 | `Cargo.toml`, `mcp/tools/repo.rs` | `embedding/*.rs`, `mcp/tools/repo.rs` |
| Phase 4 | coder | 增量索引 + Saga | `knowledge_engine.rs`, `search.rs` | `indexer/*.rs`, `saga.rs` |

> **Hard Veto**: Rust 核心模块（`storage.rs`, `registry/migrate.rs` schema 变更）由主会话把控，子代理只执行非核心编码任务。
