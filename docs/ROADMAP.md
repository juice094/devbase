# devbase Roadmap

> **当前阶段**：阶段一 — 产品化闭环（v0.3.0 准备中）
> 
> **最后更新**：2026-04-25
> 
> **版本状态**：`0.3.0` 已发布 → 下一里程碑 `0.4.0`（个人知识库跃迁）

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

## 阶段二：个人知识库跃迁（v0.4.0）— 进行中

**方向调整**：放弃"协议标准权/商业化"的宏大叙事，转向**"个人外置大脑"**——解决你自己的真实痛点。

> 你有 50 个 AI 项目参考库，但 devbase 只能回答 `lang:rust`，无法回答"和 clarity 相似的项目有哪些"。

### 核心文档

[`docs/plans/personal-knowledge-graph.md`](plans/personal-knowledge-graph.md) — 完整设计。

### 波次规划

| 波次 | 主题 | 交付物 | 预计 |
|------|------|--------|------|
| Wave 21 | Repo 画像 | Schema v16 + README/Cargo.toml 解析器 + `devbase profile` | 1 天 |
| Wave 22 | 相似度计算 | `repo_embeddings` + `devbase similar` + `devbase stack` | 1 天 |
| Wave 23 | 对比与笔记 | `devbase compare` + `devbase why` + `.devbase/notes.md` | 1 天 |
| Wave 24 | TUI 知识面板 | DetailTab::Knowledge + NL 查询 | 1-2 天 |

### 核心 CLI

```bash
devbase similar clarity        # 相似仓库排序（zeroclaw 预计第一）
devbase compare clarity zeroclaw  # 技术栈对比报告
devbase why zeroclaw           # 显示"为什么 clone"笔记
devbase stack ratatui          # 使用 ratatui 的所有仓库
devbase query "rust llm provider" # 自然语言查询
```

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
