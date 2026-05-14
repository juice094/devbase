## devbase v0.20.0 — 知识完备性

**主题**：从"能存"到"好用"，消除知识库能力缺口。

---

### ✨ 新功能

#### Vault 双向链接图遍历（Sprint E/F 延伸）
- **`devkit_vault_graph`** 支持 BFS 双向遍历：`note_id` + `depth`（1–3）参数
- DB-first 构建：从 SQLite `outgoing_links` 出发，自动补全 incoming 边
- 知识图谱不再只是静态导出，而是可查询的图结构

#### Vault Git-based 历史追踪（Sprint E）
- **`devkit_vault_history`**（第 67 个 tool）：基于 `git2` revwalk 的笔记变更追踪
- 支持 blob 内容比对，返回行级 `insertions` / `deletions`
- `VaultClient` trait 扩展：`get_vault_history()` 统一接口

#### 混合检索质量监控（Sprint F）
- **`devkit_search_quality`**（第 68 个 tool）：返回 `HybridSearchMetrics`
  - `latency_ms` — 查询耗时
  - `keyword_recall` / `vector_recall` — 各路召回数
  - `rrf_overlap` —  keyword & vector 重叠项
  - `keyword_source` — `"tantivy"` 或 `"sqlite_fallback"`
  - `rrf_k` — 融合常数

#### Block 引用支持（Sprint G）
- `WikiLink.anchor`：支持 `[[note#heading]]` 与 `[[note#^block-id]]`
- `VaultNote.block_refs`：块级引用持久化（JSON metadata，无 schema 迁移）
- Vault 导出时自动检测 broken block refs（heading 锚点不存在时报告）

#### 性能回归基线（Sprint C）
- 新增 `#[ignore]` 性能回归测试：
  - 1k 文档 keyword search `< 200ms`
  - 10k 文档 keyword search `< 500ms`
- **ADR-003**：Redis 缓存评估完成 → **决策放弃引入**，现有 SQLite + Tantivy 栈已足够

---

### 🏛️ 架构原则

#### Client-Agnostic Principle（客户端无关原则）
- 核心能力零硬编码客户端路径（移除 `.clarity` 硬编码、泛化 `skill sync` 接口）
- 适配层（`scripts/claude/`）与核心代码严格分离
- 删除 Kimi/Claude 后，devbase 的编排、注册、索引、搜索、同步能力完全独立

---

### 📊 统计

| 指标 | v0.18.0 | v0.20.0 | 变化 |
|:---|:---:|:---:|:---:|
| MCP Tools | 64 | **68** | +4 |
| Tests | 437 | **451** | +14 |
| Workspace Crates | 18 | **19** | +1 |
| Schema | 34 | 34 | 稳定 |

---

### 🛠️ 修复与优化

- **Clippy 清零**：生产代码 `field-reassign-with-default` 修复
- **Rustfmt**：8 文件格式统一
- **Tantivy 健康评分**：`devkit_index_health`（Beta）持续可用
- **SQLite WAL**：默认启用，并发安全

---

### ⚠️ 已知限制

- `tree-sitter` grammar 编译成本仍维持 15–20s（已 feature-gate，可选关闭）
- Vault 历史依赖用户将 `vault/` 目录初始化为 Git 仓库
- Block 引用中的 `^block-id` 锚点目前仅解析，不验证存在性

---

### 📦 安装

```powershell
# Windows
irm https://raw.githubusercontent.com/juice094/devbase/main/scripts/install.ps1 | iex

# 或下载预编译二进制
wget https://github.com/juice094/devbase/releases/download/v0.20.0/devbase-v0.20.0-x86_64-pc-windows-msvc.exe -O devbase.exe
```

```bash
# Linux
wget https://github.com/juice094/devbase/releases/download/v0.20.0/devbase-v0.20.0-x86_64-unknown-linux-gnu -O devbase
chmod +x devbase
```

---

### 🙏 致谢

本版本全部功能由单人维护完成，遵循 **Client-Agnostic Principle** 与 **Local-First** 架构红线。
