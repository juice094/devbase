# devbase MCP 接口契约（草案 v0.1）

> 本文件定义 `devbase` 作为 MCP（Model Context Protocol）工具提供方，向 `Clarity` 等 Agent 框架暴露的接口契约。
>
> **状态**：已实现（2026-04-09）。`devbase mcp --transport stdio` 已可用，4 个工具均已完成 Rust 实现并注册到 MCP Server。

---

## 一、设计原则

1. **只读优先**：默认工具为只读查询，写操作（如 `auto-pull`）需显式参数 `dry_run=false` + `strategy=auto-pull`。
2. **结构化输出**：所有返回值均为 JSON，便于 LLM 解析和 reasoning。
3. **错误透明**：任何失败都必须返回 `success: false` + 人类可读的错误信息，Agent 可据此决定重试或降级。
4. **上下文自包含**：每个工具调用都携带 `workspace_root` 参数，支持多工作区场景。

---

## 二、工具清单

| 工具名 | 功能 | 默认安全级别 |
|--------|------|-------------|
| `devkit_scan` | 扫描目录并发现 Git 仓库 | 只读 |
| `devkit_health` | 检查注册仓库及环境健康状态 | 只读 |
| `devkit_sync` | 预览或执行仓库同步策略 | 可变（默认 dry-run） |
| `devkit_query` | 基于标签、语言、陈旧度等条件查询知识库 | 只读 |

---

## 三、接口详述

### 3.1 `devkit_scan`

**描述**：递归扫描指定目录下的 Git 仓库，返回仓库元数据列表。可选择是否将结果持久化到 devbase Registry。

#### 输入参数（JSON Schema）

```json
{
  "type": "object",
  "properties": {
    "path": {
      "type": "string",
      "description": "待扫描的目录路径，支持绝对路径和相对路径",
      "default": "."
    },
    "register": {
      "type": "boolean",
      "description": "是否将发现的仓库注册到本地 devbase 数据库",
      "default": false
    }
  },
  "required": ["path"]
}
```

#### 返回值（JSON Schema）

```json
{
  "type": "object",
  "properties": {
    "success": { "type": "boolean" },
    "count": { "type": "integer", "description": "发现的仓库数量" },
    "registered": { "type": "integer", "description": "实际注册到数据库的数量" },
    "repos": {
      "type": "array",
      "items": {
        "type": "object",
        "properties": {
          "id": { "type": "string" },
          "local_path": { "type": "string" },
          "upstream_url": { "type": ["string", "null"] },
          "default_branch": { "type": ["string", "null"] },
          "tags": { "type": "string" }
        }
      }
    }
  }
}
```

#### 使用示例

**Agent Prompt**："帮我看看用户桌面上有哪些 Git 项目？"

**调用**：
```json
{
  "name": "devkit_scan",
  "arguments": {
    "path": "C:\\Users\\22414\\Desktop",
    "register": false
  }
}
```

**返回**：
```json
{
  "success": true,
  "count": 3,
  "registered": 0,
  "repos": [
    { "id": "clarity", "local_path": "C:\\Users\\22414\\Desktop\\clarity", "upstream_url": null, "default_branch": "main", "tags": "own-project,no-upstream" },
    { "id": "devbase", "local_path": "C:\\Users\\22414\\Desktop\\devbase", "upstream_url": null, "default_branch": null, "tags": "tool" },
    { "id": "syncthing-rust-rearch", "local_path": "C:\\Users\\22414\\Desktop\\syncthing-rust-rearch", "upstream_url": null, "default_branch": "main", "tags": "own-project,no-upstream" }
  ]
}
```

---

### 3.2 `devkit_health`

**描述**：获取 devbase Registry 中所有仓库的健康状态快照，以及系统环境（工具链版本、磁盘空间）摘要。

#### 输入参数（JSON Schema）

```json
{
  "type": "object",
  "properties": {
    "detail": {
      "type": "boolean",
      "description": "是否返回每个仓库的详细状态",
      "default": false
    }
  }
}
```

#### 返回值（JSON Schema）

