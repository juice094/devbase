# Memory Infrastructure Design

> devbase 作为"分布式记忆基础设施"的架构设计文档。
> 
> 版本：2026-04-15

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

## 2. 数据分级模型（Data Tier）

| Tier | 定义 | 默认同步策略 | 示例 |
|------|------|-------------|------|
| `public` | 完全开放、可公开分发的知识 | 同步到所有 peer，可上传公开数据集 | 农业百科、开源文档、去标识化的通用编程知识 |
| `cooperative` | 经用户授权后，可参与联邦学习或模式聚合的数据 | 同步到受信任 peer，授权后提取统计/模式 | 工具调用成功率、去标识化诊断案例、聚合后的问答对 |
| `private` | 永不出境的原始数据 | **仅本地存储**，不同步到任何外部节点 | 原始对话、私有代码、未脱敏的个人笔记 |

**默认策略**：所有新发现的工作区默认标记为 `private`（最保守）。用户通过 `devbase meta <repo> --tier <tier>` 显式升级分级。

---

## 3. 工作区类型（Workspace Type）

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

## 4. Syncthing 对接点

### 4.1 同步策略矩阵

| Data Tier | Syncthing 默认行为 | 用户可覆盖 |
|-----------|-------------------|-----------|
| `public` | ✅ 自动推送到所有 peer | 否 |
| `cooperative` | ✅ 自动推送到受信任 peer | 是（可降级为 local-only） |
| `private` | ❌ 不推送 | 是（可显式授权给特定本地设备） |

### 4.2 冲突解决

Syncthing 在并发修改时会产生 `.sync-conflict` 文件。devbase 的处理策略：

1. **检测**：`watch` 模块监控 `.sync-conflict` 文件的出现
2. **标记**：在 `repo_health` 中将该工作区标记为 `diverged`
3. **暂停**：自动暂停该工作区的 further auto-pull/auto-merge
4. **通知**：通过 `digest` 或 MCP 向用户/Agent 报告冲突
5. **解决**：保留 Git 风格的 merge 语义——用户手动 resolve 后，devbase 恢复同步

---

## 5. 变更检测策略

### 5.1 Git 工作区
继续使用 `git2::Repository::statuses()` 检测：
- `WT_MODIFIED` / `INDEX_MODIFIED` → dirty
- `graph_ahead_behind()` → ahead / behind
- 合并分析 → fast-forward / conflict

### 5.2 OpenClaw / Generic 工作区
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

**存储**：快照数据暂存于 SQLite 的 `workspace_snapshots` 表（未来 schema）。

---

## 6. MCP 接口展望

devbase 作为 MCP Server，计划向 Clarity 等 Agent 暴露以下与"记忆"相关的工具：

### `devkit_query_memory`
查询当前工作区的记忆状态：
```json
{
  "repo_id": "openclaw",
  "query": "last_synced_at",
  "data_tier": "private"
}
```

### `devkit_sync_memory`
触发指定工作区的同步（Git pull 或 Syncthing rescan）：
```json
{
  "repo_id": "openclaw",
  "target": "syncthing"
}
```

### `devkit_set_tier`
动态调整工作区的数据分级（需用户确认）：
```json
{
  "repo_id": "agri-paper",
  "tier": "cooperative"
}
```

---

## 7. 与 devbase Registry 的映射

当前 Registry Schema（v2）已支持：

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

## 8. 后续工作

1. **实现 `workspace_snapshots` 表**：支持 openclaw/generic 的变更检测
2. **扩展 `scan` 模块**：识别 `SOUL.md` / `.devbase` 标记的目录
3. **集成 Syncthing 冲突检测**：在 `watch` 模块中监听 `.sync-conflict`
4. **MCP 工具活化**：实现 `devkit_query_memory` / `devkit_sync_memory`
