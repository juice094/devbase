# Sprint 2 计划（Phase 2：2026-04-18 ~ 2026-05-01）

## 目标

1. MCP SSE 流式 tool result（解决 clarity 2–5s 阻塞问题）
2. 农业领域 schema 落地（`agri_observations` + `devkit_agri_query`）
3. CLI/TUI pagination（`--limit` / `--page`）
4. `.syncdone` 文件标记与 syncthing-rust 集成

---

## W1（04-18 ~ 04-20）

### Task 2.1: McpTool trait 扩展 `invoke_stream()`

**目标**：让 `McpTool` 支持分段返回结果，避免 Agent 循环被全量 JSON 阻塞。

**设计**：
```rust
pub enum ToolEvent {
    Progress { message: String },
    Partial { content: serde_json::Value },
    Done { result: serde_json::Value },
}

pub trait McpTool: Send + Sync {
    fn name(&self) -> &'static str;
    fn schema(&self) -> serde_json::Value;
    
    // 默认实现：全量返回（向后兼容）
    async fn invoke(&self, args: serde_json::Value) -> anyhow::Result<serde_json::Value>;
    
    // 流式实现：分段推送
    async fn invoke_stream(
        &self,
        args: serde_json::Value,
        tx: mpsc::Sender<ToolEvent>,
    ) -> anyhow::Result<()>;
}
```

**实现步骤**：
1. 在 `mcp.rs` 中定义 `ToolEvent` enum
2. 扩展 `McpTool` trait，为所有现有 tool 提供默认 `invoke_stream()` 实现（内部调用 `invoke()` 后一次性 `send(Done)`）
3. `McpServer::handle_request_stream()` 新方法，返回 `Stream<Item = serde_json::Value>`
4. SSE `messages_handler` 适配：检测到 `id` 为流式请求时，用 `handle_request_stream()` 替代 `handle_request()`

**验收标准**：
- 现有 tool 不修改代码即可编译通过（默认实现 fallback）
- SSE 模式下，`initialize` 请求仍返回全量 JSON，`tools/call` 请求可返回分段 event
- stdio transport 行为不变

---

### Task 2.2: `agri_observations` schema migration

**阻塞**：等 agri-paper 提供 DDL PR。

**devbase 侧预备工作**：
1. `init_db()` 中预留 v5 migration 占位
2. `registry.rs` 中预留 `save_agri_observation()` / `query_agri_observations()` 方法签名

---

## W2（04-21 ~ 04-24）

### Task 2.3: SSE handler 流式适配

1. `messages_handler` 支持流式响应检测（通过请求 header 或 params）
2. `progress` / `partial` / `done` event 的 SSE 格式标准化
3. TUI 侧 `--stream` 标志（可选）

### Task 2.4: CLI pagination

1. `devbase health --detail --limit 10 --page 1`
2. `devbase query "tag:third-party" --limit 20`
3. TUI 列表分页（PgUp/PgDn 翻页）

---

## W3–W4（04-25 ~ 05-01）

### Task 2.5: `devkit_health`/`devkit_query` 流式集成

1. `DevkitHealthTool::invoke_stream()` 实现：逐仓库推送 `Progress` → 全部完成后 `Done`
2. `DevkitQueryTool::invoke_stream()` 实现：分页结果逐页 `Partial` → 最后 `Done`
3. TUI 进度条展示

### Task 2.6: `.syncdone` 文件标记

1. `sync_repo()` 成功后写入 `.devbase/syncdone`
2. 格式：`{"timestamp":"2026-04-17T10:42:00Z","local_commit":"abc1234"}`
3. 与 syncthing-rust `FolderStatus::Idle` REST endpoint 集成

---

## 风险与依赖

| 风险 | 缓解措施 |
|------|---------|
| agri-paper DDL PR 延迟 | 预备占位代码，不阻塞 W1 流式 trait 开发 |
| syncthing-rust REST endpoint 未就绪 | `.syncdone` 先写文件标记，REST 集成 deferred 到 Sprint 3 |
| clarity-core SSE Client 配置未更新 | stdio 保持为主路径，SSE 为 opt-in |
