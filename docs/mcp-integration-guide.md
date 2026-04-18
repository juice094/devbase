# MCP 集成指南

> 让任何 AI 助手通过 devbase 理解你的本地代码库

---

## 什么是 MCP

MCP（Model Context Protocol）是 AI 助手与外部工具通信的标准协议。devbase 作为 **MCP Server**，向 AI 暴露一组结构化工具，让 AI 能够：

- 查询本地有哪些项目、它们的状态如何
- 批量同步仓库、检查健康度
- 获取代码统计、模块结构、GitHub 元数据
- 添加笔记、生成知识日报

**核心优势**：AI 无法识别 GUI（桌面应用对 AI 是黑盒），但 AI 可以调用 MCP 工具。devbase 是 AI 理解本地代码库的**唯一结构化入口**。

---

## 快速配置

### 1. 安装 devbase

```bash
cargo install --path .
# 或从 crates.io（未来发布）
# cargo install devbase
```

### 2. 扫描并注册你的代码库

```bash
# 扫描当前目录下的所有 Git 仓库
devbase scan . --register

# 验证注册结果
devbase health --detail
```

### 3. 配置 AI 助手连接 devbase MCP

#### Claude Code（官方 Anthropic CLI）

在 `~/.claude/mcp.json` 中添加：

```json
{
  "mcpServers": {
    "devbase": {
      "command": "devbase",
      "args": ["mcp"],
      "env": {}
    }
  }
}
```

#### Claude Code Rust（社区版）

在配置文件中添加 MCP Server：

```json
{
  "mcpServers": {
    "devbase": {
      "command": "devbase",
      "args": ["mcp", "--transport", "stdio"]
    }
  }
}
```

#### 5ire

5ire 作为 MCP Client，在设置 → MCP Server 中添加：
- Transport: stdio
- Command: `devbase mcp`

#### Kimi / 其他支持 MCP 的助手

通用配置模板：

```json
{
  "mcpServers": {
    "devbase": {
      "command": "devbase",
      "args": ["mcp"]
    }
  }
}
```

### 4. 验证连接

在 AI 助手的对话中输入：

> "请用 devbase 查看我本地有哪些项目"

AI 会调用 `devkit_health` 工具，返回你的仓库列表和状态。

---

## 可用 Tool 清单（12 个）

| Tool | 功能 | AI 使用场景 |
|------|------|-----------|
| `devkit_scan` | 扫描目录并注册工作区 | "扫描 ~/projects 目录下的所有仓库" |
| `devkit_health` | 健康检查（所有仓库状态） | "我本地有哪些项目需要同步？" |
| `devkit_sync` | 批量同步（dry-run 默认） | "预览同步这些仓库会发生什么" |
| `devkit_query_repos` | 按语言/标签/状态查询仓库 | "列出所有 dirty 的 Rust 项目" |
| `devkit_code_metrics` | 代码统计（行数、文件数、语言） | "我最大的项目是什么？" |
| `devkit_index` | 索引仓库摘要和模块结构 | "为所有仓库生成知识索引" |
| `devkit_query` | 知识库查询（tantivy 搜索） | "搜索关于 'sync policy' 的知识" |
| `devkit_note` | 为仓库添加笔记 | "给 devbase 项目添加一条笔记" |
| `devkit_digest` | 生成每日知识简报 | "生成今天的知识日报" |
| `devkit_github_info` | 查询 GitHub 元数据 | "devbase 项目有多少 stars？" |
| `devkit_paper_index` | 索引 PDF 论文 | "索引 ~/papers 目录" |
| `devkit_experiment_log` | 记录实验运行 | "记录这次实验的配置和结果" |

---

## 示例对话

### 场景 1：代码库全景扫描

**用户**：看看我本地所有项目的状态

**AI（调用 `devkit_health`）**：
```json
{
  "success": true,
  "summary": { "total_repos": 12, "dirty": 1, "behind": 3, "up_to_date": 8 },
  "repos": [
    { "id": "devbase", "status": "ahead:2,behind:0", "health": "ok" },
    { "id": "syncthing-rust", "status": "ahead:0,behind:3", "health": "stale" }
  ]
}
```

