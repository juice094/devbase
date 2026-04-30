# docs 目录重构计划 — v0.13.0

> 授权范围：较大范围整理。目标：以 "Local Context Compiler" 重新定义为核心线索，消除过时内容、合并重复文档、补齐缺失参考。

## 一、现状诊断

**39 个文件 / 8 个目录**，核心问题：

1. **分类边界模糊**：`research/` 与 `theory/` 重叠（竞争分析散落 4 个文件）；`plans/` 混杂"已实现的历史记录"与"未来计划"
2. **内容严重过时**：`archive/mcp_contract.md` 仅定义 4 个工具（实际 38 个）；`plans/roadmap-2026.md` 自标"严重过时"；`plans/skill-runtime.md` / `tui-skill-integration.md` 已完全实现
3. **缺失关键文档**：无 CLI 完整参考、无 MCP 工具清单、无 Schema v23 表结构参考、无 entities 模型说明
4. **导航失效**：`docs/README.md` 索引未纳入新增文件，状态标记（🟢/🔴/🟡）已失准

## 二、新目录结构

```
docs/
├── README.md                          ← 重写：统一导航 + 实时状态看板
├── ROADMAP.md                         ← 更新：v0.13.0 视角
│
├── _archive/                          ← archive/ 重命名，下划线 = 非活跃
│   ├── 0423-cross-project-meeting.md
│   ├── audit-index.md                 ← 从 research/AUDIT_INDEX.md 移入
│   ├── development-roadmap-0423.md
│   ├── execution-plan.md
│   ├── mcp-contract-v0.1.md           ← 重命名，标注历史版本
│   ├── smoke-test-report-20260418.md
│   ├── sprint-2-plan.md
│   ├── stage-report-2026-04-10.md
│   ├── user-testing-guide.md
│   ├── roadmap-2026.md                ← 从 plans/ 移入（已自标过时）
│   ├── skill-runtime.md               ← 从 plans/ 移入（已实现）
│   └── tui-skill-integration.md       ← 从 plans/ 移入（已实现）
│
├── architecture/                      ← 保留，核心架构文档
│   ├── README.md                      ← 新增：目录索引
│   ├── context-compiler.md            ← 新增：由 redefinition.md + workspace-as-schema.md 合并重写
│   ├── dependency-topology.md         ← 精简 dependency-topology-plan.md
│   └── workflow-dsl.md                ← 保留
│
├── guides/                            ← 扩展，面向用户的操作指南
│   ├── README.md                      ← 新增：目录索引
│   ├── quickstart.md                  ← 新增：5 分钟上手指南
│   ├── cli-reference.md               ← 新增：完整 CLI 子命令参考（含 vault/health/oplog 等）
│   ├── mcp-integration.md             ← 重写：合并 mcp-integration-guide.md + mcp-5ire-integration.md
│   ├── vault-format.md                ← 重命名 VAULT_FORMAT_SPEC.md
│   └── vault-workflow.md              ← 新增：PARA 目录结构实践指南
│
├── reference/                         ← 新增，技术参考（面向 Agent / 开发者）
│   ├── README.md                      ← 新增
│   ├── mcp-tools.md                   ← 新增：38 个工具完整清单（名称、 tier、描述、参数速查）
│   ├── schema-v23.md                  ← 新增：数据库 Schema v23（表、列、索引、迁移历史）
│   └── entities-model.md              ← 新增：统一实体模型（entities / relations / entity_types 详解）
│
├── ops/                               ← 保留，运维与审计
│   └── code-review-and-ops-plan.md
│
├── progress/                          ← 新增，从 ops/ 拆分，进度日志专用
│   └── progress-20260430.md           ← 从 ops/ 移入
│
└── research/                          ← 精简，只保留有长期价值的深度研究
    ├── ai-tool-context.md             ← 重命名 AI_TOOL_CONTEXT_RESEARCH.md
    ├── competitive-analysis.md        ← 合并 competitive-analysis.md + competitive_analysis_plan.md + competitive-risks.md + competitive-roadmap-table-a.md
    ├── memory-infrastructure.md       ← 重命名 memory_infrastructure_design.md
    └── three-repositories.md          ← 保留
```

## 三、删除/合并清单

