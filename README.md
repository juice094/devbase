<div align="center">

# 🗄️ devbase

> **开发者工作空间的世界模型编译器**

一套引擎，统一代码上下文、知识记忆与智能体推理。

[![Version](https://img.shields.io/badge/version-v0.20.1-blue)](https://github.com/juice094/devbase/releases)
[![Tests](https://img.shields.io/badge/tests-605%2B%20passed-brightgreen)](https://github.com/juice094/devbase/actions)
[![Clippy](https://img.shields.io/badge/clippy-0%20warnings-green)](https://github.com/juice094/devbase/actions)
[![License](https://img.shields.io/badge/license-AGPL--3.0%20%2F%20Commercial-orange)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.95%2B-9cf)](https://www.rust-lang.org)
[![Glama](https://glama.ai/mcp/servers/juice094/devbase/badges/score.svg)](https://glama.ai/mcp/servers/juice094/devbase)

</div>

---

## 📋 简介

devbase 将代码库、笔记与工作流编译为 AI 可推理的结构化情境 — 不是存储数据，是构建环境的心智模型。

| 你是谁 | devbase 为你做什么 |
|:---|:---|
| **人类开发者** | `devbase tui` — 终端仪表盘，一眼看清 N 个仓库的 Git 状态，按 `s` 批量安全同步 |
| **AI 智能体** | 71 个 MCP 工具：通过 `devkit_skill_run` 发现、执行、编排 Skill — 不再重复造轮子 |
| **项目维护者** | `devbase skill discover .` — 一键将项目封装为 Skill，让 AI 用户能够发现和调用 |

---

## 🌟 核心亮点

| 亮点 | 说明 |
|:---|:---|
| 📊 **TUI 仪表盘** | ratatui 终端界面：跨仓库搜索、安全同步、Skill/Workflow 发现 |
| 🔌 **71 个 MCP 工具** | stdio 本地进程通信：仓库管理、代码分析、知识图谱、智能体记忆 |
| 🏠 **本地优先** | 零数据离开本机 — SQLite + Tantivy + tree-sitter，无需云端 |
| 🔍 **混合检索** | BM25 全文 + FTS5 技能搜索 + 纯 SQL 向量搜索（`cosine_similarity` UDF），零 ML 运行时依赖 |

> [完整 71 个 Tool 矩阵 → docs/reference/mcp-tools.md](docs/reference/mcp-tools.md)

---

## 🔧 技术栈

| 组件 | 技术 |
|:---|:---|
| 终端 UI | ratatui |
| 全文检索 | Tantivy (BM25) |
| 语义检索 | SQLite BLOB + `cosine_similarity` UDF |
| 代码解析 | tree-sitter (Rust/Python/TS/Go) |
| 关系存储 | SQLite (WAL 模式, OpLog 审计) |
| 协议 | Model Context Protocol (stdio) |

---

## 📁 项目结构

```
devbase/
├── src/
│   ├── main.rs          # CLI 入口：命令解析与分发（RF-4 ≤ 1000 行）
│   ├── lib.rs           # 导出 30+ 模块
│   ├── commands/        # CLI 子命令实现
│   ├── tui/             # 终端仪表盘（ratatui）
│   ├── mcp/             # MCP Server（71 个工具，stdio 通信）
│   ├── registry/        # SQLite Registry：schema、迁移、实体、关系
│   ├── search/          # Tantivy BM25 + 向量混合检索
│   ├── vault/           # PARA 笔记系统：双向链接、BFS 图遍历
│   ├── skill_runtime/   # Skill 生命周期：发现 → 安装 → 执行 → 评分 → 发布
│   ├── workflow/        # YAML 编排引擎：5 种 step 类型
│   ├── knowledge_engine/# 代码符号提取与语义索引
│   └── sync/            # 仓库同步编排与策略
├── crates/              # 12 个独立 workspace crate
│   ├── devbase-core-types
│   ├── devbase-registry
│   ├── devbase-embedding
│   ├── devbase-vault-wikilink
│   └── ...
├── docs/                # 完整文档导航：docs/README.md
├── scripts/             # 安装脚本与 CI 辅助
├── skills/              # 示例 Skill
└── README.md
```

### 核心设计

**三层架构**：
1. **交互层** — TUI 仪表盘 + MCP Server + Workflow 引擎（人类与 AI 的接口）
2. **编译层** — 感知（tree-sitter/Tantivy/Git）→ 知识（图谱/向量/关系）→ 策略（同步/工作流/健康守卫）
3. **可靠层** — SQLite WAL 并发安全 + 索引健康检测 + OpLog 全操作审计

> 可靠性红线：所有 Registry 写入必须留下不可变审计痕迹（OpLog）；Schema 迁移前自动生成快照。详见 [docs/architecture/overview.md](docs/architecture/overview.md)。

---

## 🚀 快速开始

```powershell
# Windows 一行安装
irm https://raw.githubusercontent.com/juice094/devbase/main/scripts/install.ps1 | iex

# 或下载预编译二进制（~8.7 MB）
# https://github.com/juice094/devbase/releases/tag/v0.20.1
```

```bash
# Linux / macOS
curl -fsSL https://raw.githubusercontent.com/juice094/devbase/main/scripts/install.sh | bash

# 基础工作流
devbase scan . --register   # 1. 扫描并注册工作区
devbase tui                # 2. 打开仪表盘
devbase mcp                # 3. 启动 MCP 服务端（供 AI 调用）
```

**AI 助手配置** — 添加到 `claude_desktop_config.json` 或 `~/.kimi/mcp.json`：
```json
{ "mcpServers": { "devbase": { "command": "devbase", "args": ["mcp"] } } }
```

---

## 🤝 参与贡献

详见 [CONTRIBUTING.md](CONTRIBUTING.md) — 添加 MCP 工具、Skill Schema、构建模式说明。快速验证：

```bash
cargo build --release
cargo test --all-targets
cargo clippy --all-targets -D warnings
```

---

## 📄 许可证

双许可证：[AGPL-3.0+](LICENSE) 开源 / [商业授权](LICENSE-COMMERCIAL.md) 闭源使用。联系：`juice094@protonmail.com`。

---

<div align="center">

**[⭐ Star](https://github.com/juice094/devbase) · [🐛 Issues](https://github.com/juice094/devbase/issues) · [🤝 Contribute](CONTRIBUTING.md)**

</div>
