# devbase 架构审计报告
日期: 2026-04-26
版本: v0.13.0 (Schema v25, main@98d54e6)
审计范围: 模块依赖、God struct、死代码、循环依赖

## 1. 模块依赖图

```
main.rs
├── storage (AppContext)
│   └── registry (WorkspaceRegistry::init_db_at)
├── registry (WorkspaceRegistry)
│   ├── knowledge
│   ├── repo
│   ├── migrate
│   └── test_helpers
├── mcp (37 tools)
│   ├── tools/context.rs ──→ registry::knowledge
│   ├── tools/repo.rs
│   └── run_stdio (重建 AppContext)
├── tui
├── cli
├── vault
├── workflow
├── skill_runtime
├── semantic_index
└── search (hybrid, tantivy)
```

- **无编译期 `use` 循环**
- **逻辑循环**: `storage::AppContext` → `registry::WorkspaceRegistry::init_db_at()` → `storage::DefaultStorageBackend`
- **`mcp::run_stdio` 重复 `main.rs` 初始化逻辑**

## 2. God Objects

| 对象 | 位置 | 影响范围 | 拆分优先级 |
|------|------|---------|-----------|
| `WorkspaceRegistry` | `src/registry.rs:152` | 46+ 文件 | **P1** |
| `AppContext` | `src/storage.rs:75` | 所有命令 + 37 MCP tools | P2 |
| `McpToolEnum` | `src/mcp/mod.rs:56` | 37 variants, ~8 处编辑/工具 | P2 |

### WorkspaceRegistry 拆分提案
拆分为 facade + 多个 focused registry:
- `KnowledgeRegistry` → `save_relation`, `hybrid_search_symbols`, `record_symbol_read`
- `RepoRegistry` → repo CRUD, tags, dependency graph
- `VaultRegistry` → vault notes, links
- `IndexRegistry` → Tantivy index ops

## 3. Schema vs Code Drift

> ⚠️ 注: `relations` 表在 v24 已激活（`save_relation`/`list_dependencies` 使用），审计 agent 基于启动时代码状态，此项已修正。

| 表 | 状态 | 说明 |
|---|------|------|
| `relations` | ✅ 已激活 | v24 迁移完成，`dependency_graph.rs` 读写 |
| `code_symbol_links` | 🟡 保留 | 设计确认: symbol 不是 entity，不迁移 |
| `code_call_graph` | 🟡 保留 | 设计确认: AST 元数据定位不同 |
| `ai_discoveries` | 🔴 死表 | 无生产读路径 |
| `entity_types` | 🔴 死表 | 无生产读路径 |
| `repos` | 🔴 幽灵表 | v21 迁移逻辑: CREATE → DROP |
| `repo_notes` | 🟡 弱引用 | 仅 `note:` 查询前缀消费者 |

## 4. 死代码

- `#[allow(dead_code)]`: 14 项
- `core::node.rs` / `core::mod.rs`: **完全未使用**
- `symbol_links.rs`: 仅测试中执行
- `sync_protocol.rs`: 仅测试调用，version-vector 方法标记 `#[allow(dead_code)]`

## 5. Actionable Items

| 优先级 | 事项 | 预估工作量 |
|--------|------|-----------|
| P1 | `WorkspaceRegistry` → facade 拆分 | 2-3 天 |
| P1 | `ai_discoveries` / `entity_types` 表清理 | 2h |
| P2 | `core/` 模块删除 | 30min |
| P2 | `sync_protocol.rs` 死代码清理 | 1h |
| P2 | `McpToolEnum` 宏生成（减少 ~8 处手动编辑）| 1 天 |
