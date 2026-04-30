# 工程师冒烟测试报告（2026-04-18）

**版本**：`main@76ccaf5` → `6bc7733`  
**构建**：Release 通过  
**单元测试**：48 passed / 0 failed / 2 ignored

---

## 测试执行记录

### ✅ 通过的测试项

| # | 命令 | 结果 | 备注 |
|---|------|------|------|
| 1 | `devbase --help` | ✅ | 所有子命令正常列出 |
| 2 | `devbase scan . --register` | ✅ | 发现 devbase 自身并注册 |
| 3 | `devbase health` | ✅ | 39 repos, 6 dirty, 环境检测正常 |
| 4 | `devbase health --detail --limit 3 --page 1` | ✅ | 分页提示正确 `(more results available, use --page 2)` |
| 5 | `devbase query "lang:rust" --limit 3` | ✅ | 15 results, page 1 of ~5 |
| 6 | `devbase oplog --limit 5` | ✅ | health 操作记录正确 |
| 7 | `devbase registry backups` | ✅ | 2 个历史备份存在 |
| 8 | `devbase registry export --format json` | ✅ | 导出到 `%LOCALAPPDATA%\devbase\backup\` |
| 9 | `devbase sync --dry-run --json` | ✅ | SYNC 模式，5 个 third-party 仓库秒级返回 |
| 10 | `devbase sync --dry-run --filter-tags "rust-ml"` | ✅ | ASYNC 模式，2 个仓库正常完成 |
| 11 | `devbase mcp --transport sse --port 3003` | ✅ | SSE endpoint event 正常返回 |
| 12 | `devbase daemon --sse-port 3004` | ✅ | Daemon + 内置 SSE 双服务启动正常 |

### ⚠️ 发现的问题

#### ✅ Bug 1 已修复：`sync` ASYNC 模式超时

**现象**（修复前）：
```powershell
devbase sync --dry-run --strategy fetch-only --filter-tags "third-party"
# → 120s 超时，只打印 RUNNING 状态，无完成消息
```

**根因分析**：
- `SyncOrchestrator` ASYNC 分支的 `tokio::spawn` task 在某种调度条件下未能执行
- 当仓库数 > semaphore 容量（4）时， permit 无法释放，后续 task 死等

**修复**（`5de6855`）：
- ASYNC/BlockUi 分支临时降级为与 SYNC 相同的顺序执行逻辑
- 消除了 `tokio::spawn` + semaphore 的调度死锁
- 16 仓库同步从 120s+ 超时 → 秒级完成

**验证**：
```powershell
devbase sync --dry-run --filter-tags "reference"      # ✅ 16 仓库秒级完成
devbase sync --dry-run --filter-tags "third-party"      # ✅ 5 仓库秒级完成
```

---

#### Bug 2：SSE 测试需保持长连接

**现象**：
```powershell
curl http://127.0.0.1:3003/sse          # 获取 session_id 后断开
curl -X POST .../messages?session_id=.. # → 404
```

**根因**：`messages_handler` 通过 SSE channel 向 session 发送响应。客户端断开 SSE 后 channel 关闭，`tx.send` 失败 → 返回 404。

**这不是 bug**，是 SSE 协议的正常行为。但测试文档需要明确说明：SSE 客户端必须保持长连接。

---

## 修复验证（Bug 1）

```powershell
# 修复前：ASYNC + 多仓库 → 超时
devbase sync --dry-run --filter-tags "reference"     # ❌ 超时

# 修复前：ASYNC + 少仓库 → 正常
devbase sync --dry-run --filter-tags "rust-ml"       # ✅ 2 仓库正常

# 修复前：SYNC (--json) → 正常
devbase sync --dry-run --filter-tags "third-party" --json  # ✅ 秒级返回
```

---

## 用户侧体验测试步骤

### 前置准备

```powershell
cd C:\Users\22414\Desktop\devbase
cargo build --release
$env:PATH += ";C:\Users\22414\Desktop\devbase\target\release"
```

### Step 1：CLI 核心工作流（5 分钟）

```powershell
# 1.1 扫描并注册仓库
devbase scan "C:\Users\22414\Desktop" --register

# 1.2 健康检查（含分页）
devbase health
devbase health --detail --limit 5 --page 1
devbase health --detail --limit 5 --page 2

# 1.3 查询（含分页）
devbase query "lang:rust" --limit 5
devbase query "tag:third-party" --limit 3 --page 1

