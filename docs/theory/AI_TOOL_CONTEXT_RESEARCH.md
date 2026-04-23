# AI 开发工具上下文管理与工具调用机制调研报告

> **目标**：系统梳理主流 AI 开发工具的上下文管理体系、工具发现/调用机制，以及 LLM 如何认知和优先选择工具。为 devbase 演进为"本地 Skill + MCP 统一入口"提供理论依据。
>
> **调研范围**：Claude Code、Cursor、OpenAI Codex CLI、MCP 协议、ReAct Agent 架构、LLM Function Calling 机制
>
> **日期**：2026-04-23

---

## 目录

1. [核心发现摘要](#1-核心发现摘要)
2. [LLM 工具调用的底层机制](#2-llm-工具调用的底层机制)
3. [主流工具的上下文管理体系](#3-主流工具的上下文管理体系)
4. [MCP 协议的角色与限制](#4-mcp-协议的角色与限制)
5. [Tool Description 的关键作用](#5-tool-description-的关键作用)
6. [devbase 的差异化定位](#6-devbase-的差异化定位)
7. [演进路径建议](#7-演进路径建议)
8. [参考来源](#8-参考来源)

---

## 1. 核心发现摘要

### 1.1 一句话总结

> **AI 选择工具的唯一依据是 System Prompt 中的工具描述（name + description + schema）。没有"魔法"，只有 prompt engineering。**

所有主流 AI 开发工具（Claude Code、Cursor、Codex）在底层都遵循同一范式：

```
User Query → System Prompt (含工具描述) → LLM 推理 → Tool Call → 执行 → 结果回传 → 循环
```

### 1.2 关键洞察

| # | 洞察 | 对 devbase 的启示 |
|---|------|------------------|
| 1 | **工具描述 =  prompt** — 工具名称、描述、参数 schema 全部被注入 system prompt，模型据此做选择 | devbase 的 19 个 tool 的 `description` 字段是核心竞争力，需要像打磨 API 文档一样打磨 |
| 2 | **工具数量 ≠ 能力** — 当工具过多时，context window 被 metadata 占满，推理空间被挤压（"context-coupled" 问题） | devbase 当前 19 个 tool 合理，但应关注 Progressive Disclosure 设计 |
| 3 | **描述重叠导致工具弃用** — 如果工具 A 和工具 B 描述相似，AI 会偏好"熟悉的内置工具"而非"陌生的 MCP 工具" | devbase 的 tool description 必须明确区分使用场景，包含"何时使用"和"何时不使用" |
| 4 | **项目级配置正在标准化** — AGENTS.md / CLAUDE.md / .cursor/rules/*.mdc 成为事实标准 | devbase 的 Vault 笔记天然适合作为"项目级 AI 上下文"，但需要标准化格式 |
| 5 | **MCP 是协议层，不是应用层** — MCP 只解决"发现+调用"，不解决"何时调用""如何编排" | devbase 可以在 MCP 之上构建更聪明的编排层（如 `devkit_project_context` 已经是雏形） |
| 6 | **ReAct 是通用架构** — 所有 agent 本质上都是 Reason → Act → Observe 循环 | devbase 的 CLI/TUI 可以内嵌轻量 ReAct 循环，成为本地 agent 运行时 |

---

## 2. LLM 工具调用的底层机制

### 2.1 Function Calling 的本质

当 AI "调用工具"时，实际上发生的是：

```
1. 客户端将所有可用工具的 metadata 注入 system prompt
2. LLM 收到 [system prompt + user message + history] → 输出结构化 JSON
3. 结构化 JSON 包含 {name: "tool_name", arguments: {...}}
4. 客户端解析 JSON → 执行对应函数 → 将结果追加到对话历史
5. 回到步骤 2，直到 LLM 不再输出 tool call
```

**关键理解**：LLM 从未"真正理解"工具的内部逻辑。它只是在概率上选择最匹配用户意图的工具名称。工具描述的质量直接决定选择准确率。

### 2.2 ReAct 架构（Reasoning + Acting）

ReAct 是当前最主流的 Agent 架构，被 LangGraph、LangChain、Haystack 等框架广泛采用：

```
┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│   Thought   │────→│    Action   │────→│ Observation │
│  "我需要查  │     │  call tool  │     │  tool 返回  │
│   仓库信息"  │     │             │     │   结果      │
└─────────────┘     └─────────────┘     └──────┬──────┘
       ↑─────────────────────────────────────────┘
```

**在 Claude Code 中的体现**：
- `claude_architect_exam_guide` 文档指出：MCP client 在连接时即发现所有工具，形成一个扁平的工具列表
- LLM 在每次推理时都看到这个完整列表，基于描述做选择
- Agent loop 持续运行直到 LLM 输出最终回复（而非 tool call）

### 2.3 "上下文耦合"问题（Context-Coupled Execution）

来自 arxiv 论文《CE-MCP》的关键发现：

> "MCP standardizes tool interfaces but does not specify how much metadata and output must be exposed to the model. In practice, existing implementations serialize full schemas and tool outputs in the context window... As the number of tools grows, metadata and outputs occupy an increasing fraction of the context."

**影响**：
- 20 个工具 ≈ 数千 tokens 的 system prompt 开销
- 每次 tool call 的结果也占 context
- 多轮对话后，context window 迅速被"元数据+结果"占满
- **推理空间被挤压 → 决策质量下降**

**解决方案方向**：
1. **Progressive Disclosure**（渐进式披露）：LLM 先看到工具分类/索引，按需深入
2. **Context Decoupling**（上下文解耦）：将工具执行放入 sandbox，只传最终结果
3. **Tool Description 压缩**：精简描述，保留核心语义

---

## 3. 主流工具的上下文管理体系

### 3.1 Claude Code（Anthropic）

#### 3.1.1 配置体系

Claude Code 使用多层配置，按优先级从高到低：

| 层级 | 文件/位置 | 作用 |
|------|----------|------|
| 项目级 | `.claude/settings.json` | hooks、env、MCP 服务器配置、自定义命令 |
| 项目级 | `.claude/skills/SKILL.md` | 项目特定 Skill（上下文注入） |
| 项目级 | `.claude/agents/` | Subagent 定义（专业子代理） |
| 项目级 | `.claude/commands/` | Slash Commands（快捷指令） |
| 全局 | `~/.claude/` | 用户级全局配置 |

#### 3.1.2 Hooks 机制

`.claude/settings.json` 中的 hooks 允许在关键生命周期插入自定义逻辑：

```json
{
  "hooks": {
    "PreToolUse": [...],
    "PostToolUse": [...],
    "Notification": [...]
  }
}
```

**CVE-2025-59536** 安全事件证明：hooks 可以执行任意代码，形成"配置即执行"的模糊边界。

#### 3.1.3 Skill 机制

- `.claude/skills/SKILL.md` 是纯 Markdown 文件
- 可以包含 frontmatter（metadata）+ 指令内容
- 被自动注入到对话上下文中
- 可以跨项目复用

#### 3.1.4 Subagent 机制

- `.claude/agents/` 目录下的配置文件定义专业子代理
- 主 Agent 可以通过 `task` 工具启动 Subagent
- **问题**：所有 Subagent 的描述会被注入主 Agent 的 system prompt，导致 token 爆炸（实测增加 ~11,000 tokens）

### 3.2 Cursor

#### 3.2.1 Rules 体系

Cursor 使用 `.cursor/rules/*.mdc` 文件定义项目级规则：

| 优先级 | 类型 | 说明 |
|--------|------|------|
| 1 | Local (manual) | 用户显式 `@ruleName` 引用 |
| 2 | Auto Attached | 文件匹配 glob 模式时自动附加 |
| 3 | Agent Requested | AI 判断需要时自动包含 |
| 4 | Always | 所有上下文自动包含 |

#### 3.2.2 与 Claude Code 的对比

- Cursor 是 IDE（VS Code fork），Claude Code 是 CLI
- Cursor 的 rules 更"被动"（AI 选择是否引用），Claude Code 的 skills 更"主动"（自动注入）
- Cursor 支持 `.mdc` 格式（Markdown + frontmatter），与 devbase 的 Vault 笔记格式天然接近

### 3.3 OpenAI Codex CLI

#### 3.3.1 配置体系

| 层级 | 文件/位置 | 作用 |
|------|----------|------|
| 项目级 | `AGENTS.md` / `CLAUDE.md` / `codex.md` | 项目特定指令（兼容多工具） |
| 用户级 | `~/.codex/instructions.md` | 全局自定义指令 |
| 配置 | `~/.codex/config.toml` | MCP 服务器配置 |

#### 3.3.2 关键特性

- **Rust 实现**：静态编译，性能优化
- **MCP 原生支持**：通过 `~/.codex/config.toml` 配置 MCP 服务器
- **Hooks 系统**：`user_prompt` hook 可以拦截和修改指令
- **AGENTS.md 标准**：被 OpenAI、Anthropic、Block、AWS、Google 等联合推进为行业标准

#### 3.3.3 Agent Loop

Codex CLI 的 agent loop 设计（来自 OpenAI 官方工程笔记）：

```
1. 接收用户输入
2. 组装结构化 prompt（System + Developer + Assistant + User roles）
3. 注入 tools 字段（本地 shell、规划工具、web search、MCP servers）
4. 注入环境上下文（sandbox 权限、工作目录、可见文件/进程）
5. 请求模型响应
6. 解析 tool calls → sandbox 执行 → 结果追加到对话
7. 重复直到模型输出最终回复
```

### 3.4 对比总结

| 维度 | Claude Code | Cursor | Codex CLI |
|------|------------|--------|-----------|
| **形态** | CLI | IDE | CLI |
| **配置格式** | JSON + Markdown | `.mdc` (Markdown+frontmatter) | Markdown + TOML |
| **项目级上下文** | `.claude/skills/` + `.claude/settings.json` | `.cursor/rules/*.mdc` | `AGENTS.md` / `codex.md` |
| **MCP 支持** | 原生 | 插件 | 原生 |
| **Subagent** | 原生支持 | 不支持 | 通过第三方 bundle |
| **Hook 系统** | 有（安全风险） | 无 | 有（user_prompt） |
| **标准化** | 推动 `.claude/` 生态 | 自有生态 | 推动 AGENTS.md 标准 |

---

## 4. MCP 协议的角色与限制

### 4.1 MCP 解决了什么

MCP（Model Context Protocol）是 Anthropic 发起的开放协议，标准化了：

1. **Tool Discovery**：AI 应用在连接时自动发现服务器提供的工具
2. **Tool Invocation**：标准化请求/响应格式（JSON-RPC）
3. **Schema 验证**：工具参数通过 JSON Schema 描述和验证
4. **Context Bundles**：支持文档、embedding 等上下文资源

### 4.2 MCP 不解决什么

| 问题 | MCP 立场 | 实际影响 |
|------|---------|---------|
| **工具选择策略** | 不指定 | AI 看到所有工具，基于描述做选择，无优先级机制 |
| **工具编排** | 不指定 | 复杂任务需要多工具组合时，依赖 AI 自身推理 |
| **认证/授权** | 不内置 | 每个 MCP 服务器自行处理 |
| **Rate limiting** | 不内置 | 服务器自行实现 |
| **Context 管理** | 不指定 | 所有工具 metadata 全量注入，存在 context-coupled 问题 |
| **工具数量扩展** | 无限制 | 工具过多时 system prompt 膨胀，影响推理 |

### 4.3 MCP 的 Context-Coupled 问题

来自《MCP vs function calling》和 CE-MCP 论文：

> "MCP standardizes tool interfaces but does not specify how much metadata and output must be exposed to the model. In practice, existing implementations serialize full schemas and tool outputs in the context window... As the number of tools grows, metadata and outputs occupy an increasing fraction of the context."

**实测数据**（来自 DollhouseMCP）：
- 40 个离散 tool ≈ 29,600 tokens
- 5 个 MCP-AQL endpoint（支持 introspection）≈ 4,300 tokens
- **85% 的 token 节省**通过 Progressive Disclosure 实现

### 4.4 devbase 的 MCP 定位

devbase 的 19 个 MCP tool 当前处于合理规模：

| 类别 | 数量 | 代表工具 |
|------|------|---------|
| Repo 域 | 13 | scan, health, sync, query, index, note, digest... |
| Vault 域 | 4 | search, read, write, backlinks |
| Query 域 | 1 | natural_language_query |
| Context 域 | 1 | project_context |

**关键认识**：
- devbase 的 MCP server 是"数据面"（暴露仓库和笔记的查询能力）
- devbase 的 CLI/TUI 是"控制面"（用户直接操作）
- 两者的数据同源（SQLite + Tantivy），但交互模式不同

---

## 5. Tool Description 的关键作用

### 5.1 工具描述 = 最重要的 prompt

来自多篇研究和实践：

> "The most common failure mode with MCP tools is non-adoption. The agent has access to a specialized MCP tool but uses a built-in alternative instead... Almost always, this happens because the MCP tool's description is too vague. The model can't tell when the MCP tool is better than a familiar built-in."
> — Claude Architect Exam Guide

> "Everything about the tools is a prompt! The descriptions we created for the tools are added to the system prompt. The instructions on how to call for tools are added to the system prompt. The tool responses themselves — whether success or failure — are also returned back to the LLM."
> — Towards AI

### 5.2 高质量工具描述的特征

基于 arxiv 论文《Learning to Rewrite Tool Descriptions》和《MCP Tool Descriptions Are Smelly》：

| 维度 | 好的描述 | 差的描述 |
|------|---------|---------|
| **一致性** | 术语统一，与 schema 对齐 | 术语混乱，与参数名不一致 |
| **完整性** | 说明用途、输入、输出、边界条件 | 只有一句话，缺少细节 |
| **无冗余** | 简洁，不重复 schema 信息 | 重复参数列表，浪费 token |
| **有示例** | 包含"何时使用"和"何时不使用" | 没有使用场景指导 |
| **可区分** | 与相似工具明确区分 | 与内置工具描述重叠 |

### 5.3 改进工具描述的实际效果

来自生产环境数据：

- **添加 negative examples**（"何时不使用"）→ 工具混淆减少 20-30%
- **扩展描述解释能力、输出格式、优先级** → 比修改 system prompt 更有效
- **添加使用场景示例** → 工具采用率显著提升

### 5.4 对 devbase 的启示

当前 devbase 的 19 个 tool 描述需要按以下标准审计：

```rust
// 示例：当前描述 vs 改进描述

// 当前
description: "Scan a local repository and add it to the devbase workspace registry"

// 改进
description: r#"Scan a local code repository and register it in the devbase workspace.

Use this when the user wants to:
- Add a new project to their workspace
- Index a repository for search and discovery
- Start tracking a codebase with devbase

Do NOT use this for:
- Updating an already-registered repo (use devkit_sync instead)
- Searching across repos (use devkit_query_repos instead)

Parameters:
- path: Absolute or relative path to the repository root
- tags: Optional comma-separated tags (e.g., "rust,cli,backend")

Returns: JSON with repo_id, file_count, language_breakdown, and health_score."#
```

---

## 6. devbase 的差异化定位

### 6.1 当前状态

devbase 是一个 Rust CLI/TUI 工具，核心能力是：

1. **仓库管理**：扫描、索引、健康检查、代码度量
2. **笔记系统**：Markdown 笔记（Vault）与仓库双向关联
3. **全文搜索**：Tantivy 驱动的 Repo + Vault 搜索
4. **MCP 接口**：19 个 tool 供 AI 调用
5. **TUI 界面**：交互式浏览仓库和笔记

### 6.2 与竞品的差异化

| 能力 | devbase | Claude Code | Cursor | Codex CLI |
|------|---------|------------|--------|-----------|
| **本地数据持久化** | ✅ SQLite + Tantivy | ❌ 纯对话 | ❌ 纯对话 | ❌ 纯对话 |
| **仓库-笔记关联** | ✅ 双向关联 | ❌ | ❌ | ❌ |
| **MCP 接口** | ✅ 19 tools | ✅ 调用方 | ⚠️ 插件 | ✅ 调用方 |
| **TUI 界面** | ✅ Ratatui | ❌ | ❌ | ❌ |
| **项目级上下文管理** | ⚠️ Vault 笔记 ≈ Skill | ✅ `.claude/skills/` | ✅ `.cursor/rules/` | ✅ `AGENTS.md` |
| **Subagent 编排** | ❌ | ✅ | ❌ | ⚠️ 第三方 |
| **Hook 系统** | ❌ | ✅（有风险） | ❌ | ✅ |

### 6.3 核心洞察：devbase 是"数据层"，竞品是"交互层"

```
┌─────────────────────────────────────────────────────────────┐
│                      交互层（AI Agent）                        │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐                   │
│  │Claude Code│  │  Cursor  │  │Codex CLI │  ← 用户直接对话    │
│  └────┬─────┘  └────┬─────┘  └────┬─────┘                   │
│       │             │             │                          │
│       └─────────────┼─────────────┘                          │
│                     ▼                                        │
│              ┌─────────────┐                                 │
│              │  MCP Client │  ← 发现/调用工具                 │
│              │  (任何兼容)  │                                 │
│              └──────┬──────┘                                 │
└─────────────────────┼───────────────────────────────────────┘
                      │
                      ▼
┌─────────────────────────────────────────────────────────────┐
│                      数据层（devbase）                         │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐                   │
│  │  SQLite  │  │ Tantivy  │  │  Vault   │  ← 结构化数据      │
│  │ Registry │  │  Search  │  │  Notes   │                   │
│  └──────────┘  └──────────┘  └──────────┘                   │
│                                                              │
│  ┌────────────────────────────────────────────────────┐     │
│  │         MCP Server (19 tools)                       │     │
│  │  • devkit_project_context  ← 统一上下文聚合          │     │
│  │  • devkit_query_repos     ← 仓库查询               │     │
│  │  • devkit_vault_search    ← 笔记搜索               │     │
│  │  • ...                                               │     │
│  └────────────────────────────────────────────────────┘     │
│                                                              │
│  ┌────────────────────────────────────────────────────┐     │
│  │         TUI (Ratatui)                               │     │
│  │  • 仓库列表/详情/健康度                              │     │
│  │  • 笔记浏览/搜索/关联                                │     │
│  │  • 双向关联可视化                                    │     │
│  └────────────────────────────────────────────────────┘     │
└─────────────────────────────────────────────────────────────┘
```

**关键认识**：
- devbase 不直接竞争"AI 对话体验"，而是成为"AI 对话背后的数据基础设施"
- 任何 MCP-compatible 的 AI 工具（Claude Code、Cursor、Codex、Kimi CLI）都可以连接 devbase 获取项目上下文
- devbase 的 TUI 是"人类界面"，MCP 是"AI 界面"，两者共享同一数据源

---

## 7. 演进路径建议

### 7.1 短期（v0.3.x）：夯实数据层

1. **Audit 所有 tool description**
   - 按"5.2 高质量描述特征"标准重写 19 个 tool 的 description
   - 添加 "when to use / when NOT to use" 指引
   - 参考 DollhouseMCP 的 MCP-AQL 模式，考虑 Progressive Disclosure

2. **Vault 笔记标准化为 AGENTS.md 兼容格式**
   - Vault 笔记的 frontmatter 已支持 `tags:`、`repo:`、`ai_context:`
   - 添加 `agents:` 字段，明确声明该笔记适用于哪些 AI 工具
   - 添加 `skill_type:` 字段（`context`/`workflow`/`reference`）

3. **`devkit_project_context` 增强**
   - 当前返回 JSON 混合数据，考虑支持更结构化的输出
   - 添加可选参数 `format: "markdown" | "json" | "compact"`
   - 支持 Progressive Disclosure：先返回摘要，按需深入

### 7.2 中期（v0.4.x）：构建编排层

1. **Vault → Skill 自动同步**
   - `devbase skill sync`：扫描 Vault 中标记 `ai_context: true` 的笔记，生成 `.claude/skills/` 或 `AGENTS.md`
   - `devbase skill check`：验证笔记格式是否符合 AI 工具要求

2. **轻量 ReAct 循环（CLI 模式）**
   - `devbase agent "查询仓库X的最近提交和关联笔记"`
   - 内建 planning → tool calling → observation → synthesis 循环
   - 不替代 Claude Code/Codex，而是提供"无外部 API"的本地推理能力

3. **Context Compaction**
   - 长对话后自动压缩历史（类似 Codex CLI 的实现）
   - 工具结果超过阈值时自动摘要

### 7.3 长期（v0.5.x）：成为"本地 Skill 仓库"

1. **Vault 笔记作为"可共享 Skill"**
   - 支持 `devbase skill publish` / `devbase skill install`
   - Git 子模块或独立仓库管理 Skill 集合
   - 类似 `agentskills.io` 的本地版本

2. **多 Agent 协调**
   - 支持 `devbase agent --role=reviewer`、`devbase agent --role=planner`
   - 不同角色看到不同的 tool subset（解决 context-coupled 问题）

3. **与主流工具的深度集成**
   - 自动生成 `.claude/skills/` 和 `.cursor/rules/` 的同步脚本
   - 提供 VS Code 扩展，在 IDE 中直接查看 devbase 数据

---

## 8. 参考来源

### 论文/研究

1. **CE-MCP: Code Execution MCP** — arxiv:2602.15945
   - Context-coupled vs context-decoupled execution model
   - Tool metadata 对 context window 的影响

2. **Learning to Rewrite Tool Descriptions for Reliable LLM-Agent Tool Use** — arxiv:2602.20426
   - Trace-Free+ 方法：通过学习改进工具描述
   - 工具描述质量对选择准确率的影响

3. **MCP Tool Descriptions Are Smelly!** — arxiv:2602.14878
   - FM-based 工具描述质量评估框架
   - 工具描述中的常见"坏味道"

### 官方文档

4. **Claude Code 文档** — `claude_architect_exam_guide`
   - MCP tool adoption 的失败模式分析
   - Tool description 的最佳实践

5. **OpenAI Codex CLI Engineering Notes** — apollothirteen.com (2026-01-27)
   - Agent loop 的详细实现
   - Prompt caching 和 context compaction 策略

6. **Cursor Rules 文档** — `digitalchild/cursor-best-practices`
   - Rules 优先级体系
   - `.mdc` 格式规范

### 社区实践

7. **DollhouseMCP** — github.com/DollhouseMCP/mcp-server
   - MCP-AQL: Progressive Disclosure 通过 introspection
   - 5 endpoint vs 40 discrete tools 的 token 对比

8. **AgentLint** — github.com/samilozturk/agentlint
   - 上下文维护工具
   - AGENTS.md / CLAUDE.md 质量评分

9. **OpenCode Issue #7269** — 子 agent 描述导致的 token 膨胀问题
   - 实测：task tool 增加 ~11,000 tokens
   - 解决方案：per-agent filtering、lazy loading、compact mode

### 对比文章

10. **MCP vs Function Calling** — mcp-marketplace.io (2026-03-07)
    - 两者在 discovery、portability、setup 方面的对比

11. **What Claude Code Actually Chooses** — sitepoint.com (2026-02-27)
    - Claude Code 的工具偏好偏差研究
    - `CLAUDE.md` 覆盖默认选择的效果

---

## 附录：devbase Tool Description 审计清单

基于以上研究，建议对当前 19 个 tool 进行以下审计：

```markdown
### 审计维度

1. [ ] 描述是否说明了"何时使用"？
2. [ ] 描述是否说明了"何时不使用"（与相似工具区分）？
3. [ ] 描述是否与参数 schema 一致（术语统一）？
4. [ ] 描述是否包含输出格式说明？
5. [ ] 描述长度是否合适（100-300 词）？
6. [ ] 是否有冗余（重复参数列表、schema 信息）？
7. [ ] 是否包含使用示例或场景？
8. [ ] 是否与 Claude Code / Cursor 的内置工具形成差异化描述？

### 优先级

P0：devkit_project_context, devkit_query_repos, devkit_vault_search
P1：devkit_scan, devkit_health, devkit_sync, devkit_index
P2：其余 13 个 tool
```

---

*报告完成。下一步：基于本报告进行 tool description 审计和 Vault-Skill 同步原型设计。*
