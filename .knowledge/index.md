---
type: KnowledgeBundleIndex
title: devbase Knowledge Bundle
description: AI Agent 的 devbase 项目认知入口，包含架构、Registry、MCP、开发规范等概念文档。
version: 0.20.1
schema_version: 36
mcp_tools: 71
crates: 12
tests: 616+
timestamp: 2026-06-25T11:15:50Z
tags: [agent-instruction, devbase, architecture, onboarding]
---

# devbase Knowledge Bundle

> **面向 AI Agent 的项目认知包**。本 bundle 用 [Open Knowledge Format (OKF)](https://www.gitbook.com/blog/what-is-okf-open-knowledge-format) 组织：每个 `.md` 文件描述一个概念，文件之间用 Markdown 链接关联。

## 项目速览

- **名称**：devbase
- **版本**：0.20.1
- **定位**：开发者工作空间的世界模型编译器
- **核心能力**：把本地代码库、PARA 笔记、Skill 脚本编译为 AI 可推理的结构化情境
- **交互接口**：CLI / TUI / MCP Server（stdio，71 个 tools）

## 快速入口

| 我想了解 | 读这个 |
|----------|--------|
| 项目整体架构 | [architecture/index.md](./architecture/index.md) |
| 三层架构（交互/编译/可靠） | [architecture/three-layer-model.md](./architecture/three-layer-model.md) |
| 模块依赖拓扑（11-Tier） | [architecture/dependency-topology.md](./architecture/dependency-topology.md) |
| 架构红线与不变量 | [architecture/invariants.md](./architecture/invariants.md) |
| Registry Schema 与迁移 | [registry/index.md](./registry/index.md) |
| MCP Tool 添加路径 | [mcp/tool-adding-guide.md](./mcp/tool-adding-guide.md) |
| 构建、测试、提交规范 | [development/index.md](./development/index.md) |
| 本 bundle 的变更历史 | [log.md](./log.md) |

## 关键数字（由代码实际状态维护）

| 指标 | 数值 | 验证来源 |
|------|------|----------|
| MCP Tools | 71 | `src/mcp/mod.rs` `McpToolEnum` |
| Workspace Crates | 12 | `crates/` 目录 |
| 测试函数 | 616+ | `cargo test --workspace -- --list` |
| Schema 版本 | v36 | `src/registry/migrate.rs` `CURRENT_SCHEMA_VERSION` |
| 主入口行数 | 836 行 | `src/main.rs` |
| 生产 unwrap | 0 | 架构红线 G5 |

## 核心红线（违反任何一条 → 必须 halt）

1. **依赖注入优于全局状态**（G1 / RF-1）：禁止新增 `dirs::data_local_dir()` / `std::env::var_os` 硬编码路径。
2. **测试密封性**（G2 / RF-2）：测试禁止修改全局进程状态，文件系统测试用 `tempfile` + `StorageBackend`。
3. **Schema 单一事实来源**（G3 / RF-3）：`SCHEMA_DDL` 与 `migrate.rs` 必须同步。
4. **二进制入口限界**（G4 / RF-4）：`main.rs` 不得超过 1000 行。
5. **生产代码无 panic**（G5 / RF-6）：禁止 `unwrap()` / `expect()` / `panic!()`。
6. **无循环依赖**（G6 / RF-5）：禁止模块间双向 `use crate::` 引用。
7. **Workspace 拆分约束**（G7 / RF-7）：新增模块若对 devbase 内部其他模块的 `crate::` 引用超过 5 个，禁止提取为 workspace crate。

## 给 Agent 的使用建议

1. **先读本 bundle，再查代码**：复杂任务先通过 [dependency-topology.md](./architecture/dependency-topology.md) 定位层级，再深入具体模块。
2. **改 Schema 前先读 Registry 文档**：任何 `registry.db` 表结构变更必须遵循 [migration-policy.md](./registry/migration-policy.md)。
3. **添加 MCP Tool 走标准路径**：见 [tool-adding-guide.md](./mcp/tool-adding-guide.md)。
4. **保持 AGENTS.md 入口简短**：人类/Agent 首次进入项目时，`AGENTS.md` 会指向本 bundle。