| 操作 | 源文件 | 理由 |
|------|--------|------|
| **合并** | `redefinition.md` + `workspace-as-schema.md` → `architecture/context-compiler.md` | 同一主题（重新定义），合并消除重复 |
| **合并** | 4 份竞争分析 → `research/competitive-analysis.md` | 内容高度重叠，合并为 1 份精简版 |
| **合并** | `mcp-integration-guide.md` + `mcp-5ire-integration.md` → `guides/mcp-integration.md` | 5ire 指南是通用指南的子集，合并减少维护面 |
| **移动** | `plans/roadmap-2026.md` → `_archive/` | 自标"严重过时" |
| **移动** | `plans/skill-runtime.md` → `_archive/` | 已完全实现，历史记录 |
| **移动** | `plans/tui-skill-integration.md` → `_archive/` | 已完全实现，历史记录 |
| **移动** | `research/AUDIT_INDEX.md` → `_archive/` | 历史快照（2026-04-23） |
| **移动** | `ops/progress-20260430.md` → `progress/` | 分类归位 |
| **删除** | `plans/l0-l4-knowledge-model.md` | 部分实现，内容已分散到代码和已知限制系统 |
| **删除** | `plans/personal-knowledge-graph.md` | 规划已过时，entities/relations 已替代其目标 |
| **删除** | `plans/sse-daemon-design.md` | 明确 defer 到 v0.4.0+，无新信息 |
| **删除** | `research/architecture_audit_20260415.md` | 内容已被后续代码迭代覆盖 |
| **删除** | `research/referenced_repos_report.md` | 一次性数据提取报告，无长期价值 |
| **删除** | `research/unified-analysis.md` | 已被 `redefinition.md` / `context-compiler.md` 吸收 |
| **删除** | `testing/auditee-guide.md` | 内容已过时（基于 v0.10.0 的 known_limits 设计，现已迭代） |
| **删除** | `plan-v0.11.0-pool.md` | 已实施（r2d2_sqlite Pool 已落地） |

## 四、新增文档清单

| 文件 | 目标读者 | 内容范围 |
|------|----------|----------|
| `docs/README.md` | 人类 + Agent | 统一导航 + 实时状态看板（版本、测试、Schema、工具数） |
| `architecture/context-compiler.md` | 架构师 + Agent | "Local Context Compiler" 完整定义：六维信息模型、三缺口、W1-W6 路线图 |
| `guides/quickstart.md` | 新用户 | 安装 → 扫描 → 索引 → MCP 配置 → 第一条查询，5 分钟 |
| `guides/cli-reference.md` | 用户 + Agent | 所有 CLI 子命令完整列表、参数、示例 |
| `guides/vault-workflow.md` | 用户 | PARA 实践：Inbox → Projects → Areas → Resources → Archives |
| `reference/mcp-tools.md` | Agent + 开发者 | 38 个工具表格：名称、tier、一句话描述、关键参数 |
| `reference/schema-v23.md` | Agent + 开发者 | Schema v23 全部表结构、列定义、索引、迁移历史摘要 |
| `reference/entities-model.md` | Agent + 开发者 | entities / relations / entity_types 详解 + 查询模式 |

## 五、实施顺序

1. **Phase A：清理与移动**（文件系统操作）
   - 创建新目录（`_archive/`, `reference/`, `progress/`）
   - 移动/重命名文件
   - 删除确认过时的文件

2. **Phase B：重写核心文档**
   - `docs/README.md`（导航中枢）
   - `architecture/context-compiler.md`（重新定义）
   - `reference/mcp-tools.md` + `reference/schema-v23.md` + `reference/entities-model.md`

3. **Phase C：补齐用户指南**
   - `guides/quickstart.md`
   - `guides/cli-reference.md`
   - `guides/vault-workflow.md`

4. **Phase D：验证与收尾**
   - 检查所有内部链接
   - 更新 `ROADMAP.md`
   - 运行 `cargo test` 确认无破坏
   - git commit

## 六、不变更范围（Hard Veto）

- 不修改 `src/` 代码（文档纯文字工作）
- 不引入外部文档工具（MkDocs / Docusaurus 等）
- 不删除 `vault/` 目录中的任何内容（那是用户数据）
