<div align="center">

# 🗄️ devbase

> **World Model Compiler for Developer Workspaces**

One engine for code context, knowledge memories, and agent reasoning.

[![Version](https://img.shields.io/badge/version-v0.20.1-blue)](https://github.com/juice094/devbase/releases)
[![Tests](https://img.shields.io/badge/tests-494%2B%20passed-brightgreen)](https://github.com/juice094/devbase/actions)
[![Clippy](https://img.shields.io/badge/clippy-0%20warnings-green)](https://github.com/juice094/devbase/actions)
[![License](https://img.shields.io/badge/license-AGPL--3.0%20%2F%20Commercial-orange)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.95%2B-9cf)](https://www.rust-lang.org)

</div>

---

## 📋 简介
devbase compiles your codebases, notes, and workflows into AI-reasonable structured context — not storing data, but building your environment's mental model.

| 你是谁 | devbase 为你做什么 |
|:---|:---|
| **人类开发者** | `devbase tui` — terminal dashboard for N-repo Git status, press `s` for safe sync |
| **AI Agent** | 69 MCP tools: discover, run, and orchestrate Skills via `devkit_skill_run` |
| **项目维护者** | `devbase skill discover .` — one-click project-to-Skill packaging |

---

## 🌟 核心亮点

| 亮点 | 说明 |
|:---|:---|
| 📊 **TUI 仪表盘** | ratatui-based terminal UI: cross-repo search, safe sync, Skill/Workflow discovery |
| 🔌 **69 MCP Tools** | stdio-based local MCP server: repo mgmt, code analysis, knowledge graph, agent memory |
| 🏠 **本地优先** | Zero data leaves your machine — SQLite + Tantivy + tree-sitter, no cloud required |
| 🔍 **混合检索** | BM25 full-text + cosine vector search in pure SQL, zero ML runtime deps |

> [完整 69 个 Tool 矩阵 → docs/guides/mcp-integration-guide.md](docs/guides/mcp-integration-guide.md)

---

## 🔧 技术栈

| 组件 | 技术 |
|:---|:---|
| 终端 UI | ratatui |
| 全文检索 | Tantivy (BM25) |
| 语义检索 | SQLite BLOB + `cosine_similarity` UDF |
| 代码解析 | tree-sitter (Rust/Python/TS/Go) |
| 关系存储 | SQLite (WAL mode, OpLog audit) |
| 协议 | Model Context Protocol (stdio) |

---

## 📁 项目结构

```
devbase/
├── src/
│   ├── tui/          # Terminal dashboard
│   ├── mcp/          # MCP Server (69 tools)
│   ├── registry/     # Repo registry & health check
│   ├── index/        # Tantivy + SQLite vector index
│   ├── vault/        # PARA notes & bidirectional links
│   ├── skill/        # Skill lifecycle (discover→run→score)
│   └── workflow/     # YAML orchestration engine
├── docs/             # Architecture & guides
└── scripts/          # install.ps1 / install.sh
```

---

## 🚀 快速开始

```powershell
# Windows one-liner
irm https://raw.githubusercontent.com/juice094/devbase/main/scripts/install.ps1 | iex

# Or grab pre-built binary (~8.7 MB)
# https://github.com/juice094/devbase/releases/tag/v0.20.1
```

```bash
# Linux / macOS
curl -fsSL https://raw.githubusercontent.com/juice094/devbase/main/scripts/install.sh | bash

# Basic workflow
devbase scan . --register   # 1. Scan workspace
devbase tui                # 2. Open dashboard
devbase mcp                # 3. Start MCP server for AI
```

**AI 助手配置** — add to `claude_desktop_config.json` or `~/.kimi/mcp.json`:
```json
{ "mcpServers": { "devbase": { "command": "devbase", "args": ["mcp"] } } }
```

---

## 🤝 参与贡献

See [CONTRIBUTING.md](CONTRIBUTING.md) for adding MCP tools, Skill schemas, and build modes. Quick validation:

```bash
cargo build --release
cargo test --all-targets
cargo clippy --all-targets -D warnings
```

---

## 📄 License

Dual License: [AGPL-3.0+](LICENSE) for open source / [Commercial](LICENSE-COMMERCIAL.md) for proprietary use. Contact: `juice094@protonmail.com`.

---

<div align="center">

**[⭐ Star](https://github.com/juice094/devbase) · [🐛 Issues](https://github.com/juice094/devbase/issues) · [🤝 Contribute](CONTRIBUTING.md)**

</div>