```json
{
  "type": "object",
  "properties": {
    "success": { "type": "boolean" },
    "summary": {
      "type": "object",
      "properties": {
        "total_repos": { "type": "integer" },
        "dirty_repos": { "type": "integer" },
        "behind_upstream": { "type": "integer" },
        "no_upstream": { "type": "integer" }
      }
    },
    "environment": {
      "type": "object",
      "properties": {
        "rustc": { "type": ["string", "null"] },
        "cargo": { "type": ["string", "null"] },
        "node": { "type": ["string", "null"] },
        "go": { "type": ["string", "null"] },
        "cmake": { "type": ["string", "null"] }
      }
    },
    "repos": {
      "type": "array",
      "items": {
        "type": "object",
        "properties": {
          "id": { "type": "string" },
          "local_path": { "type": "string" },
          "upstream_url": { "type": ["string", "null"] },
          "default_branch": { "type": ["string", "null"] },
          "status": {
            "type": "string",
            "enum": ["ok", "dirty", "behind", "ahead", "diverged", "no_upstream", "error"]
          },
          "ahead": { "type": "integer" },
          "behind": { "type": "integer" }
        }
      }
    }
  }
}
```

#### 使用示例

**Agent Prompt**："我的开发环境还健康吗？"

**调用**：
```json
{
  "name": "devkit_health",
  "arguments": { "detail": true }
}
```

**返回（节选）**：
```json
{
  "success": true,
  "summary": {
    "total_repos": 19,
    "dirty_repos": 0,
    "behind_upstream": 1,
    "no_upstream": 3
  },
  "environment": {
    "rustc": "1.94.1",
    "cargo": "1.94.1",
    "node": "v24.14.1",
    "go": "go1.26.1",
    "cmake": null
  },
  "repos": [
    { "id": "openclaw", "status": "behind", "ahead": 0, "behind": 4 },
    { "id": "lazygit", "status": "ok", "ahead": 0, "behind": 0 },
    { "id": "clarity", "status": "no_upstream", "ahead": 0, "behind": 0 }
  ]
}
```

---

### 3.3 `devkit_sync`

**描述**：对注册仓库执行批量同步策略。默认 `dry-run` 为安全预览模式，只有显式关闭时才执行真正的 `fetch`/`merge`。

#### 输入参数（JSON Schema）

```json
{
  "type": "object",
  "properties": {
    "dry_run": {
      "type": "boolean",
      "description": "若为 true，仅计算并返回同步计划，不修改任何本地文件",
      "default": true
    },
    "strategy": {
      "type": "string",
      "enum": ["fetch-only", "auto-pull", "ask"],
      "description": "fetch-only: 只获取远程状态; auto-pull: 工作区干净时自动快进合并; ask: 每次合并前询问（保留给交互式 UI）",
      "default": "fetch-only"
    },
    "filter_tags": {
      "type": "string",
      "description": "仅同步包含指定标签的仓库，逗号分隔。例如 third-party,reference",
      "default": ""
    }
  }
}
```

#### 返回值（JSON Schema）

```json
{
  "type": "object",
  "properties": {
    "success": { "type": "boolean" },
    "dry_run": { "type": "boolean" },
    "strategy": { "type": "string" },
    "results": {
      "type": "array",
      "items": {
        "type": "object",
        "properties": {
          "id": { "type": "string" },
          "action": {
            "type": "string",
            "enum": ["skipped", "fetch_only", "merged_ff", "merged_commit", "blocked_dirty", "conflict", "error"]
          },
          "ahead": { "type": "integer" },
          "behind": { "type": "integer" },
          "message": { "type": "string" }
        }
      }
    }
  }
}
```

#### 使用示例

**Agent Prompt**："帮我检查一下有哪些第三方库需要更新？"

**调用**：
```json
{
  "name": "devkit_sync",
  "arguments": {
    "dry_run": true,
    "strategy": "fetch-only",
    "filter_tags": "third-party,reference"
  }
}
```

**返回**：
```json
{
  "success": true,
  "dry_run": true,
  "strategy": "fetch-only",
  "results": [
    { "id": "openclaw", "action": "fetch_only", "ahead": 0, "behind": 4, "message": "0 ahead, 4 behind origin/main" },
    { "id": "lazygit", "action": "skipped", "ahead": 0, "behind": 0, "message": "Already up to date" }
  ]
}
```

---

### 3.4 `devkit_query`

**描述**：基于标签、语言、陈旧度等条件查询 devbase 知识库。支持简单关键词搜索和结构化表达式（未来扩展）。

#### 输入参数（JSON Schema）

