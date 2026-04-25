# Skill Runtime 开发计划

> **目标**：让 devbase 从"知识基底"升级为"能力 OS"，提供 Skill 的存储、注册、发现、执行全生命周期管理。
>
> **理论基础**：AKU Registry (arXiv:2603.14805)、OmniRoute Skills System、LM-Kit.NET SkillRegistry、AGNTCY ADS、Semantic Kernel PluginCollection、BMO Agent Skill System。
>
> **版本**：v0.1 draft — 基于成熟工程理论的架构设计，未进入实现阶段。

---

## 0. 设计哲学

### 0.1 不重新发明轮子

| 领域 | 已有标准 | devbase 策略 |
|---|---|---|
| Skill 文件格式 | Kimi CLI / Clarity / BMO Agent 的 `SKILL.md` (Markdown + YAML frontmatter) | **直接兼容**，不做自定义格式 |
| 执行协议 | Python/Bash/可执行文件 | **Process-based**，不内置 Wasm/容器 |
| 发现机制 | Embedding 语义搜索 (LM-Kit.NET `FindMatchesWithEmbeddings`) | **复用已有 embedding 基础设施** |
| 工具注册 | MCP + OpenAPI + LangChain (ToolRegistry) | **Protocol-agnostic adapter** |

### 0.2 存储统一

Skill 的元数据存在 devbase 的 SQLite `skills` 表中，与 `code_symbols`、`code_embeddings`、`code_call_graph` 共享同一个知识基底。

这带来独特价值：**cross-layer 查询**。例如：
> "找到所有与'代码审计'相关的 skill，然后审计这个仓库的 Rust 代码"

### 0.3 正交性

Skill Runtime 遵循三仓库架构的正交原则：
- Skill 的业务逻辑变化 **不影响** MCP 协议格式
- 替换底层执行引擎（Python → Node.js）**不影响** Skill 注册表
- 新增 GitHub 仓库 **自动继承** 所有已安装的 Skill

---

## 1. 核心概念

### 1.1 Skill 定义

一个 Skill 是可复用的 AI 能力单元，包含：

```
skill-name/
├── SKILL.md          # 元数据 + 指令（必需）
│   ├── YAML frontmatter: name, description, version, tags, inputs, outputs
│   └── Markdown body: 执行指令、工作流、示例
├── scripts/          # 可执行脚本（可选）
│   ├── run.py        # 主入口
│   └── validate.sh   # 校验脚本
└── references/       # 参考文档（可选）
```

**YAML frontmatter 规范**（与 Kimi CLI / Clarity 兼容）：

```yaml
---
name: code-audit
version: "1.0.0"
description: Audit a Rust codebase for common issues using devbase knowledge
author: devbase-team
tags: [rust, audit, security]
inputs:
  - name: repo_id
    type: string
    description: Target repository ID in devbase
    required: true
  - name: severity
    type: string
    description: Minimum severity level
    default: "warning"
outputs:
  - name: report
    type: markdown
    description: Audit report in markdown
---
```

### 1.2 Skill 生命周期

```
Install → Register → Discover → Activate → Execute → Uninstall
```

| 阶段 | 操作 | 对应命令 / API |
|---|---|---|
| **Install** | 从 Git URL / 本地路径复制 Skill 文件到 `~/.local/share/devbase/skills/` | `devbase skill install <url>` |
| **Register** | 解析 SKILL.md，写入 SQLite `skills` 表 | 自动（install 后） |
| **Discover** | 按标签/语义搜索匹配用户意图的 Skill | `devbase skill search <query>` |
| **Activate** | 将 Skill 上下文注入 AI system prompt | MCP `devkit_skill_run` |
| **Execute** | 调用 Skill 的 entry script，传入参数 | `devbase skill run <name> --args` |
| **Uninstall** | 从文件系统和注册表中移除 | `devbase skill uninstall <name>` |

### 1.3 内置 Skill vs 自定义 Skill

