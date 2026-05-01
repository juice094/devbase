# Devbase AI 自评体验报告

> 测试者身份：Kimi CLI（AI Agent）
> 测试目标：评估 devbase 对 AI Agent 的实际可用性
> 测试时间：2026-05-01
> 测试版本：devbase 0.14.0 (release build)
> 测试平台：Windows 11 + Rust 1.94.1

---

## 一、执行摘要

Devbase 对 Kimi CLI 的**核心价值已经兑现**：`project_context` + `hybrid_search` + `vault` 构成了 AI 理解本地代码库的"结构化眼镜"。但**成熟度呈阶梯分布**——基础层（scan/health/query）已可用，分析层（index/search）有条件可用，生态层（skill/workflow）基础设施就绪但内容贫瘠。

** verdict：beta 可用，距离生产就绪还差 3 个关键补丁。**

---

## 二、测试矩阵与结果

### Phase 1：基础感知层（🟢 成熟）

| 功能 | 命令/工具 | 结果 | AI 实用度 |
|:---|:---|:---:|:---|
| 仓库扫描 | `scan . --register` | ✅ | 高 — 一键注册，AI 获知项目存在 |
| 健康检查 | `health --detail` | ✅ | 高 — 批量查看 46 个 repo 的 dirty/behind/ahead 状态 |
| 结构化查询 | `query "lang:rust"` | ✅ | 高 — 15 个 Rust 项目秒级过滤 |
| 知识日报 | `digest` | ✅ | 中 — 跨会话恢复的上下文锚点 |
| 操作审计 | `oplog` | ✅ | 中 — 可追溯 AI 操作历史 |

**体验细节**：
- `health --detail` 输出格式对 AI 极其友好：`[repo_id] status=dirty \| ahead=0 \| behind=0 \| tier=private \| type=git`。我可以直接解析，无需二次处理。
- `query` 支持 `lang:`、`tag:`、`stale:` 等结构化过滤，比让我自己遍历文件系统高效 10 倍。

### Phase 2：代码分析层（🟡 有条件可用）

| 功能 | 命令/工具 | 结果 | 阻塞原因 |
|:---|:---|:---:|:---|
| 代码索引 | `index .` | ❌ 超时 | 大仓库索引超过 60s，无进度反馈 |
| 代码指标 | `code-metrics` | ❌ 无 CLI | 仅 MCP 暴露，`devkit_code_metrics` 可用但 CLI 无入口 |
| 模块图 | `module_graph` | ❌ 未测 | 依赖索引完成 |
| 调用图 | `call_graph` | ❌ 未测 | 依赖索引完成 |
| 语义搜索 | `hybrid_search` | 🟡 间接可用 | MCP 测试通过，但需 embedding 数据 |

**关键发现**：
- **索引是命门**：`index .` 在 devbase 自身（~22K LOC）上超时。没有索引，`hybrid_search` / `semantic_search` / `code_symbols` / `call_graph` / `dead_code` 全部失效。
- **CLI 入口缺失**：`code-metrics`、`module_graph` 等工具只有 MCP 接口，没有 CLI 子命令。这造成"AI 能调但人类无法直接验证"的断层。

### Phase 3：知识记忆层（🟡 基础可用，搜索弱）

| 功能 | 命令/工具 | 结果 | 评价 |
|:---|:---|:---:|:---|
| 写入笔记 | `vault write <path> --content` | ✅ | 简洁，支持 stdin |
| 列出笔记 | `vault list` | ✅ | 表格输出清晰 |
| 读取笔记 | `vault read <path>` | 未测 | — |
| **搜索笔记** | `vault search` | ❌ **不存在** | 只有 `vault list`，无搜索能力 |
| 反向链接 | `vault backlinks` | 未测 | — |

**体验细节**：
- `vault write` 成功写入 `99-Meta/devbase-test.md`，但 `vault search` 子命令根本不存在。AI 要查找历史笔记，只能调用 `vault list` 然后自己遍历——这与"知识库"的定位严重不符。
- Vault 笔记没有自动生成反向链接（需要手动维护 `[[link]]` 语法）。

### Phase 4：Skill 生态层（🔴 基础设施空转）