```json
{
  "type": "object",
  "properties": {
    "expression": {
      "type": "string",
      "description": "查询表达式。MVP 阶段支持关键词搜索；未来支持 lang:rust stale:>30 behind:>0 等结构化语法"
    },
    "limit": {
      "type": "integer",
      "description": "最大返回结果数",
      "default": 50
    }
  },
  "required": ["expression"]
}
```

#### 返回值（JSON Schema）

```json
{
  "type": "object",
  "properties": {
    "success": { "type": "boolean" },
    "count": { "type": "integer" },
    "expression": { "type": "string" },
    "results": {
      "type": "array",
      "items": {
        "type": "object",
        "properties": {
          "id": { "type": "string" },
          "local_path": { "type": "string" },
          "upstream_url": { "type": ["string", "null"] },
          "tags": { "type": "string" },
          "default_branch": { "type": ["string", "null"] },
          "last_sync": { "type": ["string", "null"] }
        }
      }
    }
  }
}
```

#### 使用示例

**Agent Prompt**："我本地有哪些 Rust 相关的项目？"

**调用**：
```json
{
  "name": "devkit_query",
  "arguments": { "expression": "rust" }
}
```

**返回**：
```json
{
  "success": true,
  "count": 3,
  "expression": "rust",
  "results": [
    { "id": "clarity", "local_path": "C:\\Users\\22414\\Desktop\\clarity", "tags": "own-project,no-upstream" },
    { "id": "devbase", "local_path": "C:\\Users\\22414\\Desktop\\devbase", "tags": "tool" },
    { "id": "syncthing-rust-rearch", "local_path": "C:\\Users\\22414\\Desktop\\syncthing-rust-rearch", "tags": "own-project,no-upstream" }
  ]
}
```

---

## 四、与 Clarity 的集成方式（计划）

### 4.1 运行形态

`devbase` 将以轻量级 **MCP Server** 形式运行，提供 stdio 或 SSE 传输层。Clarity 作为 MCP Client，通过 `clap` 子命令或嵌入式服务启动它：

```bash
# stdio 模式（默认）
devbase mcp --transport stdio

# SSE 模式（可选，用于远程调试）
devbase mcp --transport sse --port 6277
```

### 4.2 当前实现状态

- `devbase mcp --transport stdio` 已可用，启动后进入基于 `tokio::io` 的 JSON-RPC 消息循环。
- 4 个工具（`devkit_scan`、`devkit_health`、`devkit_sync`、`devkit_query`）均已实现并注册到 MCP Server，输入输出严格匹配本契约的 JSON Schema。
- 已通过集成测试（`cargo test mcp`），覆盖 `initialize`、`tools/list`、各工具调用及错误处理场景。

### 4.3 Rust 侧接口参考

> 以下展示的是 devbase 内部 `McpTool` trait 的实现方式，供 Clarity 等调用方参考具体的 Rust 调用形态。

```rust
// src/mcp.rs (devbase 内部实现)
use crate::{scan, health, sync, query};

pub trait McpTool: Send + Sync {
    fn name(&self) -> &'static str;
    fn schema(&self) -> serde_json::Value;
    async fn invoke(&self, args: serde_json::Value) -> anyhow::Result<String>;
}

pub(crate) struct DevKitSyncTool;

#[async_trait::async_trait]
impl McpTool for DevKitSyncTool {
    fn name(&self) -> &'static str { "devkit_sync" }
    fn schema(&self) -> serde_json::Value { /* JSON Schema 同本契约第 3.3 节 */ serde_json::json!({}) }
    async fn invoke(&self, args: serde_json::Value) -> anyhow::Result<String> {
        sync::run_json(args).await
    }
}
```

### 4.3 上下文注入

在 Clarity 的会话初始化阶段，Agent 可隐式调用 `devkit_health` 或 `devkit_query`，将结果注入 system prompt 的上下文中：

```text
[System Context]
- 用户本地有 19 个 Git 仓库（3 个自有项目，16 个第三方参考库）。
- 当前 rustc 版本：1.94.1，node：v24.14.1，go：go1.26.1。
- openclaw 落后上游 4 个提交。
```

这样 LLM 在回答"帮我编译这个项目"时，能直接知道环境是否具备条件，无需反复询问用户。

---

## 五、版本与变更记录

| 日期 | 版本 | 变更 |
|------|------|------|
| 2026-04-05 | v0.1 | 初始草案。定义 4 个工具的 JSON Schema 和集成方式。devbase CLI 已实现对应能力，MCP Server 尚未开发。 |
