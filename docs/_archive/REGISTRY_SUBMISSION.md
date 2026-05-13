# MCP Registry 提交指南

本文档汇总 devbase 提交到各 MCP 目录的操作步骤。

---

## 已准备的材料

| 文件 | 用途 |
|------|------|
| `scripts/install.ps1` | Windows 一键安装脚本 |
| `scripts/install.sh` | Linux/macOS 一键安装脚本 |
| `smithery.yaml` | Smithery.ai 配置文件 |
| `server.json` | MCP 标准 manifest（v0.2.3，19 tools）|
| `README.md` | 已更新安装说明和 tool 矩阵 |

---

## 渠道 A：awesome-mcp-servers（推荐优先）

**为什么先做这个**：SEO 最好，搜索 "MCP server" 排第一，完全免费，只需一次 PR。

**操作步骤**：

1. 打开 https://github.com/punkpeye/awesome-mcp-servers
2. 点击右上角 **Fork**
3. 在你的 fork 里，编辑 `README.md`
4. 找到 **Developer Tools** 分类（或 **Knowledge Management**），在列表末尾添加：

```markdown
- [devbase](https://github.com/juice094/devbase) - Developer Knowledge OS. Local-first bimodal workspace for humans (TUI) and AI agents (MCP). Manage Git repos, vault notes (PARA/Obsidian-compatible), and assets with 19 MCP tools including unified project context queries. `cargo install --path .` or `irm ... \| iex`
```

5. Commit 并点击 **Contribute → Open pull request**
6. PR 标题写：`Add devbase to Developer Tools`
7. 等合并（通常 1-3 天）

---

## 渠道 B：Smithery.ai

**为什么做这个**：支持一键安装 `npx @smithery/cli install devbase --client claude`

**操作步骤**：

1. 打开 https://smithery.ai
2. 注册/登录（可用 GitHub 账号）
3. 点击 **Add Server** 或 **Submit**
4. 填写：
   - Name: `devbase`
   - Repository: `https://github.com/juice094/devbase`
   - Description: `Developer Knowledge OS — manage Git repos, vault notes, and assets via MCP`
   - 选择 `stdio` 传输
5. 提交审核（通常当天通过）

> `smithery.yaml` 已放在仓库根目录，Smithery 会自动读取。

---

## 渠道 C：mcp.so

**操作步骤**：

1. 打开 https://mcp.so
2. 找到 **Submit Server** 按钮
3. 填写：
   - GitHub URL: `https://github.com/juice094/devbase`
   - Categories: `Developer Tools`, `Knowledge Management`
4. 提交

---

## 渠道 D：Glama.ai

**操作步骤**：

1. 打开 https://glama.ai/mcp
2. 点击 **Add Server**
3. 粘贴仓库链接并填写描述
4. 提交

---

## 渠道 E：PulseMCP

PulseMCP 主要从官方 Registry 同步，但可以通过邮件请求手动添加。

**操作步骤**：

1. 发邮件到 `hello@pulsemcp.com`
2. 主题：`Server submission: devbase (juice094/devbase)`
3. 正文：

```
Hi PulseMCP team,

I'd like to submit my MCP server to your directory:

- Name: devbase
- Repository: https://github.com/juice094/devbase
- Description: Developer Knowledge OS — local-first bimodal workspace for humans (TUI) and AI agents (MCP). 19 tools for repo management, vault notes, and unified project context.
- Transport: stdio
- Install: cargo build --release (Rust) or irm https://raw.githubusercontent.com/juice094/devbase/main/scripts/install.ps1 | iex

Please let me know if you need any additional information.

Best,
[你的名字]
```

---

## 后续维护

**需要更新 Registry 的情况**（低频，每次发版时做一次即可）：

| 事件 | 操作 |
|------|------|
| 发新版本（如 v0.2.4） | 更新 `server.json` 中的 version 和 tools 列表 |
| 新增/删除 MCP tool | 同步更新 `server.json`、`smithery.yaml`、`README.md` |
| awesome-mcp-servers 合并后 | 无需维护，除非仓库迁移或改名 |

**维护频率**：每发一个版本花 5-10 分钟更新 manifest 即可。社区目录（awesome/mcp.so/Glama）一次提交后基本不用管。

**优先级建议**：
1. **必做**：awesome-mcp-servers PR（长期曝光）
2. **推荐**：Smithery（一键安装体验好）
3. **可选**：mcp.so / Glama / PulseMCP（锦上添花）
