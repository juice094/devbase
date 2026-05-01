# MCP 集成指南

> 让任何 AI 助手通过 devbase 理解你的本地代码库。

---

## 什么是 MCP

MCP（Model Context Protocol）是 AI 助手与外部工具通信的标准协议。devbase 作为 **MCP Server**，向 AI 暴露 38 个结构化工具，让 AI 能够：

- 查询本地有哪些项目、它们的状态如何
- 批量同步仓库、检查健康度
- 获取代码统计、模块结构、调用关系
- 管理 Vault 笔记、查询操作日志

**核心优势**：AI 无法识别 GUI（桌面应用对 AI 是黑盒），但 AI 可以调用 MCP 工具。devbase 是 AI 理解本地代码库的**唯一结构化入口**。

---

## 快速配置

### 1. 安装 devbase

```bash
cargo install --path .
# 验证
devbase --version
```

### 2. 扫描并注册代码库

```bash
devbase scan . --register
devbase health --detail
```

### 3. 配置 AI 助手

#### Kimi CLI

Kimi CLI 使用 `~/.kimi/mcp.json`（Linux/macOS）或 `%USERPROFILE%\.kimi\mcp.json`（Windows）配置 MCP Server。

**一键配置**（从仓库模板复制）：

```powershell
# Windows
Copy-Item configs\kimi-mcp.json $env:USERPROFILE\.kimi\mcp.json

# Linux / macOS
cp configs/kimi-mcp.json ~/.kimi/mcp.json
```

**手动编辑**：

```json
{
  "mcpServers": {
    "devbase": {
      "command": "devbase",
      "args": ["mcp"],
      "env": {
        "DEVBASE_MCP_ENABLE_DESTRUCTIVE": "1",
        "DEVBASE_MCP_TOOL_TIERS": "stable,beta"
      }
    }
  }
}
```

配置后重启 Kimi CLI 或执行 `/mcp reload` 生效。

#### Claude Code

编辑 `~/.claude/mcp.json`：

```json
{
  "mcpServers": {
    "devbase": {
      "command": "devbase",
      "args": ["mcp"]
    }
  }
}
```

#### Cursor

在 Cursor Settings → MCP → Add Server：

- **Type**: Command
- **Command**: `devbase mcp`

---

## 环境变量

| 变量 | 默认值 | 说明 |
|------|--------|------|
| `DEVBASE_MCP_ENABLE_DESTRUCTIVE` | `0` | `1` 启用 destructive 工具（sync / skill_run / skill_discover / vault_write） |
| `DEVBASE_MCP_TOOL_TIERS` | `stable,beta,experimental` | 暴露哪些 tier 的工具，逗号分隔 |

---

## 工具列表概览

devbase 提供 38 个工具，按域分类：

| 域 | 工具数 | 代表能力 |
|:---|:---|:---|
| 仓库管理 | 5 | scan, health, sync, query_repos, index |
| 代码分析 | 6 | code_metrics, module_graph, code_symbols, call_graph, dependency_graph, dead_code |
| 知识检索 | 8 | semantic_search, hybrid_search, cross_repo_search, related_symbols, knowledge_report ... |
| Vault | 4 | vault_search, vault_read, vault_write, vault_backlinks |
| Skill | 4 | skill_list, skill_search, skill_run, skill_discover |
| 项目上下文 | 1 | project_context |
| 运维 | 3 | oplog_query, known_limit_store, known_limit_list |
| 其他 | 7 | query, note, digest, paper_index, github_info, arxiv_fetch, experiment_log |

完整清单参见 [`reference/mcp-tools.md`](../reference/mcp-tools.md)。

---

## 最佳实践

1. **先问 devbase，再读文件**：复杂任务先调用 `project_context` 获取结构，再按需读取源代码
2. **利用 Vault 做跨会话记忆**：关键决策写入 Vault，下次通过 `vault_search` 召回
3. **通过 OpLog 审计**：重要操作后查询 `devkit_oplog_query` 确认执行记录
4. **谨慎启用 destructive**：只在受信任的环境中设置 `DEVBASE_MCP_ENABLE_DESTRUCTIVE=1`
