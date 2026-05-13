# RFC: ClaudeCode 工作流深度集成 — v0.18.0

**Status**: Draft  
**Target Version**: v0.18.0  
**Author**: juice094  
**Date**: 2026-05-13

## 1. 用例分析

ClaudeCode 是 Anthropic 推出的终端 AI 编程助手。其典型工作流：

```
1. 启动      → Claude 扫描项目目录，建立初步理解（往往耗时且片面）
2. 需求理解  → 用户用自然语言描述需求
3. 文件探索  → Claude 用 grep/find 暴力搜索相关代码
4. 编辑执行  → 读文件 → 改文件 → 验证（循环）
5. 提交      → git add/commit/push
6. 会话结束  → 对话历史丢失，下次从零开始
```

**痛点**：
- P1: 启动扫描慢，对大型仓库（如 devbase 本身）需要数十秒才能建立上下文
- P2: 代码搜索依赖关键词匹配，无法基于语义（"找认证相关的代码" → grep "auth" 漏掉 "login"）
- P3: 修改前无影响分析，经常漏改调用点或测试
- P4: 会话不持久，跨会话知识丢失（昨天的决策今天不记得）
- P5: 无法自动执行标准化工作流（如：修改 → clippy → test → commit message生成）

## 2. 设计目标

让 devbase 成为 ClaudeCode 的"外接海马体"：

| 环节 | devbase 能力 | ClaudeCode 收益 |
|------|-------------|----------------|
| 启动 | `devkit_project_brief` | 秒级获得项目全景，替代暴力扫描 |
| 探索 | `devkit_hybrid_search` | 语义搜索找代码，减少 50% 文件读取 |
| 编辑前 | `devkit_impact_analysis` | 修改前预知影响范围，降低回归风险 |
| 验证 | `devkit_evaluate` | 一键运行 clippy/test/fmt，保障质量 |
| 会话中 | `devkit_session_save/capture` | 关键决策实时沉淀为项目记忆 |
| 跨会话 | `devkit_session_recall` | 启动时自动注入相关历史决策 |
| 复杂任务 | Workflow Engine | 标准化重构/发布/审查流程 |

## 3. 核心功能设计

### 3.1 Project Brief Generator (`devkit_project_brief`)

**问题**：Claude 启动时需要快速理解项目结构、关键模块、技术约束。

**设计**：
```json
{
  "repo_id": "devbase",
  "format": "markdown" // markdown | json
}
```

**输出结构**：
```markdown
# Project Brief: devbase

## Overview
本地优先的 AI 情境编译器。Rust CLI，SQLite + Tantivy 索引。

## Key Modules
- src/mcp/ — MCP Server（60 tools）
- src/registry/ — SQLite schema + migrations
- src/workflow/ — YAML workflow engine
- crates/ — 18 个零耦合 workspace crates

## Dependency Graph（高内聚模块）
- mcp → registry → storage
- workflow → skill_runtime → registry

## Active Contexts
- feat/claudecode-integration（当前分支关联的 context）

## Known Limits
- [L3-001] Windows 路径长名问题（已缓解）
- [L3-002] Candle 编译时间（v0.17.0 已外迁）

## Recent Changes（最近 7 天）
- v0.17.0: Agent Memory 向量存储
- v0.16.1: Workflow-Session 绑定
```

**实现**：聚合 `repo_modules` + `code_symbols` + `known_limits` + `oplog` + `agent_contexts` 数据，生成 LLM-optimized Markdown。

### 3.2 Impact Analysis (`devkit_impact_analysis`)

**问题**：Claude 说"我要重构 `run_skill`"，但不知道谁调用了它、哪些测试覆盖它。

**设计**：
```json
{
  "repo_id": "devbase",
  "symbol_name": "run_skill",
  "depth": 2 // 调用链深度
}
```

**输出**：
```json
{
  "symbol": "run_skill",
  "file": "src/skill_runtime/executor.rs:12",
  "callers": [
    {"symbol": "execute_skill_step", "file": "src/workflow/executor.rs:45"},
    {"symbol": "test_run_skill_success", "file": "src/skill_runtime/executor.rs:502"}
  ],
  "callees": [
    {"symbol": "resolve_interpreter", "file": "src/skill_runtime/executor.rs:257"},
    {"symbol": "recall_context_memories", "file": "src/skill_runtime/executor.rs:231"}
  ],
  "related": [
    {"symbol": "ExecutionResult", "file": "src/skill_runtime/mod.rs:45", "link_type": "return_type"}
  ],
  "tests": [
    "test_run_skill_success",
    "test_run_skill_not_found",
    "test_hard_veto_guard"
  ],
  "history": [
    {"date": "2026-05-13", "change": "添加 auto-recall 逻辑", "commit": "b1fff28"}
  ]
}
```

