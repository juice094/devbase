# devbase Roadmap

> **当前阶段**：阶段一 — 产品化闭环（v0.3.0 准备中）
> 
> **最后更新**：2026-04-25
> 
> **版本状态**：`0.4.0-alpha` 进行中 → 下一里程碑 `0.4.0`（AI Skill 编排基础设施）

---

## 阶段一：产品化闭环（v0.3.0）— ✅ 已完成

**核心原则：功能冻结。不修 bug、不补文档之外的一切代码。**

### 验收标准

| # | 标准 | 状态 |
|---|------|------|
| 1 | 34 MCP tools 全量通过 MCP Inspector | ✅ `cargo test --lib mcp` 14 passed |
| 2 | README Quick Start 三步内跑通 | ✅ 已走查，移除虚假声明 |
| 3 | 无"计划中"残留文档 | ✅ roadmap-2026.md 已归档 |
| 4 | CONTRIBUTING.md + ARCHITECTURE.md + AGENTS.md 闭环 | ✅ |
| 5 | Tests 全绿 + Clippy 零警告 | ✅ 239 passed / 0 failed / 3 ignored |
| 6 | GitHub Release 预编译二进制 | ✅ `devbase.exe` 22.6 MB 已上传 |

**Release**: https://github.com/juice094/devbase/releases/tag/v0.3.0

### 明确不做（Deferred）

| 功能 | 原因 | 预计阶段 |
|------|------|---------|
| SSE transport | 未实现，无 ETA | 阶段二或更晚 |
| 跨仓库搜索 (`/`) | TUI grep，新功能 | 阶段二 |
| Stars 趋势可视化 | 新功能，非阻塞 | 阶段二 |
| 自然语言查询 | 新功能，非阻塞 | 阶段二 |
| 智能同步建议 | 新功能，非阻塞 | 阶段二 |
| Skill 市场 / Registry 服务 | 需社区规模支撑 | 阶段二 |
| 跨设备注册表同步 | 依赖 syncthing-rust | 阶段二 |
| 架构拆分为多 crate | 22.7 KLOC 单 crate 仍最优 | 50+ tools 或编译 > 60s 时 |

### 明确不做（Deferred）

| 功能 | 原因 | 预计阶段 |
|------|------|---------|
| SSE transport | 未实现，无 ETA，阻塞发布 | 阶段二或更晚 |
| 跨仓库搜索 (`/`) | TUI grep，新功能 | 阶段二 |
| Stars 趋势可视化 | 新功能，非阻塞 | 阶段二 |
| 自然语言查询 | 新功能，非阻塞 | 阶段二 |
| 智能同步建议 | 新功能，非阻塞 | 阶段二 |
| Skill 市场 / Registry 服务 | 需社区规模支撑 | 阶段二 |
| 跨设备注册表同步 | 依赖 syncthing-rust | 阶段二 |
| 架构拆分为多 crate | 22.7 KLOC 单 crate 仍最优 | 50+ tools 或编译 > 60s 时 |

---

## 阶段二：AI Skill 编排基础设施（v0.4.0）— 进行中

**方向调整**：devbase 不是"个人外置大脑"，而是**将 GitHub 项目转换为 AI 可执行 Skill 的编排基础设施**。

> 50 个仓库不是给人浏览的参考库，而是 AI Skill 的原材料。devbase 的职责是：分析项目 CLI/API 表面 → 自动生成 SKILL.md → 注册到 Skill Registry → 让弱 AI 子代理能够发现、组合、执行。

### 核心文档

- [`docs/architecture/workflow-dsl.md`](architecture/workflow-dsl.md) — Workflow DSL 规范（v0.4.0-reserved）
- [`docs/architecture/workspace-as-schema.md`](architecture/workspace-as-schema.md) — 统一实体模型设计

### Phase 1 已完成 ✅

| 任务 | 状态 | 说明 |
|------|------|------|
| Schema v16 | ✅ | `entity_types` + `entities` + `relations` 统一模型，渐进双写 |
| Skill 自动封装 | ✅ | `devbase skill discover <path>` — Rust/Node/Python/Go/Docker/Generic 检测 |
| 分类体系 | ✅ | `ai`/`dev`/`data`/`infra`/`communication` 二级分类自动推断 |
| Workflow DSL 规范 | ✅ | YAML Schema 冻结，Engine 延后至 v0.5.0 |
| 评分预留 | ✅ | `success_rate`/`usage_count`/`rating` 字段入库，算法 v0.6.0 实现 |

### Phase 2 待完成（当前）

| 任务 | 交付物 | 优先级 |
|------|--------|--------|
| discover 非 dry-run | `devbase skill discover <path> --install` 真正注册 Skill | P0 |
| Git URL discover | `devbase skill discover https://github.com/...` 直接克隆+封装 | P0 |
| Skill 执行验证 | 将 devbase/zeroclaw 封装后 `skill run` 验证 entry_script | P0 |
| MCP discover tool | `devkit_skill_discover` 暴露给 AI Agent | P1 |
| Workflow Engine | v0.5.0：YAML 解析 + 拓扑排序 + 并行调度 | P2 |
| 评分算法 | v0.6.0：基于 execution audit 自动计算 success_rate/rating | P2 |

### 不做（明确排除）

- ❌ SSE transport（stdio 已足够）
- ❌ `.devbase` 目录规范（无外部采纳者）
- ❌ MCP 协议扩展提案（Star = 0，不会被采纳）
- ❌ 商业化 / 付费版
- ❌ 拆分 crate

---

## Future / Icebox

- 自然语言查询（TUI 内 `?` / `:` 模式）
- 智能同步建议（基于规则的 AI 辅助）
- 跨设备注册表同步（syncthing-rust 集成）
- 形式化验证 / TEE 集成
- L0-L4 五层知识模型 TOML Schema

---

*本 Roadmap 替代 `plans/roadmap-2026.md` 成为唯一活跃主路线图。*
*历史计划见 `docs/archive/`。*
