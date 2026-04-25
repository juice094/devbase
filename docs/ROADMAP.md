# devbase Roadmap

> **当前阶段**：阶段一 — 产品化闭环（v0.3.0 准备中）
> 
> **最后更新**：2026-04-25
> 
> **版本状态**：`0.2.4` → 下一里程碑 `0.3.0`（首个产品化发布）

---

## 阶段一：产品化闭环（v0.3.0）

**核心原则：功能冻结。不修 bug、不补文档之外的一切代码。**

### 验收标准（全部达成方可打标 v0.3.0）

| # | 标准 | 状态 | 说明 |
|---|------|------|------|
| 1 | 34 MCP tools 全量通过 MCP Inspector | 🟡 待验证 | stdio 模式全覆盖；SSE 移出本阶段 |
| 2 | README Quick Start 三步内跑通 | 🟡 待验证 | `install` → `scan` → `tui` 无报错 |
| 3 | 无"计划中"残留文档 | 🟡 进行中 | 清理 roadmap-2026.md 等过期计划 |
| 4 | CONTRIBUTING.md + ARCHITECTURE.md + AGENTS.md 闭环 | ✅ 已完成 | 开发者 onboarding 已就位 |
| 5 | `cargo test --all-targets` 全绿 + `cargo clippy -D warnings` 零警告 | ✅ 已完成 | 239 passed / 0 failed / 3 ignored |
| 6 | 三平台 CI 通过（Windows/Linux/macOS） | 🟡 待确认 | 当前仅 Windows CI 活跃 |

### 阶段一剩余工作清单

**文档**
- [ ] 验证 34 tools 通过 MCP Inspector（stdio）
- [ ] README Quick Start 端到端走查（新机器、空 registry）
- [ ] 归档过期计划文件（`plans/roadmap-2026.md` 等）
- [ ] 撰写 v0.3.0 Release Notes

**Bugfix**
- [ ] 修复 `tokei` RUSTSEC-2020-0163 上游警告（或标记为 acceptable）
- [ ] 确认 Linux/macOS 编译无平台相关问题

**分发**
- [ ] GitHub Release 预编译二进制（Windows x64 / Linux x64 / macOS x64+ARM）
- [ ] `cargo install devbase` 发布到 crates.io（可选）
- [ ] 一键安装脚本（PowerShell / Bash）

**Wave 19 收尾（已实现）**
- [x] TUI Skill Panel（`k` 键进入 Skill 列表 → 详情 → 执行 → 结果）— commit `65bf15d`

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

## 阶段二：协议层跃迁（v0.4.0+）

**触发条件：v0.3.0 已发布且稳定运行。**

阶段二不是需要"外部许可"才能启动的宏大叙事，而是 **v0.3.0 发布后的自然下一步**。外部指标（Star 数、IDE 收录、付费意愿）**仅作为参考**，用于调整阶段二的具体优先级，而非准入门槛。

### 里程碑

| 优先级 | 里程碑 | 交付物 |
|--------|--------|--------|
| P0 | SSE transport | `run_sse()` + HTTP 流式传输 |
| P0 | `.devbase` 目录规范 v1.0 | 配置文件、缓存、索引的标准化目录结构 |
| P1 | IDE 集成申请 | Cursor / Claude Desktop / 5ire 的配置模板 + 申请提交 |
| P1 | MCP 协议扩展提案 | 针对"本地仓库上下文"的标准化接口提案 |
| P2 | 社区自治 | CONTRIBUTING.md 已就位；阶段二视反馈决定是否拆分核心 crate |
| P2 | 商业化验证 | 团队级知识库同步托管版 PoC |

### 降级方案

若 v0.3.0 发布后市场反馈冷淡：
- **阶段二收缩**为"细分场景深度优化"（TUI grep、Stars 趋势、性能优化）
- **不强行推进**协议标准权、社区扩张、商业化
- **保持节奏**：每 2-4 周一个小版本，持续打磨核心体验

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
