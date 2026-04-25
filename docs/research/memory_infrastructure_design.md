# Memory Infrastructure Design

> devbase 作为"分布式记忆基础设施"的架构设计文档。
> 
> 版本：v0.2.3 · 2026-04-23

---

## 1. 设计目标

devbase 的管理对象从 "Git 仓库" 升级为 **"知识工作区（Knowledge Workspace）"**。任何被开发者视为"需要被追踪、保鲜、同步的认知资产"的目录，都可以被 devbase 注册为工作区。

核心职责：
- **发现（Discover）**：自动识别本地环境中的知识工作区
- **分级（Classify）**：按敏感度对数据分级，决定其流通边界
- **检测（Detect）**：监控工作区的变更状态（dirty / behind / diverged）
- **同步（Sync）**：与上游（Git）或 peer（Syncthing）保持一致
- **服务（Serve）**：通过 MCP 协议向 Clarity 等 Agent 提供结构化环境上下文

---

## 2. 与当前 Vault 系统的映射

本文档最初写于 2026-04-15（v0.1.0 前夕），当时"记忆基础设施"主要以工作区快照和 Syncthing 同步为核心。经过 v0.2.0–v0.2.3 的迭代，**Vault 笔记系统已成为当前"记忆基础设施"的主要落地形态**。

### 2.1 已实现的 Vault MCP 工具

| 工具 | 功能 | 状态 |
|------|------|------|
| `devkit_vault_search` | 按关键词搜索 Vault 笔记（标题、标签、内容） | ✅ v0.2.0 实现 |
| `devkit_vault_read` | 读取单篇笔记的完整内容与 frontmatter | ✅ v0.2.0 实现 |
| `devkit_vault_write` | 创建或追加笔记内容 | ✅ v0.2.0 实现 |
| `devkit_vault_backlinks` | 查询笔记的反向链接（WikiLink `[[...]]`） | ✅ v0.2.2 实现 |

### 2.2 项目上下文关联

`devkit_project_context`（v0.2.0+）提供了 **repo 与 vault 的统一查询入口**：
- 根据项目标识匹配已注册的仓库元数据
- 通过 `vault_repo_links` 表查询显式关联的 Vault 笔记
- 按项目名称模糊匹配 Vault 笔记路径/ID
- 扫描 `workspace/assets/` 下的项目相关资源

这意味着 Agent 不再需要独立的 `devkit_query_memory` 来"查询工作区记忆"——`devkit_project_context` 已经覆盖了 repo + vault + assets 的关联检索。

### 2.3 架构原则的演进

Vault 系统采用了与原文档一致的**文件系统即真相源**原则：
- 笔记内容存储在 `%LOCALAPPDATA%/devbase/workspace/vault/` 的 `.md` 文件中
- SQLite `vault_notes` 表仅索引元数据（title, tags, outgoing_links 等），不存储正文
- Tantivy 全文索引作为派生缓存，支持快速搜索

这一演进使得"记忆基础设施"从纯技术性的工作区快照，扩展为**可写、可搜、可关联的开发者知识库**。

---

## 3. 数据分级模型（Data Tier）

| Tier | 定义 | 默认同步策略 | 示例 |
|------|------|-------------|------|
| `public` | 完全开放、可公开分发的知识 | 同步到所有 peer，可上传公开数据集 | 农业百科、开源文档、去标识化的通用编程知识 |
| `cooperative` | 经用户授权后，可参与联邦学习或模式聚合的数据 | 同步到受信任 peer，授权后提取统计/模式 | 工具调用成功率、去标识化诊断案例、聚合后的问答对 |
| `private` | 永不出境的原始数据 | **仅本地存储**，不同步到任何外部节点 | 原始对话、私有代码、未脱敏的个人笔记 |

**默认策略**：所有新发现的工作区默认标记为 `private`（最保守）。用户通过 `devbase meta <repo> --tier <tier>` 显式升级分级。

---

## 4. 工作区类型（Workspace Type）

| Type | 描述 | 变更检测方式 | 当前支持状态 |
|------|------|-------------|-------------|
| `git` | 传统 Git 仓库 | `git2` 原生状态查询 | ✅ 已支持 |
| `openclaw` | Agent 记忆空间（如 `~/.openclaw/workspace`） | 文件 `mtime` + `blake3` 哈希快照 | 📝 设计阶段 |
| `generic` | 任意目录的知识库（如 `agri-paper/data`） | 文件 `mtime` + `blake3` 哈希快照 | 📝 设计阶段 |

**非 Git 工作区的检测逻辑（未来实现）**：
```rust
// 伪代码
fn detect_generic_workspace(path: &Path) -> bool {
    path.join("SOUL.md").exists()
        || path.join("MEMORY.md").exists()
        || path.join(".devbase").exists()
}
```

---

## 5. Syncthing 对接点

### 5.1 同步策略矩阵

| Data Tier | Syncthing 默认行为 | 用户可覆盖 |
|-----------|-------------------|-----------|
| `public` | ✅ 自动推送到所有 peer | 否 |
| `cooperative` | ✅ 自动推送到受信任 peer | 是（可降级为 local-only） |
| `private` | ❌ 不推送 | 是（可显式授权给特定本地设备） |

### 5.2 冲突解决

Syncthing 在并发修改时会产生 `.sync-conflict` 文件。devbase 的处理策略：