**内置 Skill**（随 devbase 分发）：
- `embed-repo` — 调用 `local.py` 生成 embeddings
- `search-workspace` — 封装 hybrid + cross-repo + related symbols
- `knowledge-report` — 封装 `generate_report`
- `code-audit` — 组合上述工具做代码审计

**自定义 Skill**（用户安装）：
- 从 GitHub URL 安装：`devbase skill install https://github.com/.../my-skill.git`
- 从本地路径安装：`devbase skill install ./my-skill/`
- 支持版本管理和依赖声明（未来）

---

## 2. 数据库 Schema

### 2.1 `skills` 表（Schema v14）

```sql
CREATE TABLE skills (
    id              TEXT PRIMARY KEY,           -- skill 标识符（kebab-case）
    name            TEXT NOT NULL,              -- 显示名称
    version         TEXT NOT NULL,              -- SemVer
    description     TEXT NOT NULL,              -- 一句话描述
    author          TEXT,                       -- 作者
    tags            TEXT,                       -- JSON array: ["rust", "audit"]
    entry_script    TEXT,                       -- 入口脚本路径（相对 skill 目录）
    skill_type      TEXT NOT NULL DEFAULT 'custom', -- builtin | custom | system
    local_path      TEXT NOT NULL,              -- 本地绝对路径
    inputs_schema   TEXT,                       -- JSON Schema for inputs
    outputs_schema  TEXT,                       -- JSON Schema for outputs
    embedding       BLOB,                       -- f32 BLOB（语义搜索用）
    installed_at    TEXT NOT NULL,              -- ISO 8601
    updated_at      TEXT NOT NULL,              -- ISO 8601
    last_used_at    TEXT                        -- ISO 8601
);

CREATE INDEX idx_skills_tags ON skills(tags);
CREATE INDEX idx_skills_type ON skills(skill_type);
CREATE VIRTUAL TABLE skills_fts USING fts5(name, description, tags);
```

### 2.2 `skill_executions` 表（审计追踪）

```sql
CREATE TABLE skill_executions (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    skill_id        TEXT NOT NULL REFERENCES skills(id),
    args            TEXT,                       -- JSON: 传入参数
    status          TEXT NOT NULL,              -- pending | running | success | failed | timeout
    stdout          TEXT,                       -- 标准输出
    stderr          TEXT,                       -- 标准错误
    exit_code       INTEGER,
    started_at      TEXT NOT NULL,
    finished_at    TEXT,
    duration_ms     INTEGER
);
```

---

## 3. CLI 接口设计

### 3.1 命令矩阵

```
devbase skill
├── list          [--type <builtin|custom|all>] [--json]
├── install       <git-url|local-path> [--version <ver>]
├── uninstall     <skill-id>
├── search        <query> [--semantic] [--limit <n>]
├── run           <skill-id> [--arg key=value] [--timeout <sec>]
├── info          <skill-id>
├── validate      <skill-id>              # 校验 SKILL.md 格式
├── publish       [--dry-run]             # 打包为 git tag + release
└── sync          --target <clarity|kimi> # 同步到外部 skill 系统
```

### 3.2 使用示例

```bash
# 列出所有内置 skill
devbase skill list --type builtin

# 从 GitHub 安装社区 skill
devbase skill install https://github.com/juice094/devbase-skills/tree/main/code-audit

# 语义搜索与"代码审计"相关的 skill
devbase skill search "audit rust code for security issues"

# 运行 skill，传入参数
devbase skill run code-audit --arg repo_id=devbase --arg severity=error

# 验证本地 skill 格式
devbase skill validate ./my-custom-skill/
```

---

## 4. 执行引擎设计

### 4.1 架构