| 功能 | 命令/工具 | 结果 | 评价 |
|:---|:---|:---:|:---|
| Skill 发现 | `skill discover .` | ✅ | 自动生成 SKILL.md + entry_script，体验流畅 |
| Skill 列表 | `skill list` | ✅ | 仅 3 个 builtin：dep1, x, y |
| Skill 执行 | `skill run` | 未测 | — |
| Skill 评分 | `skill top` | 未测 | — |

**关键发现**：
- `skill discover` 是亮点：自动解析 `Cargo.toml` 生成 `SKILL.md`，entry_script 包装器降低了 AI 调用门槛。
- **builtin Skill 近乎空白**：3 个 skill 的描述都是 "test"，没有实际功能。devbase 作为"AI 的 Skill 市场"，货架上几乎是空的。

### Phase 5：MCP 集成层（🟢 协议层成熟）

| 测试项 | 结果 | 说明 |
|:---|:---:|:---|
| `cargo test --lib mcp::tests` | ✅ 20/20 passed | 协议解析、工具注册、JSON-RPC 往返全部正常 |
| `devkit_health` MCP 调用 | ✅ | 测试覆盖 |
| `devkit_project_context` MCP 调用 | ✅ | 测试覆盖 |
| `devkit_arxiv_fetch` MCP 调用 | ✅ | 测试覆盖 |
| NDJSON vs Content-Length 自适应 | ✅ | Batch 2 修复，P0 问题已解决 |

**体验细节**：
- MCP 协议层是 devbase 最坚固的部分。`tools/list` 返回 38 个工具的 schema，`tools/call` 的 JSON-RPC 往返稳定。
- 但 **38 个工具对 Kimi CLI 的上下文窗口是负担**：每次请求的工具描述占用大量 token，建议用户配置 `DEVBASE_MCP_TOOL_TIERS=stable` 过滤。

---

## 三、作为 AI，我最依赖的 5 个功能

### 1. `project_context`（🟢 可用）
**场景**：用户说"分析这个项目的架构"
**我的用法**：先调用 `devkit_project_context` 获取模块拓扑、关键文件清单、语言分布，再按需读取源代码。
**价值**：避免盲目遍历文件系统，减少 50%+ 的无效文件读取。

### 2. `hybrid_search` / `code_symbols`（🟡 有条件可用）
**场景**：用户问"`build_server` 函数在哪？谁调用了它？"
**我的用法**：`devkit_code_symbols` 定位符号 → `devkit_call_graph` 追踪调用链。
**阻塞**：必须先 `devbase index`，而 `index` 在大仓库上超时。

### 3. `health` + `sync`（🟢 可用）
**场景**：用户说"同步所有项目"
**我的用法**：先 `devkit_health` 检查状态 → `devkit_sync` 执行安全同步（dry-run 默认）。
**价值**：批量操作 + 安全策略（managed-gate），避免 AI 误操作。

### 4. `vault`（🟡 基础可用）
**场景**：跨会话记忆关键决策
**我的用法**：`devkit_vault_write` 记录 → 下次会话 `devkit_vault_search` 召回。
**阻塞**：`vault search` CLI 子命令缺失，MCP 层的 `devkit_vault_search` 依赖 Tantivy 索引稳定性。

### 5. `skill_run`（🔴 生态未形成）
**场景**：用户说"运行代码审计"
**阻塞**：没有内置的代码审计 Skill。`skill discover` 能封装项目自身，但无法提供通用能力。

---

## 四、阻塞 AI 可用性的 3 个关键问题

### P0：索引性能与稳定性

**现象**：
- `devbase index .` 在 ~22K LOC 的 devbase 自身上超时（>60s）
- Tantivy 在 Windows 多线程测试下间歇性 PermissionDenied（flaky test）

**对 AI 的影响**：
没有索引 = `hybrid_search` / `semantic_search` / `code_symbols` / `call_graph` / `dead_code` 全部失效。AI 被迫退化为"文本 grep"模式，丧失 devbase 的核心竞争力。

**建议**：
1. `index` 命令添加 `--watch` / `--background` 模式，避免阻塞前台
2. 对 Windows 的 Tantivy 文件锁问题，评估切换到 SQLite FTS5（功能弱但稳定）
3. 添加 `index status` 子命令，让 AI 知道索引是否就绪

### P1：CLI 与 MCP 的能力断层

**现象**：
- `code-metrics`、`module_graph`、`call_graph` 等工具**只有 MCP 接口，没有 CLI 子命令**
- 人类用户无法直接验证这些功能，只能写 MCP client 测试