1. **检测**：`watch` 模块监控 `.sync-conflict` 文件的出现
2. **标记**：在 `repo_health` 中将该工作区标记为 `diverged`
3. **暂停**：自动暂停该工作区的 further auto-pull/auto-merge
4. **通知**：通过 `digest` 或 MCP 向用户/Agent 报告冲突
5. **解决**：保留 Git 风格的 merge 语义——用户手动 resolve 后，devbase 恢复同步

---

## 6. 变更检测策略

### 6.1 Git 工作区

继续使用 `git2::Repository::statuses()` 检测：
- `WT_MODIFIED` / `INDEX_MODIFIED` → dirty
- `graph_ahead_behind()` → ahead / behind
- 合并分析 → fast-forward / conflict

### 6.2 OpenClaw / Generic 工作区

引入**轻量级快照机制**（不依赖 Git）：

```rust
pub struct WorkspaceSnapshot {
    pub repo_id: String,
    pub file_hashes: HashMap<PathBuf, String>, // blake3 hash
    pub recorded_at: DateTime<Utc>,
}
```

检测流程：
1. 遍历工作区下的所有文件（排除 `.git/`、临时文件）
2. 计算每个文件的 `blake3` 哈希
3. 与上一次 `WorkspaceSnapshot` 对比
4. 若有差异 → 标记为 `dirty`
5. 更新 `last_synced_at` 为最近一次 Syncthing 完成同步的时间

**存储**：`workspace_snapshots` 表已在 Schema v3 中创建（见 `src/registry/migrate.rs`），但**写入与比对逻辑尚未实现**，openclaw/generic 工作区的实际变更检测仍停留在设计阶段。

---

## 7. MCP 接口展望

devbase 作为 MCP Server，向 Clarity 等 Agent 暴露以下与"记忆"相关的工具：

### 7.1 已实现的 Vault 工具（替代原"记忆查询"概念）

- `devkit_vault_search` — 全文搜索 Vault 笔记
- `devkit_vault_read` — 读取笔记内容与 frontmatter
- `devkit_vault_write` — 创建或追加笔记
- `devkit_vault_backlinks` — 查询笔记反向链接
- `devkit_project_context` — 统一项目上下文（repo + vault + assets）

### 7.2 尚未实现的早期设计工具

以下工具在本文档初版中提出，但**截至 v0.2.3 仍未实现**：

#### `devkit_query_memory`
查询当前工作区的记忆状态：
```json
{
  "repo_id": "openclaw",
  "query": "last_synced_at",
  "data_tier": "private"
}
```
> **状态**：📝 未实现。其功能已被 `devkit_project_context` 和 `devkit_query_repos` 部分覆盖。

#### `devkit_sync_memory`
触发指定工作区的同步（Git pull 或 Syncthing rescan）：
```json
{
  "repo_id": "openclaw",
  "target": "syncthing"
}
```
> **状态**：📝 未实现。Git 同步已可通过 `devkit_sync` 完成；Syncthing 集成仍待开发。

#### `devkit_set_tier`
动态调整工作区的数据分级（需用户确认）：
```json
{
  "repo_id": "agri-paper",
  "tier": "cooperative"
}
```
> **状态**：📝 未实现。目前仅支持通过 CLI `devbase meta <repo> --tier <tier>` 手动设置。

---

## 8. 与 devbase Registry 的映射

当前 Registry Schema（v8）已支持：

```sql
CREATE TABLE repos (
    id TEXT PRIMARY KEY,
    local_path TEXT NOT NULL,
    language TEXT,
    discovered_at TEXT NOT NULL,
    workspace_type TEXT DEFAULT 'git',
    data_tier TEXT DEFAULT 'private',
    last_synced_at TEXT
);
```

**CLI 接口**：
```bash
# 设置数据分级
devbase meta <repo_id> --tier cooperative

# 设置工作区类型
devbase meta <repo_id> --workspace-type openclaw
```

---

## 9. 后续工作

| 序号 | 任务 | 状态 | 备注 |
|------|------|------|------|
| 1 | 实现 `workspace_snapshots` 表写入与比对逻辑 | 📝 未实现 | Schema v3 已创建表，但 Rust 端检测逻辑未接入 |
| 2 | 扩展 `scan` 模块识别 `SOUL.md` / `.devbase` 标记的目录 | 📝 未实现 | openclaw/generic 工作区自动发现的前提 |
| 3 | 集成 Syncthing 冲突检测 | 📝 未实现 | `.sync-conflict` 监听与 `repo_health` 标记 |
| 4 | MCP 工具 `devkit_query_memory` | 📝 未实现 | 功能被 `devkit_project_context` 部分覆盖 |
| 5 | MCP 工具 `devkit_sync_memory` | 📝 未实现 | Syncthing 触发层待开发 |
| 6 | MCP 工具 `devkit_set_tier` | 📝 未实现 | 需设计用户确认交互流程 |
| 7 | Vault 笔记系统 | ✅ 已实现 | v0.2.0–v0.2.2 完成 search/read/write/backlinks |
| 8 | `devkit_project_context` 统一上下文 | ✅ 已实现 | v0.2.0 提供 repo + vault + assets 关联查询 |

---

*本文档于 2026-04-23 根据 v0.2.3 实现状态修订。原始版本：2026-04-15。*