```
┌─────────────────────────────────────────┐
│  SkillExecutor                          │
│  ├── load_skill(skill_id) → SkillMeta   │
│  ├── resolve_interpreter(entry_script)  │
│  │   ├── .py  → python                  │
│  │   ├── .sh  → bash (Git Bash on Win)  │
│  │   ├── .ps1 → powershell              │
│  │   └── binary → direct exec           │
│  ├── spawn_process(args, env, timeout)  │
│  ├── capture_stdout_stderr()            │
│  └── return ExecutionResult             │
└─────────────────────────────────────────┘
              │
              ▼
┌─────────────────────────────────────────┐
│  Sandbox (轻量级隔离)                    │
│  ├── timeout: 30s default               │
│  ├── cwd: skill 目录                     │
│  ├── env: 注入 devbase 相关变量          │
│  │   ├── DEVBASE_REGISTRY_PATH          │
│  │   ├── DEVBASE_SKILL_ID               │
│  │   └── DEVBASE_HOME                   │
│  └── 禁止: 网络访问（未来可配置）         │
└─────────────────────────────────────────┘
```

### 4.2 与现有工具的关系

Skill 的 entry script 可以直接调用 devbase 的现有能力：

```python
#!/usr/bin/env python3
# skills/embed-repo/scripts/run.py
import subprocess
import sys

repo_id = sys.argv[1]
subprocess.run([
    "python", "tools/embedding-provider/local.py",
    "--repo-id", repo_id,
    "--device", "auto"
], check=True)
```

或者通过 MCP 调用（更松耦合）：

```python
# 通过 devbase MCP stdio server 调用工具
from mcp import ClientSession, StdioServerParameters

params = StdioServerParameters(command="devbase", args=["mcp", "--transport", "stdio"])
# ... 调用 devkit_hybrid_search 等工具
```

---

## 5. MCP 集成

### 5.1 新增 MCP Tools

| Tool | 功能 | 状态 |
|---|---|---|
| `devkit_skill_list` | 列出所有可用 skill（含元数据） | 规划中 |
| `devkit_skill_search` | 语义搜索 skill | 规划中 |
| `devkit_skill_run` | 执行指定 skill，传入参数 | 规划中 |
| `devkit_skill_install` | 从 URL 安装 skill | 规划中 |

### 5.2 AI 调用示例

```json
// AI 通过 MCP 调用 skill
{
  "name": "devkit_skill_run",
  "arguments": {
    "skill_id": "code-audit",
    "args": {
      "repo_id": "devbase",
      "severity": "warning"
    }
  }
}

// 返回
{
  "status": "success",
  "stdout": "## Audit Report\n...",
  "stderr": "",
  "exit_code": 0,
  "duration_ms": 1250
}
```

---

## 6. 分阶段实现计划

### Wave 16a — Schema & Storage ✅

**目标**：建立 Skill 的存储层和 CLI 框架。

- [x] `src/skill_runtime/schema.rs` — `skills` / `skill_executions` 表定义
- [x] `src/skill_runtime/registry.rs` — CRUD：install / list / uninstall / get
- [x] `src/skill_runtime/parser.rs` — 解析 SKILL.md（YAML frontmatter + Markdown body）
- [x] `src/cli/skill.rs` — CLI 子命令框架：`devbase skill list`
- [x] SQLite migration（Schema v14）
- [x] 3 个内置 skill 模板：`embed-repo`、`search-workspace`、`knowledge-report`

**验收标准**：
- ✅ `devbase skill list --type builtin` 输出 3 个内置 skill
- ✅ `devbase skill install ./skills/embed-repo/` 成功写入 SQLite
- ✅ `devbase skill info embed-repo` 显示完整元数据

### Wave 16b — Discovery & Search ✅（text search done, semantic pending embeddings）

**目标**：让 AI 能发现正确的 skill。

- [x] `devbase skill search <query>` — 基于 LIKE 的文本搜索
- [ ] `devbase skill search <query> --semantic` — 基于 embedding 的语义搜索（pending batch embeddings）
- [ ] Skill embedding 生成（复用 `local.py` 为 SKILL.md 的 description 生成向量）
- [x] `devbase skill validate <path>` — 校验 SKILL.md 格式合规性

**验收标准**：
- ✅ `devbase skill search "audit"` 返回 `code-audit`
- `devbase skill search "find bugs" --semantic` 返回 `code-audit`（语义匹配）
- ✅ `devbase skill validate` 能检测出格式错误的 SKILL.md

### Wave 17 — Execution Engine ✅

**目标**：让 skill 能真正执行。

