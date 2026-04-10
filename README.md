# devbase

> 开发者的本地代码库知识管理系统 —— AI 生态的"消化系统"。

[![Rust](https://img.shields.io/badge/rust-2024%20edition-orange.svg)](https://www.rust-lang.org/)
[![Tests](https://img.shields.io/badge/tests-25%20passed-brightgreen.svg)]()

`devbase` 是你本地代码仓库群的统一入口。它将散落在各处的 Git 仓库转化为**可查询、可同步、可分析的结构化知识库**，并通过 [MCP (Model Context Protocol)](https://modelcontextprotocol.io/) 让 AI Agent 直接感知你的开发环境。

---

## ✨ 核心特性

- **🔍 扫描与注册**：`devbase scan` 递归发现 `.git` 仓库，自动识别语言（Rust/Node/Go/Python/C++）并持久化到本地数据库，支持 ZIP 快照来源识别。
- **❤️ 健康检查**：`devbase health` 检测每个仓库的 dirty/behind/ahead 状态，同时检查本地工具链版本（rustc/cargo/node/go/cmake）。
- **🔄 智能同步**：`devbase sync` 按标签和策略批量同步仓库，支持并发编排、错误分类、dry-run 预览。
- **🔎 结构化查询**：`devbase query` 支持 `lang:rust`、`stale:>30`、`behind:>10`、`tag:own-project` 等表达式。
- **🤖 MCP 服务器**：`devbase mcp --transport stdio` 暴露 7 个工具，供 Claude Desktop、Cursor、Cline 等 Agent 调用。
- **📊 交互式 TUI**：`devbase tui` 提供键盘驱动的实时界面，后台操作不阻塞 UI；批量同步时底部状态栏实时显示进度，弹窗区分完成/运行中/等待状态。
- **📝 知识日报**：`devbase digest` 生成 24 小时知识代谢报告，聚合新发现、异常仓库、项目关系。
- **⚙️ 后台守护进程**：`devbase daemon` 定时执行 health → re-index → discovery → digest 自动化闭环。
- **🌐 中英文自动适配**：首次启动自动检测系统语言，界面支持中文/英文无缝切换，配置持久化。

---

## 🚀 快速开始

```bash
# 编译并安装
cargo build --release

# 扫描并注册当前目录下的所有 Git 仓库
./target/release/devbase scan . --register

# 查看所有注册仓库的健康状态
./target/release/devbase health --detail

# 查询所有 Rust 项目
./target/release/devbase query "lang:rust"

# 启动交互式 TUI
./target/release/devbase tui

# 启动后台守护进程（每 3600 秒 tick 一次）
./target/release/devbase daemon --interval 3600

# 生成知识日报
./target/release/devbase digest
```

---

## 🖥️ TUI 操作指南

启动 `devbase tui` 后：

| 按键 | 动作 |
|------|------|
| `↑` / `↓` | 切换仓库 |
| `Home` / `End` | 跳到首项 / 末项 |
| `PgUp` / `PgDn` | 快速翻页 |
| `s` | 异步获取当前仓库的 fetch preview |
| `S` | 批量同步具有相同标签的仓库（弹出进度窗口，显示完成/运行/等待计数和已用时间） |
| `t` | 编辑当前仓库的标签 |
| `r` | 刷新仓库列表 |
| `h` | 显示/隐藏帮助条 |
| `q` / `Ctrl+C` | 退出 TUI |
| `Esc` / `Enter` | 关闭弹窗 |

---

## 🔧 MCP 工具清单

`devbase mcp --transport stdio` 注册以下工具：

| 工具名 | 功能 |
|--------|------|
| `devkit_scan` | 扫描目录并注册仓库 |
| `devkit_health` | 检查仓库健康状态 |
| `devkit_sync` | 同步仓库 |
| `devkit_query` | 查询知识库 |
| `devkit_index` | 索引仓库摘要和模块结构 |
| `devkit_note` | 为仓库添加学习笔记 |
| `devkit_digest` | 生成知识日报 |

---

## ⚙️ 配置文件

devbase 在启动时会读取 `~/.config/devbase/config.toml`（Windows 为 `%APPDATA%\devbase\config.toml`）。配置文件不存在时自动使用默认值。

示例配置：

```toml
[general]
language = "auto"          # "auto" | "zh-CN" | "en"

[daemon]
interval_seconds = 3600
incremental = true
health_stale_hours = 24

[cache]
ttl_seconds = 300

[watch]
max_files = 512

[digest]
window_hours = 24
```

---

## 🏗️ 架构定位

devbase 位于三层架构的**抽象层**：

```text
Clarity / Cursor / Claude Desktop (应用层)
              │
              │ MCP
              ▼
           devbase (抽象层：语义、关系、策略)
              │
              │ REST / 配置契约
              ▼
      syncthing / git (实体层：文件、网络、存储)
```

更多信息请参阅 [`ARCHITECTURE.md`](./ARCHITECTURE.md)。

---

## 📁 项目状态

- **当前版本**：v0.1.0-beta
- **注册仓库数**：22（含自有项目与第三方参考库）
- **测试状态**：27 个测试（25 passed, 2 ignored）
- **已知限制**：
  - LLM 语义提取因本地 Ollama 不可用而降级为规则模式
  - Clarity TUI 侧 MCP 调用链尚未完全打通

---

## 📜 许可证

MIT（待定）
