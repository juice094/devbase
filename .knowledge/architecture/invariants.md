---
type: Invariants
title: devbase 架构不变量清单
description: 不可打破的架构规则。违反任意一条必须 halt，转交人类裁决或回滚。
timestamp: 2026-06-25T11:15:50Z
tags: [architecture, invariants, guardrails]
---

# devbase 架构不变量清单

> 原则：不可打破的规则列表，每次代码审查对照检查。

---

## 全局不变量（G1–G7）

| # | 规则 | 违反后果 | 检测方式 |
|---|------|---------|---------|
| G1 | 依赖注入优于全局状态：禁止新增 `dirs::data_local_dir()` / `std::env::var_os` 硬编码路径 | 数据层被查询层污染，路径逻辑散落 | `grep -rn "dirs::data_local_dir\|std::env::var_os" src/` |
| G2 | 测试密封性：测试禁止修改全局进程状态，文件系统测试用 `tempfile` + `StorageBackend` | 测试间互相污染， flaky | `cargo test --test-threads=16` |
| G3 | Schema 单一事实来源：`SCHEMA_DDL` 与 `migrate.rs` 必须原子同步 | 测试与生产 schema 不一致 | `test_in_memory_schema_version` |
| G4 | 二进制入口限界：`main.rs` 不得超过 1000 行 | CLI 逻辑膨胀，难以维护 | 行数检查 |
| G5 | 生产代码无 panic：禁止 `unwrap()` / `expect()` / `panic!()` | 运行时崩溃 | `cargo clippy` + 人工审查 |
| G6 | 无循环依赖：禁止模块间双向 `use crate::` 引用 | 编译/理解复杂度爆炸 | `cargo check` + 依赖图审查 |
| G7 | Workspace 拆分约束：新增模块若对 devbase 内部其他模块的 `crate::` 引用超过 5 个，禁止提取为 workspace crate | 子 crate 反向耦合主 crate | 代码审查 |

---

## 分层不变量（T01–T12）

### Tier 0–1（原子基础层）

| # | 规则 | 说明 |
|---|------|------|
| T01 | `core` 只定义无业务语义的枚举和结构体 | `NodeType` / `Node` / `Edge` 不得出现 devbase 专属逻辑 |
| T02 | `registry` Schema 变更必须经过三步：migration → 备份 → 兼容性检查 | 见 [registry/migration-policy.md](../registry/migration-policy.md) |
| T03 | `embedding` 必须是纯函数工具包，无副作用 | 禁止在 `encode` 中写文件、改全局状态 |

### Tier 2–3（扫描与分析层）

| # | 规则 | 说明 |
|---|------|------|
| T04 | `scan` 新增语言支持不得改动 `semantic_index` 公共 API | 语言检测规则可独立实验 |
| T05 | `symbol_links` 的阈值和算法可独立调优，不破坏下游 | Jaccard 阈值默认 0.3，可调 |

### Tier 4（查询层）

| # | 规则 | 说明 |
|---|------|------|
| T06 | `query` 表达式解析必须向后兼容 | `lang:rust` 语法不得删除，只能扩展 |
| T07 | `search/hybrid` RRF 权重可独立调优，不影响 tool schema | 向量/BM25 融合策略是内部实现细节 |

### Tier 5（同步层）

| # | 规则 | 说明 |
|---|------|------|
| T08 | 新增 sync 策略必须先定义于 `sync/policy`，再实现于 `sync/tasks` | 禁止直接在 orchestrator 中硬编码策略逻辑 |

### Tier 6–7（Skill / Workflow 层）

| # | 规则 | 说明 |
|---|------|------|
| T09 | `skill_runtime::executor` 必须自包含副作用描述 | 每个 Skill 的 entry_script 必须声明读写范围 |
| T10 | Workflow 新增 `StepType` 只需改动 `workflow/model` → `parser` → `executor`，不影响 Skill Runtime | 见 [dependency-topology.md](./dependency-topology.md) §Tier 7 |

### Tier 9–10（MCP / TUI 层）

| # | 规则 | 说明 |
|---|------|------|
| T11 | `mcp/tools/*` 不得直接调用 `rusqlite::Connection`，必须通过 `registry` 封装 | 已知例外：`repo.rs`、`repo/nl_query.rs`、`brief.rs`、`impact.rs` |
| T12 | `tui/render/*` 是纯消费者层，禁止写入 registry | TUI 状态机只读取，不写入 |

---

## 历史说明

- 本文件继承并统一了原 `AGENTS.md` 中的 RF-1 ~ RF-7 与 `docs/architecture/invariants.md` 的 G/T 表。
- 为减少 Agent 认知负担，统一采用 **G/T 编号体系**：G 表示全局（Global），T 表示分层（Tier）。