**AI**：你本地注册了 12 个项目。其中：
- **1 个**有未提交变更（dirty）
- **3 个**落后于远程（behind），建议 fetch
- **8 个**状态正常

`syncthing-rust` 已经 3 天没有同步，落后远程 3 个 commit，建议运行 `devbase sync`。

---

### 场景 2：智能筛选

**用户**：我有哪些 Rust 项目超过 1000 行代码？

**AI（调用 `devkit_code_metrics`）**：
```json
{
  "success": true,
  "count": 3,
  "repos": [
    { "repo_id": "devbase", "total_lines": 12543, "source_lines": 9876, "language_breakdown": {"Rust": {"code": 9876, "comments": 1200}} },
    { "repo_id": "syncthing-rust", "total_lines": 45210, "source_lines": 38900, "language_breakdown": {"Rust": {"code": 38900, "comments": 3400}} }
  ]
}
```

**AI**：你有 3 个 Rust 项目超过 1000 行：
1. **syncthing-rust** — 45,210 行（代码 38,900 行）
2. **devbase** — 12,543 行（代码 9,876 行）

---

### 场景 3：安全同步

**用户**：同步所有 behind 的仓库，但要先预览

**AI（调用 `devkit_sync` with dry_run=true）**：
```json
{
  "success": true,
  "dry_run": true,
  "results": [
    { "repo": "devbase", "action": "DRY_RUN", "would_pull": "fast-forward 3 commits" },
    { "repo": "syncthing-rust", "action": "BLOCKED", "reason": "dirty working tree" }
  ]
}
```

**AI**：预览结果：
- ✅ **devbase** — 可以安全 fast-forward 合并 3 个 commit
- ⚠️ **syncthing-rust** — 工作区有未提交变更，已自动跳过（Conservative 策略保护）

是否执行实际同步？

---

### 场景 4：跨仓库代码搜索

**用户**：搜索所有项目中包含 `fetch_github_stars` 的地方

**AI（调用 `devkit_query_repos` 获取仓库列表，然后用户通过 TUI 的 `/` 搜索功能查看）**：

> 注：此场景目前需要人类在 TUI 中按 `/` 执行搜索，AI 侧可通过 `devkit_query_repos` 获取仓库路径后，建议用户打开 TUI 搜索。

**未来**：计划增加 `devkit_grep` MCP tool，让 AI 直接发起跨仓库代码搜索。

---

## 传输模式

devbase MCP Server 支持两种传输模式：

### stdio（推荐，本地 AI 助手）

```bash
devbase mcp
# 或
devbase mcp --transport stdio
```

AI 助手通过子进程启动 devbase，通过 stdin/stdout 通信。

### SSE（服务器模式，远程 AI 或调试）

```bash
devbase mcp --transport sse --port 3001
```

AI 助手通过 HTTP SSE 连接 `http://localhost:3001/sse`。

---

## 故障排除

### AI 说"找不到 devbase 工具"

1. 确认 devbase 在 PATH 中：`which devbase`
2. 确认配置文件的 JSON 格式正确
3. 重启 AI 助手客户端

### `devkit_health` 返回空列表

1. 先运行 `devbase scan . --register` 注册仓库
2. 确认注册表路径：`%LOCALAPPDATA%\devbase\registry.db`（Windows）或 `~/.local/share/devbase/registry.db`（Linux/macOS）

### MCP 通信超时

- stdio 模式下，devbase 首次启动可能需要 1-2 秒初始化数据库
- 在配置中增加 `timeout: 10000`（10 秒）

---

## 路线图

| 功能 | 状态 |
|------|------|
| 12 个基础 MCP tool | ✅ 可用 |
| `devkit_query_repos` 结构化查询 | ✅ 可用 |
| `devkit_code_metrics` 代码统计 | ✅ 可用 |
| `devkit_grep` 跨仓库代码搜索 | 🚧 规划中 |
| `devkit_module_graph` 模块依赖图 | 🚧 规划中 |
| 自然语言查询接口 | 🚧 规划中 |

---

*最后更新：2026-04-15*
