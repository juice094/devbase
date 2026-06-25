---
type: ConceptIndex
title: devbase 架构概念索引
description: 架构相关概念文档的入口，包含分层模型、依赖拓扑、不变量。
timestamp: 2026-06-25T11:15:50Z
tags: [architecture, index]
---

# devbase 架构概念索引

## 项目定位

devbase 是**本地优先的开发者工作空间数据库与知识库管理器**。它把代码仓库、PARA 笔记、Skill 与工作流编译成 AI 可推理的结构化情境。

## 架构文档

| 概念 | 文档 | 一句话说明 |
|------|------|------------|
| 三层架构 | [three-layer-model.md](./three-layer-model.md) | 交互层 → 编译层 → 可靠层 |
| 依赖拓扑 | [dependency-topology.md](./dependency-topology.md) | 11 个 Tier 的模块依赖关系与迭代策略 |
| 架构不变量 | [invariants.md](./invariants.md) | 不可打破的架构红线（G/T 体系） |
| 项目工作树 | [project-worktree.md](./project-worktree.md) | 按实际文件系统整理的模块与文件速查 |

## 关键设计原则

1. **依赖注入优于全局状态**：所有 IO 边界路径通过参数、`StorageBackend` 或 `AppContext` 注入。
2. **本地优先**：Registry DB 只存在用户本地配置目录，不向远程传输。
3. **客户端无关**：核心能力不硬编码特定客户端路径或 API。
4. **所有状态变更操作必须幂等**。