# 1.4 同步预览（⚠️ 使用 --json 模式避免超时）
devbase sync --dry-run --strategy fetch-only --filter-tags "third-party" --json

# 1.5 查看操作日志
devbase oplog --limit 10
```

**预期体验**：
- health/query 分页提示清晰，知道还有多少页
- `--json` 输出可 piping 到 `jq` 或其他工具
- oplog 显示最近操作的时间线

### Step 2：TUI 交互（3 分钟）

```powershell
devbase tui
```

**操作清单**：
- [ ] 上下键浏览仓库列表，观察左侧 emoji 分组（📁 Git / 🔮 openclaw / 📂 generic）
- [ ] 观察右侧详情面板的 tag 颜色：`[sync]` 青色、`[AI]` 洋红色、`[domain]` 黄色
- [ ] 按 `s` 弹出 Safe Sync Preview
- [ ] 弹窗中确认分类：Will Run / Protected / Blocked / Up to Date
- [ ] 按 `Enter` 执行（仅 Will Run 仓库被同步）
- [ ] 按 `q` 退出

**⚠️ 注意**：如果仓库数量很多，TUI Safe Sync 可能触发 ASYNC 超时 Bug。建议先 `--filter-tags` 减少范围。

### Step 3：Registry 备份与恢复（2 分钟）

```powershell
devbase registry export --format sqlite
devbase registry export --format json
devbase registry backups
devbase registry clean
```

**预期体验**：备份文件自动命名带时间戳，clean 保留最近 10 个。

### Step 4：MCP SSE 流式测试（3 分钟）

```powershell
# 4.1 启动 SSE Server（保持窗口运行）
devbase mcp --transport sse --port 3002

# 4.2 另开一个 PowerShell，保持 SSE 长连接
curl.exe -N http://127.0.0.1:3002/sse
# → 记录返回的 session_id

# 4.3 再开一个 PowerShell，发送请求
$body = '{"jsonrpc":"2.0","id":1,"method":"tools/list"}'
Invoke-WebRequest -Uri 'http://127.0.0.1:3002/messages?session_id=<SESSION_ID>' -Method POST -ContentType 'application/json' -Body $body

# 4.4 测试流式 health
$body = '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"devkit_health","arguments":{"detail":true},"_stream":true}}'
Invoke-WebRequest -Uri 'http://127.0.0.1:3002/messages?session_id=<SESSION_ID>' -Method POST -ContentType 'application/json' -Body $body
# → 在 SSE 连接窗口观察分批 event
```

**关键**：SSE 连接窗口（curl -N）必须保持打开，否则 session 失效。

### Step 5：Daemon + 内置 SSE（2 分钟）

```powershell
# 5.1 后台启动 daemon（含 SSE）
devbase daemon --sse-port 3002 --interval 300

# 5.2 测试 SSE 连通性（同 Step 4）
curl.exe -N http://127.0.0.1:3002/sse
```

**预期体验**：daemon 在后台周期性执行 health/index/discovery/digest，同时提供 SSE MCP 服务。

### Step 6：.syncdone 标记验证（2 分钟）

```powershell
# 6.1 对一个仓库执行真实 fetch-only 同步（安全，不修改本地文件）
devbase sync --strategy fetch-only --filter-tags "rust-ml" --json

# 6.2 检查 .syncdone 文件
Get-Content C:\Users\22414\dev\third_party\burn\.devbase\syncdone | ConvertFrom-Json
```

**预期输出**：
```json
{
  "timestamp": "2026-04-18T...",
  "local_commit": "abc1234...",
  "action": "FETCH"
}
```

---

## 反馈收集

发现 bug 或体验问题时，请按以下格式反馈：

```
**测试项**：TUI Safe Sync / SSE streaming / CLI pagination …
**命令**：devbase health --detail --limit 5
**实际结果**：…
**预期结果**：…
**环境**：Windows 11 / devbase@76ccaf5 / registry.db 39 repos
```

---

## 已知限制（无需反馈）

1. `agri_observations`：Schema v5 表已创建，但 `devkit_agri_query` tool 未启用（等 agri-paper DDL PR）
2. ~~`sync` ASYNC 超时~~：已修复 (`5de6855`)
3. syncthing-rust REST endpoint：`.syncdone` 文件已写入，REST 集成待上游
4. TUI 分页翻页：PgUp/PgDn 未实现（CLI 分页已完成）
5. clarity-core coupling：仍为 path dep，编译较重，Sprint 3 计划提取 `devbase-core`