- [x] `src/skill_runtime/executor.rs` — Process-based 执行引擎
- [x] `devbase skill run <id> --arg key=value` — CLI 执行入口
- [x] Sandbox：timeout、stdout/stderr capture、exit code handling
- [x] `skill_executions` 表自动记录每次执行
- [x] 内置 skill 实现：
  - `embed-repo`：调用 `local.py`
  - `search-workspace`：封装 keyword search workflow
  - `knowledge-report`：封装 registry metrics report

**验收标准**：
- ✅ `devbase skill run knowledge-report --arg repo_id=devbase` 成功生成报告
- ✅ 执行结果被记录到 `skill_executions` 表
- ✅ 超时 skill 被自动 kill（timeout 参数已支持）

### Wave 18 — MCP Integration ✅

**目标**：让 AI 通过 MCP 调用 skill。

- [x] `DevkitSkillListTool` — MCP tool #32
- [x] `DevkitSkillSearchTool` — MCP tool #33
- [x] `DevkitSkillRunTool` — MCP tool #34
- [x] 更新 `ai_first_user.rs`：验证 Skill Runtime 注册状态

**验收标准**：
- ✅ AI 能调用 `devkit_skill_run` 执行 skill（通过 MCP stdio）
- ✅ `ai_first_user.rs` 新增 skill 调用验证步骤（3 builtin skills registered）

### Wave 19 — Ecosystem（进行中）

**目标**：skill 可分发、可同步。

- [x] `devbase skill install <git-url>` — 从 GitHub 安装（auto-detect http/https/git@）
- [ ] `devbase skill publish` — 打包并推送到 git
- [ ] `devbase skill sync --target clarity` — 同步到 Clarity skill 系统
- [ ] Skill 依赖管理（skill A 依赖 skill B）
- [ ] Skill 市场 / registry 服务

---

## 7. 与现有模块的关系

| 现有模块 | Skill Runtime 如何使用 |
|---|---|
| `registry` | Skill 与 repo 关联：`repo:` frontmatter 字段 → `repos` 表外键 |
| `search/hybrid` | Skill 语义搜索复用 embedding 基础设施 |
| `semantic_index` | 为 SKILL.md 的 description 生成 embedding |
| `mcp/` | 新增 3 个 MCP tools 暴露 skill 能力 |
| `vault/` | Vault 笔记可标记 `skill_type: true` 自动转为 skill |
| `tools/embedding-provider/` | 内置 skill `embed-repo` 的底层实现 |

---

## 8. 风险与缓解

| 风险 | 缓解措施 |
|---|---|
| Skill 格式与 Kimi CLI / Clarity 不兼容 | 严格遵循现有 SKILL.md 规范，不自定义字段 |
| 执行引擎安全（恶意 skill） | Phase 1 不做网络 sandbox，但限制执行时间 + 只读 registry 路径 |
| 嵌入式 skill 过多导致 context 爆炸 | Skill 分级（builtin/custom），AI 默认只加载 builtin |
| 与现有 31 个 MCP tools 的功能重叠 | Skill = 组合编排，MCP tool = 原子操作，边界清晰 |

---

## 9. 参考文献

1. **Agent Knowledge Architecture** — arXiv:2603.14805v1, 2026. AKU Registry + Knowledge Topology.
2. **OmniRoute Skills System** — GitHub: `diegosouzapw/OmniRoute`. SQLite-backed registry + executor + sandbox.
3. **LM-Kit.NET SkillRegistry** — `lm-kit.com`. Semantic matching, hot reload, remote loading.
4. **AGNTCY Agent Directory Service** — arXiv:2509.18787v1. Schema + Indexing + Storage + Distribution layers.
5. **Semantic Kernel PluginCollection** — Microsoft. Kernel as DI container for plugins.
6. **BMO Agent** — GitHub: `joelhans/bmo-agent`. Markdown+YAML skill format.
7. **ToolRegistry** — arXiv:2507.10593v1. Protocol-agnostic tool unification.
8. **devbase Three-Repositories Architecture** — `docs/theory/three-repositories.md`.