**实现**：复用现有的 `call_graph` + `related_symbols` + `dead_code` + `code_symbols` 数据，通过统一的 `impact_analysis` API 聚合。

### 3.3 Session-Aware Claude Wrapper

**问题**：ClaudeCode 会话结束即丢失，无法跨会话保持上下文。

**设计**（无需修改 ClaudeCode 本身）：

提供一个 wrapper 脚本 `devbase-claude`：

```bash
#!/bin/bash
# devbase-claude wrapper

# 1. 读取 active context
CONTEXT_ID=$(devbase context resolve)

# 2. 如果有 context，导出 memories 为 Claude system prompt 补充
if [ -n "$CONTEXT_ID" ]; then
  MEMORIES=$(devbase session recall --context-id "$CONTEXT_ID" --limit 10)
  export CLAUDE_SYSTEM_PROMPT_EXTRA="$MEMORIES"
fi

# 3. 生成 project brief
BRIEF=$(devbase project brief --repo-id "$(basename $(pwd))")
export CLAUDE_PROJECT_BRIEF="$BRIEF"

# 4. 启动 ClaudeCode
claude "$@"

# 5. 会话结束后，自动捕获对话摘要（需要用户确认）
echo "Capture this session to devbase? [y/N]"
read -r CAPTURE
if [ "$CAPTURE" = "y" ]; then
  devbase session capture "$CONTEXT_ID" "decision" "$(cat /tmp/claude-summary.txt)"
fi
```

**长期**：向 Anthropic 提议官方 MCP integration，让 ClaudeCode 原生支持 devbase tools。

### 3.4 Standardized Development Workflow

**设计**：预置 workflow YAML，Claude 可通过 `devkit_workflow_run` 触发。

**`workflows/refactor.yml`**：
```yaml
id: safe-refactor
name: Safe Refactor Pipeline
inputs:
  - name: repo_id
  - name: symbol_name
  - name: description
steps:
  - id: analyze
    step_type: skill
    skill: devbase-impact-analysis
    inputs:
      repo_id: "{{ inputs.repo_id }}"
      symbol_name: "{{ inputs.symbol_name }}"

  - id: edit
    step_type: skill
    skill: claude-code-edit
    inputs:
      description: "{{ inputs.description }}"
      affected_files: "{{ steps.analyze.outputs.affected_files }}"
    depends_on: [analyze]

  - id: evaluate
    step_type: skill
    skill: devbase-evaluate
    inputs:
      repo_id: "{{ inputs.repo_id }}"
    depends_on: [edit]

  - id: capture
    step_type: skill
    skill: devbase-session-capture
    inputs:
      context_id: "{{ env.DEVBASE_ACTIVE_CONTEXT }}"
      memory_type: "decision"
      content: "Refactored {{ inputs.symbol_name }}: {{ inputs.description }}"
    depends_on: [evaluate]
```

## 4. 实施路线

### P1: Project Brief + Impact Analysis（2 周）

- [ ] `devkit_project_brief` MCP tool + CLI command
- [ ] `devkit_impact_analysis` MCP tool（聚合 call_graph + related_symbols + tests）
- [ ] 更新 `docs/guides/claudecode-integration.md`

### P2: Session Wrapper + Auto-Capture（1 周）

- [ ] `devbase-claude` wrapper 脚本（POSIX + PowerShell）
- [ ] 会话结束后自动摘要提取（基于 git diff + oplog）
- [ ] `devkit_session_export` / `devkit_session_import`（Markdown 格式）

### P3: Workflow Templates（1 周）

- [ ] 预置 workflow YAML: `safe-refactor`, `code-review`, `release-prep`
- [ ] Workflow 模板注册到 devbase skill registry
- [ ] TUI workflow 模板选择器

## 5. 成功指标

| 指标 | 基线 | 目标 |
|------|------|------|
| Claude 启动理解时间 | 30-60s（大型仓库） | < 5s（Project Brief 注入） |
| 代码搜索轮次 | 5-10 次 grep | 2-3 次 hybrid search |
| 修改后回归 Bug | 频繁 | 降低 50%（Impact Analysis） |
| 跨会话知识保留 | 0% | 80%（Session Memory） |

## 6. 风险评估

| 风险 | 缓解 |
|------|------|
| ClaudeCode API 封闭，无法深度集成 | 先通过 wrapper 脚本 + MCP tools 外围集成；长期推动官方支持 |
| Project Brief 过大导致 token 爆炸 | 支持 `max_tokens` 限制 + 分层摘要（overview → modules → limits） |
| Impact Analysis 误报 | 结合 test coverage + manual review flag |
