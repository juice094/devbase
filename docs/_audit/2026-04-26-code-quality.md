# devbase 代码质量审计报告
日期: 2026-04-26
版本: v0.13.0 (Schema v25)
审计范围: unsafe、TODO、重复代码、函数/文件长度、Public API、错误处理

## 1. Unsafe 审计

- **生产代码 unsafe: 0**
- 全部 683 处 `.unwrap()` 均在 `#[cfg(test)]` 块中
- 生产代码已达成 zero-unwrap

## 2. TODO / FIXME / XXX / HACK

- **TODO: 1 处** (`skill_runtime/dependency.rs:168`)
- FIXME / XXX / HACK: 0

## 3. 代码重复

### 3.1 MCP Tools JSON Schema 样板
- `mcp/tools/repo.rs`: 2,376 行，25+ 工具有大量复制粘贴的 `inputSchema` 构造
- 建议: 提取 `mcp_schema!` 宏或 builder pattern

### 3.2 SQL Join 模式
- `entities` + `repo_tags` join 在 `registry/repo.rs` 和 `mcp/tools/repo.rs` 中重复
- 建议: 提取到 `registry::queries` 模块

### 3.3 i18n 双胞胎
- `i18n/en.rs` 和 `i18n/zh_cn.rs`: 137 行结构完全相同的映射表
- 建议: 用 `phf` 或 `fluent` 替代当前硬编码 struct

## 4. 过长函数 (>100 lines)

- **16 个函数**超过 100 行
-  standout: `registry/migrate.rs::init_db_at` — **1,214 行**
  - 包含全部 25 个 schema migration
  - 建议: 拆分为 `migrations/v{n}.rs` 模块

## 5. 过长文件 (>1000 lines)

| 文件 | 行数 | 主要问题 |
|------|------|---------|
| `mcp/tools/repo.rs` | 2,376 | 25+ 工具聚合，schema 样板重复 |
| `tui/state.rs` | 1,298 | UI 状态机复杂 |
| `registry/migrate.rs` | 1,273 | 25 个 migration 内联 |
| `semantic_index.rs` | 1,133 | 多职责 |
| `knowledge_engine.rs` | 1,023 | 知识推理 + 持久化混合 |

## 6. Public API 表面

- `lib.rs` re-exports **全部 32 模块** 为 `pub mod`
- `registry/`, `mcp/tools/`, `skill_runtime/`, `tui/`, `vault/`, `workflow/` 中大量 `pub` 项仅内部消费
- 建议: 降级为 `pub(crate)`，减少 API 承诺

## 7. 错误处理一致性

- ~90% 使用 `anyhow::Result`
- 异常:
  - `search.rs`: 直接泄漏 `TantivyError`
  - `skill_runtime/clarity_sync.rs`: 裸 `Result<>`（隐式 anyhow）
- `FromStr` impls 均正确

## 8. Actionable Items

| 优先级 | 事项 | 预估工作量 |
|--------|------|-----------|
| P1 | `init_db_at` 拆分为 per-version migration 模块 | 4h |
| P1 | `mcp/tools/repo.rs` 提取 schema 宏 | 1 天 |
| P2 | `lib.rs` pub mod 审计降级 | 2h |
| P2 | `search.rs` TantivyError 封装 | 1h |
| P2 | i18n 硬编码 struct 替换 | 4h |
