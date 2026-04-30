# 5 分钟上手指南

> 从安装到第一条 AI 查询，只需 5 分钟。

---

## 1. 安装

```bash
# 从源码安装（需要 Rust 1.94+）
git clone https://github.com/juice094/devbase.git
cd devbase && cargo install --path .

# 验证安装
devbase --version
```

---

## 2. 扫描代码库

```bash
# 扫描当前目录下的所有 Git 仓库，并注册到 devbase
devbase scan . --register

# 验证注册结果
devbase health
```

输出示例：
```
摘要:
  total_repos: 46
  dirty_repos: 3
  behind_upstream: 1
```

---

## 3. 索引仓库

让 devbase 理解你的代码结构：

```bash
# 索引所有已注册的仓库
devbase index

# 或索引特定仓库
devbase index ./my-project
```

索引会提取：
- README 摘要和关键词
- 模块结构（cargo targets / Python packages）
- 代码符号（函数、结构体、枚举）
- 调用关系图

---

## 4. 配置 AI 助手（MCP）

### Kimi CLI

编辑 `~/.kimi/mcp.json`：

```json
{
  "mcpServers": {
    "devbase": {
      "command": "devbase",
      "args": ["mcp"],
      "env": {
        "DEVBASE_MCP_ENABLE_DESTRUCTIVE": "1",
        "DEVBASE_MCP_TOOL_TIERS": "stable,beta"
      }
    }
  }
}
```

### Claude Code

编辑 `~/.claude/mcp.json`：

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

---

## 5. 第一条 AI 查询

配置完成后，在 AI 对话中尝试：

> "分析 devbase 的 sync 模块架构"

AI 会：
1. 调用 `devkit_project_context` 获取项目上下文（模块树 + 符号 + 调用关系）
2. 根据返回的结构，决定读取哪些文件
3. 给出架构分析

---

## 下一步

- 了解完整 CLI 命令 → [`cli-reference.md`](cli-reference.md)
- 使用 Vault 管理笔记 → [`vault-workflow.md`](vault-workflow.md)
- 查看所有 MCP 工具 → [`reference/mcp-tools.md`](../reference/mcp-tools.md)
