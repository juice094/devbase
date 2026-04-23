# 5ire MCP 集成指南

> 让 5ire AI 助手通过 devbase 理解你的本地代码库

---

## 5ire 简介

5ire 是一个本地优先的 AI 助手平台（Electron + TypeScript），支持 MCP Client 协议。与 devbase 结合后，5ire 可以：

- 查询本地所有项目的状态和健康度
- 获取代码统计、模块结构、Stars 趋势
- 批量同步仓库（通过 dry-run 预览）
- 管理你的开发者知识库和 Vault 笔记

---

## 配置方式一：5ire GUI 界面（推荐）

### 步骤 1：打开 Tool 页面

启动 5ire → 左侧导航栏点击 **Tool** → 点击 **+ New** 或 **Add Server**

### 步骤 2：填写 devbase 配置

| 字段 | 值 |
|------|-----|
| **Name** | `devbase` |
| **Key** | `devbase`（唯一标识） |
| **Transport** | `stdio` |
| **Endpoint** | `devbase mcp` |
| **Description** | `Developer workspace knowledge base — query local repos, sync, health, code metrics` |
| **Is Active** | ✅ 勾选 |

### 步骤 3：保存并连接

点击 **Save** → 5ire 会自动尝试连接 devbase MCP Server → 状态变为 🟢 **Connected**

### 步骤 4：验证

在 5ire 的聊天窗口中输入：

> "请用 devbase 查看我本地有哪些项目"

5ire 会调用 `devkit_health`，返回你的仓库列表。

---

## 配置方式二：5ire 数据库直接配置（高级）

5ire 的 MCP Server 配置存储在 SQLite 数据库中。如果你需要批量配置或脚本化：

```sql
-- 5ire 数据库路径（示例）
-- Windows: %APPDATA%\5ire\5ire.db
-- macOS: ~/Library/Application Support/5ire/5ire.db
-- Linux: ~/.config/5ire/5ire.db

INSERT INTO mcp_servers (key, name, transport, endpoint, description, is_active, created_at, updated_at)
VALUES (
    'devbase',
    'devbase',
    'stdio',
    'devbase mcp',
    'Developer workspace knowledge base',
    1,
    datetime('now'),
    datetime('now')
);
```

重启 5ire 后生效。

---

## 配置方式三：5ire 内置模板（开发者）

如果你是 5ire 开发者或想贡献内置模板，可以在 `src/mcp.config.ts` 中添加：

```typescript
{
  key: 'Devbase',
  command: 'devbase',
  description: 'Developer workspace database and knowledge-base manager for local repos',
  args: ['mcp'],
  isActive: false,
},
```

---

## 5ire 中可用的 devbase Tool

连接成功后，5ire 的 AI 助手可以调用以下 19 个 tool：

### Repo（13 个）

| Tool | 5ire 中的使用场景 |
|------|----------------|
| `devkit_scan` | "扫描 ~/projects 目录并注册所有仓库" |
| `devkit_health` | "我本地有哪些项目需要同步？" |
| `devkit_sync` | "预览同步这些仓库会发生什么" |
| `devkit_query_repos` | "列出所有 dirty 的 Rust 项目" |
| `devkit_index` | "为所有仓库生成知识索引" |
| `devkit_note` | "给 devbase 项目添加一条笔记" |
| `devkit_digest` | "生成今天的知识日报" |
| `devkit_github_info` | "devbase 项目有多少 stars？" |
| `devkit_paper_index` | "索引 ~/papers 目录" |
| `devkit_experiment_log` | "记录这次实验的配置" |
| `devkit_code_metrics` | "我最大的项目是什么？" |
| `devkit_module_graph` | "devbase 项目有哪些二进制目标？" |
| `devkit_natural_language_query` | "用自然语言搜索我的代码库知识" |

### Vault（4 个）

| Tool | 5ire 中的使用场景 |
|------|----------------|
| `devkit_vault_search` | "在 Vault 中搜索关于架构的笔记" |
| `devkit_vault_read` | "读取 Vault 中某篇笔记的内容" |
| `devkit_vault_write` | "在 Vault 中创建或更新一条笔记" |
| `devkit_vault_backlinks` | "查看某篇笔记被哪些其他笔记引用" |

### Query（1 个）

| Tool | 5ire 中的使用场景 |
|------|----------------|
| `devkit_query` | "搜索关于 sync policy 的知识" |

### Context（1 个）

| Tool | 5ire 中的使用场景 |
|------|----------------|
| `devkit_project_context` | "获取当前项目的上下文摘要" |

---

## 示例对话

### 场景 1：代码库健康检查

**用户**：看看我本地项目的整体状态

**5ire（调用 `devkit_health`）**：
> 你本地注册了 12 个项目：
> - 🟢 8 个状态正常
> - 🟡 3 个落后于远程（behind）
> - 🔴 1 个工作目录有未提交变更
>
> 建议：运行 `devbase sync` 同步那 3 个 behind 的项目。

### 场景 2：按条件筛选

**用户**：我有哪些超过 1000 行的 Rust 项目？

**5ire（调用 `devkit_code_metrics`）**：
> 你有 3 个 Rust 项目超过 1000 行：
> 1. **syncthing-rust** — 45,210 行
> 2. **devbase** — 12,543 行
> 3. **claude-code-rust** — 8,900 行

### 场景 3：模块结构查询

**用户**：devbase 项目有哪些可执行文件？

**5ire（调用 `devkit_module_graph` with repo_id="devbase"）**：
> devbase 项目的模块结构：
> - `devbase` (bin) — src/main.rs
> - `devbase` (lib) — src/lib.rs

### 场景 4：Vault 笔记管理

**用户**：我在 Vault 里有没有记录关于 MCP 集成的想法？

**5ire（调用 `devkit_vault_search` with query="MCP 集成"）**：
> 找到 2 条相关笔记：
> 1. `docs/mcp-integration-guide.md` — 最后更新 2026-04-23
> 2. `ideas/mcp-sse-vs-stdio.md` — 最后更新 2026-04-20

---

## 故障排除

### 5ire 显示 "Connection failed"

1. 确认 devbase 在 PATH 中：`which devbase`（或 `where devbase` on Windows）
2. 确认 devbase 已注册仓库：`devbase scan . --register`
3. 检查 5ire 日志：View → Toggle Developer Tools → Console

### devbase 返回空结果

1. 先手动运行 `devbase health --detail` 确认有数据
2. 5ire 中重新连接 devbase server（Disable → Enable）

### Tool 调用超时

- devbase 首次查询可能需要 1-2 秒初始化 SQLite 连接
- 在 5ire 设置中增加 MCP timeout（如果有该选项）

---

## 与 Claude Code 的对比

| 维度 | 5ire + devbase | Claude Code + devbase |
|------|---------------|----------------------|
| 界面 | GUI (Electron) | CLI/TUI |
| 本地模型 | ✅ 支持 Ollama | ❌ 云端 API |
| 知识库 | ✅ 内置 + devbase | ✅ devbase |
| 适用场景 | 桌面办公、可视化 | 终端开发、键盘流 |

**最佳实践**：5ire 用于日常知识管理和可视化查看，Claude Code 用于编码时的上下文查询。

---

*最后更新：2026-04-23*
