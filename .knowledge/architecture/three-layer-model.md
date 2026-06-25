---
type: ArchitectureModel
title: devbase 三层架构模型
description: 交互层、编译层、可靠层的职责划分与数据流转。
timestamp: 2026-06-25T11:15:50Z
tags: [architecture, three-layer-model, data-flow]
---

# devbase 三层架构模型

```text
┌─────────────────────────────────────────┐
│ 交互层（Application / Protocol Layer）   │
│  • TUI 仪表盘（ratatui）                 │
│  • CLI 子命令（clap）                    │
│  • MCP Server（stdio，71 tools）         │
│  • Workflow 引擎（YAML 编排）            │
├─────────────────────────────────────────┤
│ 编译层（Compilation / Knowledge Layer）  │
│  感知 → 知识 → 策略                      │
│  • scan / vault/scanner / discovery      │
│  • entities / relations / embeddings     │
│  • query / health / search/hybrid        │
│  • sync / skill_runtime / workflow       │
├─────────────────────────────────────────┤
│ 可靠层（Reliability / Storage Layer）    │
│  • SQLite（WAL 模式）                    │
│  • Tantivy 全文/符号索引                 │
│  • OpLog 操作审计                        │
│  • 迁移前自动备份                        │
└─────────────────────────────────────────┘
```

## 交互层

**职责**：为人类开发者和 AI Agent 提供接口。

| 接口 | 实现 | 关键模块 |
|------|------|----------|
| CLI | `clap` derive | `src/main.rs`, `src/commands/` |
| TUI | `ratatui` + `crossterm` | `src/tui/` |
| MCP Server | stdio JSON-RPC 2.0 | `src/mcp/` |
| Workflow | YAML DSL | `src/workflow/` |

**约束**：
- TUI 是纯消费者层，禁止写入 registry（T12）。
- MCP Tool 不得直接调用 `rusqlite::Connection`，必须通过 `registry` 封装（T11）。
- 所有状态变更 MCP tool 必须幂等（G3）。

## 编译层

**职责**：把无结构的本地资产转化为结构化情境。

### 感知

| 模块 | 输入 | 输出 |
|------|------|------|
| `scan` | 文件系统 | Git 仓库、语言检测、代码统计 |
| `vault/scanner` | Markdown 笔记 | frontmatter、wikilink |
| `discovery_engine` | Cargo.toml / package.json | 跨仓库依赖关联 |

### 知识

| 模块 | 输入 | 输出 |
|------|------|------|
| `registry` | 感知层数据 | SQLite 中的 entities / relations / repos / vault_notes |
| `semantic_index` | 源码文件 | tree-sitter 符号、调用图 |
| `embedding` | 文本 | f32 向量、cosine_similarity |
| `symbol_links` | code_symbols | 相似签名、同文件聚类关系 |

### 策略

| 模块 | 职责 |
|------|------|
| `query` | 结构化查询：`lang:rust stale:>30` |
| `health` | dirty / ahead / behind / diverged |
| `search/hybrid` | BM25 + 向量 RRF 融合 |
| `sync` | 批量同步编排 |
| `skill_runtime` | Skill 发现/安装/执行/评分/发布 |
| `workflow` | YAML 工作流调度与执行 |

## 可靠层

**职责**：保证数据安全、可审计、可恢复。

| 组件 | 技术 | 说明 |
|------|------|------|
| Registry DB | SQLite + WAL | `registry.db`，bundled 模式 |
| 全文索引 | Tantivy | `search_index/` |
| 符号索引 | Tantivy | `symbol_index/` |
| 审计 | OpLog | 所有 scan/sync/health 自动记录 |
| 备份 | 自动快照 | Schema 迁移前生成 `backup-YYYYMMDD-HHMMSS.db` |

## 数据流转示例

```text
1. scan 发现本地仓库
   └─ index 提取模块结构（cargo metadata）
   └─ semantic_index 提取符号 + 调用图

2. registry 存储 repo 元数据、符号、调用边

3. query / search/hybrid / project_context 聚合查询

4. MCP Server 将 JSON 传给 AI Agent

5. TUI 为人类展示仓库健康状态
```
