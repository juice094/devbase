---
type: ConceptIndex
title: devbase MCP 概念索引
description: MCP Server 架构、工具分类、添加路径。
timestamp: 2026-06-25T11:15:50Z
tags: [mcp, tools, protocol, index]
---

# devbase MCP 概念索引

## 概述

devbase 通过 **Model Context Protocol (MCP)** 向 AI Agent 暴露能力。传输层为 **stdio only**，不暴露网络端口。

## 关键数字

- **MCP Tools**：71 个（见 `src/mcp/mod.rs` `McpToolEnum`）
- **传输**：stdio JSON-RPC 2.0
- **启动命令**：`devbase mcp [--tools stable,beta,experimental]`

## 工具分类

| 域 | 代表 Tools | 说明 |
|----|-----------|------|
| 仓库管理 | `devkit_scan`, `devkit_health`, `devkit_sync`, `devkit_status` | 核心高频 |
| 查询检索 | `devkit_query`, `devkit_query_repos`, `devkit_natural_language_query` | DSL + 自然语言 |
| 索引分析 | `devkit_index`, `devkit_index_stream`, `devkit_index_health` | 代码语义索引 |
| 代码分析 | `devkit_code_metrics`, `devkit_module_graph`, `devkit_call_graph`, `devkit_dead_code`, `devkit_code_symbols` | 静态分析 |
| Vault 笔记 | `devkit_vault_search/read/write/backlinks/daily/graph/history/export` | PARA 知识系统 |
| Skill | `devkit_skill_list/search/run/discover/sync` | Skill 生命周期 |
| 工作流 | `devkit_workflow_list/run/status` | YAML 编排 |
| Session | `devkit_session_save/list/resume/attach/detach/...` | Agent 会话记忆 |
| 关系/本体 | `devkit_relation_store/query/delete`, `devkit_ontology_import` | 知识图谱 |
| 搜索质量 | `devkit_hybrid_search`, `devkit_search_quality`, `devkit_cross_repo_search` | BM25 + 向量混合 |

## 快速定位

- Tool trait 定义：`src/mcp/mod.rs` `McpTool`
- Tool 实现目录：`src/mcp/tools/`
- Tool 路由枚举：`src/mcp/mod.rs` `McpToolEnum`
- 稳定性分级：`ToolTier`（Stable / Beta / Experimental）

## 相关文档

- [tool-adding-guide.md](./tool-adding-guide.md) — 添加新 Tool 的标准路径
