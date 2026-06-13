# devbase — Cognitive Anchor

> **Purpose**: This file is designed to survive context compression. It contains
> immutable facts and current state that every AI session must know before
> working on this project. If you are reading this after a context reset,
> treat this as your primary source of truth.

---

## Immutable Facts（不可变事实）

| ID | Fact | Source | Status |
|----|------|--------|--------|
| F-001 | Version | `Cargo.toml` | **v0.20.1** |
| F-002 | Edition | `Cargo.toml` | **Rust 2024** |
| F-003 | Test Coverage | CI | **605 passed, 0 failed, 7 ignored** |
| F-004 | Production Unwrap | Architecture Invariants | **0** (G5 rule enforced) |
| F-005 | MCP Tools | `src/mcp/mod.rs` | **71** (5 Stable / 58 Beta / 8 Experimental) |
| F-006 | Schema Version | `registry/migrate.rs` | **v36** |
| F-007 | Entities Table | Schema v21+ | **唯一真相源** (`repos` 表已删除) |
| F-008 | SQLite Mode | `storage.rs` | **WAL mode** |
| F-009 | Clippy | CI | **`-D warnings` 全绿** |
| F-010 | Release Assets | GitHub Releases | **Linux + Windows x64** 预编译二进制 |

## 架构红线（Architecture Guardrails）

- **RF-1**: 无裸 `init_db()` 调用，全部使用 `StorageBackend` 注入
- **RF-2**: `TempStorageBackend` 用于测试隔离（禁止 `DEVBASE_DATA_DIR` 竞态）
- **RF-3**: `entities` 表是唯一真相源
- **RF-4**: 二进制上下文 ≤ 1MB
- **RF-5**: 模块间无循环依赖
- **RF-6**: 生产代码零 `unwrap`/`expect`/`panic`（测试除外）
- **RF-7**: 路径输出必须脱敏（`sanitize_path()` 掩码 home 目录）

## 当前上下文（Current Context）

| 属性 | 值 |
|------|-----|
| 默认分支 | `main` |
| 最新 Release | `v0.20.1` (2026-05-17) |
| 当前 Phase | Phase 1 Production Hardening ✅ 完成 |
| 下一 Phase | Phase 12 — v0.21.0 "External Capability Grafting" |
| 活跃 PR | 无（PR #55 已合并） |

## 已知架构 Gaps（不可与 Immutable Facts 混淆）

这些是**待实现**的能力，不是 bug：

| Gap | 影响 | 计划版本 | 状态 |
|-----|------|----------|------|
| ~~`relations` 表零生产读取路径~~ | ~~统一实体模型的图遍历能力未暴露~~ | ~~v0.21.0~~ | **已完成** — `devkit_relation_store/query/delete` 已存在，`project_context` 已读取 |
| ~~Workflow 引擎零 MCP 暴露~~ | ~~AI 无法发现/触发工作流~~ | ~~v0.21.0~~ | **已完成** — `devkit_workflow_list/run/status` 已存在 |
| ~~`project_context` 不完整~~ | ~~缺少 relations/limits/skills/workflows~~ | ~~v0.21.0~~ | **已完成** — 已补充 `known_limits` + `skills` |
| MCP 工具测试覆盖不均 | 部分 Beta 工具仅有 smoke test | v0.21.0 | 待评估 |
| ~~`mcp/tools/repo.rs` 2376 行~~ | ~~维护负担~~ | ~~v0.21.0~~ | **已完成** — 已拆分为 `tools/` 目录 |
| ~~`init_db_at` 1214 行~~ | ~~迁移函数过大~~ | ~~v0.21.0~~ | **已完成** — 已拆分为 `registry/migrate.rs` + 子模块 |

## 防失忆校验清单（每次会话启动）

- [ ] 已读取本文件（`devbase/.claude/CLAUDE.md`）
- [ ] 已确认 `Cargo.toml` 版本与上表 F-001 一致
- [ ] 如果 handoff 文档说"未完成"，确认是新环境问题还是全局阻塞
- [ ] 如果修改 Schema，已更新 `registry/migrate.rs` 和 `SCHEMA_DDL`

## 快速入口

| 你想做什么 | 命令 |
|-----------|------|
| 运行测试 | `cargo test --all-targets` |
| 检查 clippy | `cargo clippy --all-targets -D warnings` |
| 检查格式化 | `cargo fmt --check` |
| 运行 invariant checks | `scripts/invariant-checks/run-checks.ps1` |
| 启动 MCP Server | `cargo run -- mcp` |
| 启动 TUI | `cargo run -- tui` |
| 扫描当前目录 | `devbase scan . --register` |
| 索引仓库 | `devbase index` |

## 关键文件映射

| 概念 | 文件 |
|------|------|
| 架构决策 | `docs/architecture/` |
| 稳定工具文档 | `docs/reference/stable-tools/` |
| 快速开始 | `docs/guides/quickstart.md` |
| MCP 集成指南 | `docs/guides/mcp-integration.md` |
| 变更日志 | `CHANGELOG.md` |
| Agent 简报 | `AGENTS.md` |
| 贡献指南 | `CONTRIBUTING.md` |

---

**Last Updated**: 2026-06-13
**Version**: v0.20.1