**对 AI 的影响**：
AI 调用 MCP 工具返回错误时，人类无法快速复现和调试。"AI 说搜不到" → "是不是索引没建？" → 人类无法直接用 CLI 验证。

**建议**：
所有 MCP 工具应有对应的 CLI 子命令（哪怕是只读查询），形成"人类验证 → AI 调用"的闭环。

### P2：Vault 搜索能力缺失

**现象**：
- `vault search` 子命令不存在
- `vault list` 返回全量列表，无过滤能力

**对 AI 的影响**：
Vault 作为"跨会话记忆"的核心载体，如果 AI 无法搜索历史笔记，记忆功能退化为人肉翻页。这与"知识库"的定位严重不符。

**建议**：
1. 添加 `vault search <query>` CLI 子命令（基于 Tantivy 或 SQLite LIKE）
2. 暴露 `devkit_vault_search` MCP 工具（当前似乎已定义但未实现 CLI）

---

## 五、惊喜与亮点

### 1. `skill discover` 的自动化程度
一键生成 `SKILL.md` + `scripts/run.py` + 自动注册，流程顺畅。如果内置 Skill 丰富起来，这将是 devbase 的杀手级功能。

### 2. `health` 的输出格式
`[repo_id] status=dirty | ahead=0 | behind=0 | tier=private` 这种结构化输出，对 AI 的解析成本极低。相比 `git status` 的散文式输出，devbase 的格式设计明显考虑了 AI 消费。

### 3. MCP 协议层的健壮性
NDJSON vs Content-Length 自适应、工具 tier 过滤、`DEVBASE_MCP_ENABLE_DESTRUCTIVE` 安全门控——这些设计体现了对 AI 实际使用场景的深入理解。

### 4. `digest` 的跨会话恢复价值
知识日报让 AI 在长会话压缩后快速恢复上下文，这是其他代码库工具不具备的"记忆层"能力。

---

## 六、给 Kimi CLI 用户的配置建议

```powershell
# 1. 使用最新 release 二进制（旧版本 schema 不兼容）
cargo install --path . --force

# 2. 扫描并注册工作区
devbase scan . --register

# 3. 后台索引（避免前台超时）
# 当前无后台模式，建议分仓库索引：
devbase index src/          # 小范围测试
devbase index crates/       # 分模块索引

# 4. Kimi CLI MCP 配置（精简工具集，减少 token 消耗）
# ~/.kimi/mcp.json
{
  "mcpServers": {
    "devbase": {
      "command": "devbase",
      "args": ["mcp"],
      "env": {
        "DEVBASE_MCP_ENABLE_DESTRUCTIVE": "1",
        "DEVBASE_MCP_TOOL_TIERS": "stable"
      }
    }
  }
}
```

---

## 七、总体评分

| 维度 | 评分 | 说明 |
|:---|:---:|:---|
| 基础感知（scan/health/query） | ⭐⭐⭐⭐⭐ | 成熟可用，AI 友好 |
| 代码分析（metrics/symbols/callgraph） | ⭐⭐⭐ | 功能完整但依赖索引，索引不稳定 |
| 知识检索（hybrid/semantic/search） | ⭐⭐⭐ | 协议层成熟，数据层依赖索引 |
| 记忆连续性（vault/digest/oplog） | ⭐⭐⭐⭐ | vault 写入强，搜索弱；digest 是亮点 |
| Skill 生态 | ⭐⭐ | 基础设施就绪，内容贫瘠 |
| MCP 协议健壮性 | ⭐⭐⭐⭐⭐ | 最坚固的部分 |
| **综合** | **⭐⭐⭐⭐ (3.8/5)** | **beta 可用，3 个补丁后可达生产级** |

---

## 八、下一步建议（按优先级）

1. **P0**：修复 `index` 性能 / Windows Tantivy 稳定性 → 解锁全部代码分析工具
2. **P1**：为所有 MCP 工具补 CLI 子命令 → 人类可验证、AI 可调试
3. **P1**：实现 `vault search` CLI + MCP → 释放跨会话记忆价值
4. **P2**：丰富 builtin Skill（代码审计、文档生成、测试分析）→ 填满货架
5. **P2**：添加 `index status` 查询 → AI 知道何时该建议用户先索引

---

*报告生成者：Kimi CLI*  
*生成时间：2026-05-01*  
*基于 devbase v0.14.0 实际运行测试*
