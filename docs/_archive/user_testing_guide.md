# devbase 用户测试指南（Sprint 2 验收测试）

**版本**：`main@76ccaf5`（2026-04-18）  
**构建状态**：✅ Release 构建成功  
**测试状态**：48 passed / 0 failed / 2 ignored

---

## 快速开始

```powershell
# 1. 构建 release 版本
cd C:\Users\22414\Desktop\devbase
cargo build --release

# 2. 运行（或添加到 PATH）
.\target\release\devbase.exe --help
```

---

## 测试 1：CLI 基础流程（核心工作流）

### 1.1 扫描工作区
```powershell
devbase scan "C:\Users\22414\Desktop" --register
```
**预期**：发现 Git repo 和非 Git 工作区（`SOUL.md` / `.devbase` 标记），注册到 registry.db。

### 1.2 健康检查（含分页）
```powershell
# 基础健康检查
devbase health

# 详细模式 + 分页
devbase health --detail --limit 5 --page 1
devbase health --detail --limit 5 --page 2
```
**预期**：第二页显示 `--page 2` 提示，JSON 输出包含 `pagination: {total, page, limit, has_more}`。

### 1.3 查询（含分页）
```powershell
devbase query "lang:rust" --limit 3 --page 1
devbase query "tag:third-party" --limit 10
```
**预期**：结果按 `limit` 截断，`has_more=true` 时提示下一页命令。

### 1.4 安全同步（dry-run）
```powershell
devbase sync --dry-run --strategy fetch-only --filter-tags "third-party"
```
**预期**：只读预览，显示每个仓库的 action（SKIP / FETCH / BLOCKED 等）。

### 1.5 操作日志
```powershell
devbase oplog --limit 10
devbase oplog --repo "devbase" --limit 5
```
**预期**：列出最近 scan / sync / health 操作记录。

---

## 测试 2：TUI 交互界面

```powershell
devbase tui
```

**测试项**：
- [ ] 列表按 `workspace_type` 分组（Git / openclaw / generic）
- [ ] 选中仓库后，右侧面板显示 tags（颜色按前缀区分：`[sync]` 青 / `[AI]` 洋红 / `[domain]` 黄）
- [ ] `s` 键弹出 Safe Sync Preview 弹窗
- [ ] 弹窗内分类显示：Will Run / Protected / Blocked / Up to Date
- [ ] `Enter` 执行安全同步（仅 Will Run 仓库）
- [ ] `q` 退出

---

## 测试 3：MCP SSE 流式传输

### 3.1 启动 SSE Server
```powershell
# 独立 SSE Server
devbase mcp --transport sse --port 3002

# 或 Daemon 模式内置 SSE
devbase daemon --sse-port 3002 --interval 300
```

### 3.2 使用浏览器/curl 测试连接
```powershell
# 获取 SSE endpoint
curl -N http://127.0.0.1:3002/sse

# 发送 tools/list 请求（获取 session_id 从 SSE event）
curl -X POST "http://127.0.0.1:3002/messages?session_id=<SESSION_ID>" `
  -H "Content-Type: application/json" `
  -d '{"jsonrpc":"2.0","id":1,"method":"tools/list"}'

# 发送流式 health 请求
curl -X POST "http://127.0.0.1:3002/messages?session_id=<SESSION_ID>" `
  -H "Content-Type: application/json" `
  -d '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"devkit_health","arguments":{"detail":true},"_stream":true}}'
```

**预期**：
- `tools/list` 返回 10 个 tool
- `_stream=true` 时，先收到 `Progress` event，再分批收到 `Partial` event，最后 `Done`
- `_stream=false`（或省略）时，一次性返回完整 JSON-RPC 响应

---

## 测试 4：Registry 备份与恢复

```powershell
# 导出备份
devbase registry export --format sqlite
devbase registry export --format json

# 查看备份列表
devbase registry backups

# 清理旧备份（保留最近 10 个）
devbase registry clean
```

**预期**：备份文件出现在 `%LOCALAPPDATA%\devbase\backups\`。

---

## 测试 5：.syncdone 文件标记

```powershell
# 执行一次真实同步（非 dry-run）
devbase sync --strategy fetch-only --filter-tags "third-party"

# 检查同步成功的仓库根目录
Get-Content .\.devbase\syncdone | ConvertFrom-Json
```

**预期**：
```json
{
  "timestamp": "2026-04-18T...",
  "local_commit": "abc1234...",
  "action": "FETCH"
}
```

---

## 反馈模板

发现 bug 或体验问题时，请按以下格式反馈：

```
**测试项**：TUI Safe Sync / SSE streaming / CLI pagination …
**命令**：`devbase health --detail --limit 5`
**实际结果**：…
**预期结果**：…
**环境**：Windows 11 / devbase@76ccaf5 / registry.db 39 repos
```

---

## 已知限制

1. **agri_observations**：Schema v5 表已创建，但 `devkit_agri_query` tool 未启用（等 agri-paper DDL PR）
2. **syncthing-rust 集成**：`.syncdone` 文件已写入，REST endpoint 集成待上游
3. **TUI 分页**：PgUp/PgDn 翻页未实现（CLI 分页已完成）
4. **clarity-core coupling**：仍为 path dep，编译较重，Sprint 3 计划提取 `devbase-core`
