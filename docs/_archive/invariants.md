> **⚠️ 文档迁移提示**：本文件为历史不变量清单。最新、已维护的架构红线与不变量已按 OKF（Open Knowledge Format）整理至 `.knowledge/architecture/invariants.md`，采用全局不变量 G1–G7 + 分层不变量 T01–T12 体系。本文件保留历史上下文，但规则编号、范围与检测方式可能已过时。最新数据：v0.20.1 / Schema v36 / 71 MCP tools / 12 workspace crates。

# 架构不变量清单（Invariants）

> 来源：架构治理方法论参考（Kimi 会话 `e9f2965f-b949-46a5-9d7c-afd6d4d9232c`）
> 原则：不可打破的规则列表，每次代码审查对照检查。

---

## 全局不变量

| # | 规则 | 违反后果 | 检测方式 |
|---|------|---------|---------|
| G1 | `registry::WorkspaceRegistry` 不得依赖任何 Tier 4+ 模块 | 数据层被查询层污染，Schema 变更引发级联修改 | `cargo check` + 依赖拓扑审查 |
| G2 | `i18n` / `config` 不得包含业务逻辑 | 基础配置层膨胀，语言文件与业务耦合 | 代码审查：只含静态字符串和结构体定义 |
| G3 | 所有状态变更 MCP tool 必须幂等 | 重复调用导致数据损坏 | 单元测试：同一参数调用两次结果一致 |
| G4 | Breaking change 只能通过新增 tool 实现，不修改现有 schema | 下游 Agent 契约破裂 | Schema 版本对比 + MCP tool 清单审计 |
| G5 | 生产代码不得新增 `unwrap()` / `expect()`（RF-6） | 运行时 panic | `cargo clippy` + 人工审查 |

## 分层不变量

### Tier 0–1（原子基础层）

| # | 规则 | 说明 |
|---|------|------|
| T01 | `core` 只定义无业务语义的枚举和结构体 | NodeType / Node / Edge 不得出现 devbase 专属逻辑 |
| T02 | `registry` Schema 变更必须经过三步：migration → 备份 → oplog_analytics 兼容性检查 | 见 `dependency-topology.md` §二、Tier 1 |
| T03 | `embedding` 必须是纯函数工具包，无副作用 | 禁止在 encode 中写文件、改全局状态 |

### Tier 2–3（扫描与分析层）

| # | 规则 | 说明 |
|---|------|------|
| T04 | `scan` 新增语言支持不得改动 `semantic_index` 公共 API | 语言检测规则可独立实验 |
| T05 | `symbol_links` 的阈值和算法可独立调优，不破坏下游 | Jaccard 阈值默认 0.3，可调 |

### Tier 4（查询层）

| # | 规则 | 说明 |
|---|------|------|
| T06 | `query` 表达式解析必须向后兼容 | `lang:rust` 语法不得删除，只能扩展 |
| T07 | `search/hybrid` RRF 权重可独立调优，不影响工具 schema | 向量/BM25 融合策略是内部实现细节 |

### Tier 5（同步层）

| # | 规则 | 说明 |
|---|------|------|
| T08 | 新增 sync 策略必须先定义于 `sync/policy`，再实现于 `sync/tasks` | 禁止直接在 orchestrator 中硬编码策略逻辑 |

### Tier 6–7（Skill / Workflow 层）

| # | 规则 | 说明 |
|---|------|------|
| T09 | `skill_runtime::executor` 必须自包含副作用描述 | 每个 Skill 的 entry_script 必须声明读写范围 |
| T10 | Workflow 新增 `StepType` 只需改动 `workflow/model` → `parser` → `executor`，不影响 Skill Runtime | 见 `dependency-topology.md` §二、Tier 7 |

### Tier 9–10（MCP / TUI 层）

| # | 规则 | 说明 |
|---|------|------|
| T11 | `mcp/tools/*` 不得直接调用 `rusqlite::Connection`，必须通过 `registry` 封装 | 防止 SQL 注入和 Schema 漂移 |
| T12 | `tui/render/*` 是纯消费者层，新增面板不改动任何下层逻辑 | TUI 状态机只读取，不写入 registry |

## 模块提取演习检查表

> 每季度执行一次。任何模块若无法在半天内提取为独立包并写出 50 字 README，说明耦合过重。

| 模块 | 上次检查 | 能否提取 | README 50 字验证 |
|------|---------|---------|-----------------|
| `devbase-embedding` | 2026-04 | ✅ 已提取 | 本地 BERT embedding 生成，零外部依赖 |
| `devbase-registry-health` | 2026-04 | ✅ 已提取 | 批量健康查询，消除 N+1 |
| `skill_runtime` | 2026-04 | ⚠️ 待验证 | parser/registry/executor 边界清晰，但 clarity_sync 耦合待审 |
| `workflow` | 2026-04 | ⚠️ 待验证 | model/parser/scheduler 可拆，executor 依赖 skill_runtime::executor |

---

*违反任何不变量 → 必须在 ADR 中记录例外理由，否则回滚。*
