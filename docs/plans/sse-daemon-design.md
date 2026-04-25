# SSE Daemon 设计草案（阶段二候选）

> **状态**：设计草案，未进入实现  
> **目标版本**：v0.4.0+  
> **阻塞原因**：阶段一功能冻结；SSE 非 v0.3.0 验收标准  
> **来源**：从 `docs/archive/DEVELOPMENT_ROADMAP_0423.md` 波次 3 提取

---

## 背景

devbase 当前仅支持 `stdio` MCP 传输。SSE（Server-Sent Events）HTTP 传输可让 MCP Client（如 clarity-gateway、5ire）通过持久化 HTTP 连接调用 devbase，无需维护本地进程生命周期。

---

## 设计目标

1. **Daemon 模式**：`devbase daemon` 启动后台 HTTP Server
2. **SSE 端点**：`/mcp/v1/sse` 提供事件流；`/mcp/v1/message` 接收 JSON-RPC 请求
3. **流式 Tool 调用**：支持 `McpTool::invoke_stream()`，长运行 tool（如 `index_repo`）可分段推送进度
4. **零破坏**：现有 `stdio` 模式不受影响，SSE 为新增传输层

---

## 关键链路

```
[devbase W1: invoke_stream trait] 
        │
        ▼
[devbase W2: SSE handler 流式适配]
        │
        ▼
[devbase W3-W4: Daemon 内置 SSE Server]
        │
        ▼
[clarity-gateway: SSE 持久化适配]     ← 依赖 devbase daemon 可用
        │
        ▼
[集成测试: devbase daemon + clarity-gateway 长连接]
```

---

## 技术选型

- **HTTP 框架**：`axum`（已熟悉，tokio 生态）
- **SSE 实现**：`axum::response::Sse` + `tokio::sync::broadcast`
- **进程管理**：`devbase daemon start/stop/status`，PID 文件锁
- **配置**：`[daemon]` 段，`port = 8765`，`bind = "127.0.0.1"`

---

## 验收标准

- [ ] `devbase daemon` 启动后，`curl http://localhost:8765/mcp/v1/sse` 返回事件流
- [ ] MCP Inspector 可通过 SSE 连接调用全部 34 tools
- [ ] 长运行 tool（如 `index_repo`）支持进度分段推送
- [ ] `devbase daemon stop` 优雅关闭，无僵尸连接

---

## 风险

| 风险 | 缓解 |
|------|------|
| clarity 侧 SSE 适配延期 | devbase daemon 独立可用，不阻塞 clarity |
| SSE 长连接稳定性 | 心跳机制 + 自动重连 |
| 端口冲突 | 默认 127.0.0.1:8765，支持配置覆盖 |

---

*Defer 至阶段二启动后评估具体排期。*
