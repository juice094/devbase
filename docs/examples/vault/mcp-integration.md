---
id: mcp-integration-guide
repo: devbase
tags: [mcp, architecture, protocol]
ai_context: true
created: 2026-04-20
updated: 2026-04-23
---

# MCP 集成架构笔记

## 当前架构

devbase 作为 MCP Server，通过 stdio 和 SSE 两种 transport 对外暴露工具。

### stdio 模式

```
Clarity Agent -> MCP Client (stdio) -> devbase mcp
```

- 适合本地快速启动
- 每次连接都重新初始化
- 无状态

### SSE 模式（待完善）

```
Clarity Gateway -> SSE -> devbase daemon (常驻)
```

- 需要 `devbase daemon` 常驻运行
- 连接持久化
- 支持流式输出（待实现 `invoke_stream`）

## 已知问题

1. **Tool 暴露粒度**：19 个 tools 全量暴露，Clarity `McpManager` 全量注册
2. **Context 膨胀**：所有 tool descriptions 注入 system prompt
3. **Solution 方向**：tool 分级 + Clarity 侧注册过滤

## 关联仓库

- `C:\Users\22414\Desktop\devbase\src\mcp\tools\`
- `C:\Users\22414\Desktop\clarity\crates\clarity-core\src\mcp\`
