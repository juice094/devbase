---
type: ChangeLog
title: devbase Knowledge Bundle 变更日志
description: 记录 .knowledge/ OKF bundle 的结构变更与数据更新。
timestamp: 2026-06-25T11:15:50Z
tags: [meta, okf, changelog]
---

# devbase Knowledge Bundle 变更日志

## 2026-06-25 — OKF Bundle 初始化

- 类型：`refactor`
- 范围：`.knowledge/`、`AGENTS.md`、`docs/architecture/`

### 变更内容

1. 在项目根创建 `.knowledge/` OKF bundle，将原本散落在 `AGENTS.md`、`docs/architecture/overview.md`、`docs/architecture/dependency-topology.md`、`docs/architecture/invariants.md` 中的架构/拓扑/规范知识拆分为独立概念文档。
2. 重写 `AGENTS.md` 为 OKF 入口索引，顶部添加 YAML frontmatter，正文仅保留核心红线和指向本 bundle 的链接。
3. 修复多处数据不一致：
   - MCP Tools：统一为 **71** 个（以 `src/mcp/mod.rs` `McpToolEnum` 为准）
   - Registry Schema：统一为 **v36**（以 `src/registry/migrate.rs` 为准）
   - 项目版本：统一为 **v0.20.1**
   - `main.rs` 行数：统一为 **836** 行
4. 统一架构红线编号为 **G/T 体系**（全局不变量 G1–G7 + 分层不变量 T01–T12），替代原 AGENTS.md 的 RF-1–RF-7。

### 新增概念文档

- `.knowledge/index.md`
- `.knowledge/architecture/index.md`
- `.knowledge/architecture/three-layer-model.md`
- `.knowledge/architecture/dependency-topology.md`
- `.knowledge/architecture/invariants.md`
- `.knowledge/registry/index.md`
- `.knowledge/registry/schema.md`
- `.knowledge/registry/migration-policy.md`
- `.knowledge/mcp/index.md`
- `.knowledge/mcp/tool-adding-guide.md`
- `.knowledge/development/index.md`
- `.knowledge/development/build-and-test.md`
- `.knowledge/development/code-style.md`
- `.knowledge/log.md`

### 向后兼容说明

- `AGENTS.md` 仍保留在项目根，Kimi / Claude 等 Agent 的既有读取路径不受影响。
- `docs/architecture/` 中的原始文档保留，但关键数据已更新；未来逐步将深度内容迁移至 `.knowledge/`。

## 2026-06-25 — AGENTS.md 与 OKF bundle 进一步整理

- 类型：`docs`
- 范围：`AGENTS.md`、`.knowledge/architecture/project-worktree.md`、`docs/README.md`、`docs/architecture/invariants.md`

### 变更内容

1. 在 `AGENTS.md` 末尾新增“完整项目结构参考”，将项目 worktree 直接放入 Agent 入口文件，降低新 Agent 探索成本。
2. 按 OKF 标准新增 `.knowledge/architecture/project-worktree.md`，作为项目工作树的权威归档版本，frontmatter 类型为 `ArchitectureTopology`。
3. 同步更新 `AGENTS.md` 的“模块地图”，增加“关键文件”列。
4. 修复 `docs/architecture/invariants.md` 顶部缺失的迁移提示，指向 `.knowledge/architecture/invariants.md`。
5. 更新 `docs/README.md`：
   - `main.rs` 行数：833 → 836
   - Agent 架构/不变量/拓扑链接：从 `docs/architecture/*` 指向 `.knowledge/architecture/*`
   - 文档目录中明确标注历史架构文档已迁移，并新增 OKF 权威入口链接。
6. 统一 AGENTS.md / CLAUDE.md / `.knowledge/index.md` 的红线编号映射（G1–G7 ↔ RF-1–RF-7）。

## 2026-06-25 — 归档历史架构文档

- 类型：`docs`
- 范围：`docs/architecture/` → `docs/_archive/`，`docs/README.md`、`SUPPORT.md`、`CONTRIBUTING.md`、`README.md`、`docs/guides/ai-instance-handoff.md`、`docs/reference/entities-model.md`、`docs/_audit/six-dimension-gap-analysis-20260430.md`

### 变更内容

1. 将以下 4 份历史架构/拓扑/不变量文档从 `docs/architecture/` 归档至 `docs/_archive/`：
   - `overview.md` → `_archive/overview.md`
   - `context-compiler.md` → `_archive/context-compiler.md`
   - `dependency-topology.md` → `_archive/dependency-topology.md`
   - `invariants.md` → `_archive/invariants.md`
2. 更新 `docs/README.md`：
   - 从“架构设计”表格移除上述 4 份历史文档。
   - 在“归档”表格注册上述 4 份文档，并标注迁移目标。
3. 更新所有指向旧路径的活跃链接：
   - `SUPPORT.md`：Architecture / Architecture Guardrails → `.knowledge/architecture/`
   - `CONTRIBUTING.md`：架构参考 → `.knowledge/architecture/`
   - `README.md`：可靠性红线 → `.knowledge/architecture/invariants.md`
   - `docs/guides/ai-instance-handoff.md`：架构图/拓扑 → `.knowledge/architecture/`
   - `docs/reference/entities-model.md`：情境编译器 → `.knowledge/architecture/`
   - `docs/_audit/six-dimension-gap-analysis-20260430.md`：标注 context-compiler.md 已归档
4. `docs/architecture/` 下保留继续维护的技术性文档：ADR、Workspace 拆分计划、Workflow DSL 规范等。

### 关键数字

- MCP Tools：**71**
- Registry Schema：**v36**
- 项目版本：**v0.20.1**
- `main.rs` 行数：**836**
- 测试函数：**616+**
- Workspace Crates：**12**
