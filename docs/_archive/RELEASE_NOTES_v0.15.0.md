# devbase v0.15.0 Release Notes

**Release Date**: 2026-05-04（CHANGELOG 收录日期；正式 git tag 在合入 main 后补打）
**Schema Version**: v30（无 schema 变更）
**Tests**: 490+ workspace passed / 0 failed / 4 ignored
**Branch**: `fix/project-health-cleanup`

---

## Highlights

本次发布以 **`plans/v0.15.0-directions.md` 调研结论**为路线图，按 P1~P5 优先级闭环交付五项工程能力，覆盖**搜索基础设施**、**架构纯度**、**离线适配**、**环境感知**与**CI 守卫**五个维度。

### P1 · Tantivy BM25 代码符号搜索

将 `hybrid.rs` 中的 SQLite `LIKE` 降级关键词路径替换为 **Tantivy BM25 索引**，实现真正意义上的代码符号级全文检索。

- 新增 `search/symbol_index.rs`：独立 Schema (`repo_id`, `name`, `signature`, `file_path`, `line_start`)
- `keyword_search_symbols` 主路径走 Tantivy BM25，SQLite LIKE 作为冷启动 fallback
- 索引流程 `index.rs` 在生成 code symbols 时**同步写入** symbol_index
- `StorageBackend` 扩展 `symbol_index_path()`（6 个 backend 实现全部覆盖）

**收益**：大仓库符号查询从 `LIKE '%token%'` 全表扫描升级为倒排索引 + BM25 评分。

---

### P2 · AppContext 职责拆分

`storage.rs` 此前承载了 7 个 Client trait 的实现（~860 行），违反 SRP 且阻碍单元测试。

**Phase 1**：6 个 Client trait impl 迁出 `storage.rs`
- `scan.rs` / `health.rs` / `sync.rs` / `digest.rs` / `knowledge_engine/mod.rs` / `registry.rs`
- `storage.rs` 860 → 430 行（**-50%**）
- 删除冗余 `conn_mut()`

**Phase 2**：内联 SQL 下沉
- 新增 `registry/code_symbols.rs` + `registry/dead_code.rs`
- `CodeSymbolRow` / `DeadCodeRow` + 纯函数查询（12 个单元测试）
- `RegistryClient` 退化为纯代理层（pure facade）

**收益**：消除了 devbase 最大的耦合黑洞之一；为后续 `devbase-mcp` 独立发布扫清障碍。

---

### P3 · Embedding 多后端（Candle + Ollama）

打破 `CandleProvider` 单后端 + 首次运行强制联网下载模型的痛点。

- 新增 `OllamaProvider`（`ureq` HTTP `/api/embed`）
- `create_provider(backend, model, base_url, timeout)` 配置化创建
- `generate_query_embedding` 通过 `OnceLock` 懒加载配置化 provider
- 默认模型改为 `all-minilm`（384-dim，与 Candle 维度兼容，方便后端切换）

**配置示例**（`config.toml`）：

```toml
[embedding]
backend = "candle"        # 或 "ollama"
model = "all-minilm"
base_url = "http://127.0.0.1:11434"  # Ollama only
timeout_seconds = 30
```

---

### P4 · Health 环境检测扩展

`EnvVersionCache` 从 **5 个工具链** 扩展到 **9 个**。

| 新增工具 | 检测命令 | 备注 |
|:---|:---|:---|
| `python` | `python --version` | 兼容 `python3` 回退 |
| `bun` | `bun --version` | |
| `zig` | `zig version` | |
| `java` | `java -version` | **stderr 输出**，新增 stderr fallback 解析 |

- `get_tool_version` 支持 stderr fallback（Java 习惯打到 stderr）
- `fmt_version` 改进：Java 引号提取、Docker/Python 格式处理

**收益**：现代开发环境（多语言混栈、容器化、jvm 生态）首次在 `devbase health` 中获得完整版本快照。

---

### P5 · 架构不变量自动化 CI

将 `docs/architecture/invariants.md` 中的人工审查规则转化为可在 PR 中自动 enforce 的脚本。

- 新增 `tools/invariant-checks/run-checks.ps1`
  - **G5**：diff-only 检测新增生产代码 `unwrap`/`expect`/`panic!`（自动排除 `#[cfg(test)]` 块）
  - **T11**：检测 `mcp/tools/*` 直接调用 `rusqlite::Connection`（违反 MCP trait 化方向）
  - **T12**：检测 `tui/render/*` 写入操作（违反 TUI 纯消费者约束）
- CI job `invariant-check` 加入 `.github/workflows/ci.yml`

**收益**：RF-6（生产 0 panic）和模块边界约束（T11/T12）从"代码审查依赖"升级为"CI 强制门控"。

---

## Changed

- `EmbeddingConfig` 默认模型：`nomic-embed-text` → `all-minilm`（384-dim 统一，方便 Candle/Ollama 切换）
- `AGENTS.md` 阶段描述更新：v0.14.3 → v0.15.0 已发布
- 主 crate `Cargo.toml` 与 `[workspace.package].version` bump 至 `0.15.0`

---

## Fixed

- **TTL 缓存负值 bug**（`97172ec`）：`elapsed < ttl_seconds` → `elapsed >= 0 && elapsed < ttl_seconds`
  - 防止系统时间回溯（NTP 调整、跨时区夏令时切换）导致缓存条目永不过期
- `crates/devbase-embedding/src/lib.rs` 遗留 `unwrap` 清零（`encode_with_candle` → `ok_or` 显式错误传播）

---

## Schema Changes

**无 schema 变更**。Schema 保持 v30（v0.14.3 引入的 `code_symbols.attributes` 列）。

---

## Architecture Health

| 维度 | v0.14.3 | v0.15.0 |
|:---|:---:|:---:|
| 生产 `unwrap()` 数 | 0 | 0 |
| `cargo clippy --all-targets -D warnings` | ✅ | ✅ |
| `cargo fmt --check` | ✅ | ✅ |
| Workspace crates | 18 | **19** |
| `storage.rs` 行数 | 860 | **430（-50%）** |
| `EnvVersionCache` 覆盖工具 | 5 | **9** |
| 自动化不变量检查 | 手工 | **CI 强制（G5/T11/T12）** |
| Embedding provider | Candle only | **Candle + Ollama 配置切换** |

---

## Upgrade Notes

- 无 breaking change；现有 SQLite registry / Tantivy 索引继续工作。
- 首次运行 v0.15.0 会自动建立 `symbol_index` 子目录（位置由 `StorageBackend::symbol_index_path()` 决定）。
- 如需切换到 Ollama embedding，参见上方 P3 章节的 `config.toml` 示例。

---

## Known Issues

- `search::tests::test_search_repos` / `test_search_vault` 在多线程下偶发 flaky（单线程通过）。根因未定位，已记录在 `docs/_audit/project-status-snapshot-2026-04-29.md` §B2。
- Tantivy + SQLite 双写仍无事务级一致性，依靠 v0.14.3 引入的反向补偿扫描兜底。

---

*本 Release Notes 由接管 Agent 于 2026-05-11 补写，弥补 v0.11.0~v0.14.x 期间 Release Notes 文件缺失的历史问题。后续版本应在发布同步生成。*
