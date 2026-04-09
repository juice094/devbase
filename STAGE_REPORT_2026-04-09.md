# devbase 阶段性报告

**日期**：2026-04-09  
**范围**：模块解构、第三方项目横向对比、MCP/TUI/Sync/Registry/Watch/Syncthing 多层架构升级，以及本轮工程硬化（配置系统、Daemon 增量化、repo_tags 规范化）  
**基线版本**：v0.1.0 (2026-04-05) → 当前增强版

---

## 一、原始状态（2026-04-05 基线）

| 模块 | 原始能力 | 主要缺陷 |
|------|----------|----------|
| `scan` | 递归发现 `.git`，提取 upstream/branch | 无语言检测、无 ZIP 快照识别、无测试 |
| `health` | 仅输出仓库数量和简单明细 | **严重欠实现**：未检测 dirty/behind、未检测环境工具链 |
| `query` | 简单 `LIKE` 模糊查询 | 仅 MVP，不支持结构化语法（`lang:` / `stale:`） |
| `sync` | fetch-only / auto-pull / ask | 无标签过滤、无结构化输出、错误处理粗糙 |
| `tui` | 左右分栏基础界面 | 功能单一，后台操作会阻塞 UI |
| `registry` | 单表 SQLite `repos` | 字段平铺，无法表达多 remote 和健康缓存 |
| `mcp` | 仅有契约文档 | 无运行时代码 |

---

## 二、当前状态（2026-04-09 下午）

### 2.1 架构升级总览

- **应用层**：MCP Server 已实现（`devbase mcp --transport stdio`），暴露 **7 个工具**。stdio 输出已兼容 `Content-Length` 格式规范。
- **交互层**：TUI 引入异步事件循环，后台 Git 操作不再阻塞 UI，支持实时 spinner 和批量同步弹窗。
- **抽象层**：`SyncOrchestrator` 支持并发批量同步（默认并发度 4），错误降级为分类状态标签。
- **存储层**：Registry 从单表升级为 `repos` + `repo_remotes` + `repo_health` + `repo_tags` 多表，兼容旧数据自动迁移；新增 5 分钟健康缓存，health 查询提速约 13 倍。
- **知识层**：新增 `knowledge_engine`（README 摘要 + Rust 模块结构提取）、`discovery_engine`（依赖发现 + Jaccard 相似度）、`digest`（日报生成）。
- **实体层**：引入 `sync_protocol.rs`（版本向量 + 块级索引抽象）、`watch.rs`（目录监控 + 事件聚合 + 变更调度）、`syncthing_client.rs`（Syncthing REST API 桥接，支持 `devbase syncthing-push`）。
- **配置层**：新增 `config.rs`，支持 `~/.config/devbase/config.toml` 配置，消除了所有 Magic Number。
- **Daemon**：实现 health → re-index → discovery → digest 自动化闭环，且 health/re-index 已支持增量策略。

### 2.2 模块变更清单

#### `src/mcp.rs`（新增后持续优化）
- 实现基于 `tokio::io` 的 stdio JSON-RPC 消息循环。
- 支持 `initialize`、`tools/list`、`tools/call`。
- 注册 7 个 MCP Tool：
  - `devkit_scan` / `devkit_health` / `devkit_sync` / `devkit_query`
  - `devkit_index` / `devkit_note` / `devkit_digest`
- stdio 输出已修正为 `Content-Length: <len>\r\n\r\n<json>\n`，兼容 Anthropic 规范客户端。
- 错误响应从手动字符串拼接改为 `serde_json::json!` 安全构造。
- 底部集成 7 个 `#[tokio::test]` 集成测试。

#### `src/scan.rs`
- 新增 `detect_language`：根据 `Cargo.toml` / `package.json` / `go.mod` / `pyproject.toml` / `CMakeLists.txt` 自动识别语言。
- 新增 ZIP 快照检测：目录名以 `-main` / `-master` 结尾时，初始 tags 为 `discovered,zip-snapshot,needs-migration`。
- 新增 `run_json`：返回结构化 JSON（`success`、`count`、`registered`、`repos`）。
- 数据库写入适配新 Registry 多表结构（通过 `WorkspaceRegistry::save_repo`）。
- 底部新增 12 个单元测试。

#### `src/health.rs`
- 新增 `run_json`：返回完整 MCP 契约 JSON，包含：
  - `summary`：`total_repos`、`dirty_repos`、`behind_upstream`、`no_upstream`
  - `environment`：`rustc`、`cargo`、`node`、`go`、`cmake` 版本
  - `repos`：每个仓库的 `status`（ok/dirty/ahead/behind/diverged/detached/no_upstream/error）、ahead、behind
- `calc_ahead_behind` 增强容错：detached HEAD 或找不到 upstream ref 时不再报错，返回 `ok` 状态。
- **Registry 缓存预热**：优先读取 `repo_health` 缓存（TTL 可配置），命中时跳过 `git2::Repository::open`。
- 分析完成后自动调用 `WorkspaceRegistry::save_health` 缓存结果。
- 原有 `run` 函数改为 `run_json` 的 CLI 包装层。

#### `src/query.rs`
- 新增轻量级查询表达式解析器，支持：
  - `lang:<语言>`（按构建文件推断）
  - `stale:>N` / `stale:<N`（基于 `last_sync` 天数）
  - `behind:>N` / `behind:<N` / `behind:=N`（基于 `git2::graph_ahead_behind`，**优先读 health 缓存**）
  - `tag:<tag>`（标签**精确匹配**，不再误匹配子串）
  - `note:<text>`（搜索学习笔记）
  - `semantic:<text>`（基于 README 关键词的语义搜索）
  - 无 key 关键词（回退到 id/path/tags LIKE）
- 多个条件以 AND 组合。
- 新增 `run_json` 返回结构化 JSON，每次查询自动记录到 `ai_queries`。
- `compute_behind` 增加 `origin/HEAD` fallback，避免从未 fetch 的仓库漏结果。

#### `src/sync.rs`
- 新增 `--filter-tags` CLI 参数（OR 逻辑）。
- 新增 `SyncOrchestrator`：
  - `SyncMode::SYNC`：顺序执行
  - `SyncMode::ASYNC` / `SyncMode::BlockUi`：`tokio::spawn` + `Semaphore` 限制并发度为 4
  - 支持 `on_progress` 回调，供 TUI 实时更新
- 新增 `classify_sync_error`：将技术错误映射为 `network-error`、`auth-failed`、`conflict`、`blocked-dirty`、`error`。
- `sync_repo` 改为内部错误降级模式：所有失败均返回 `Ok(SyncSummary { action: "ERROR", error_kind: Some(...), ... })`，不再中断批量同步。
- 同步结束后打印 90 字符宽汇总表格（含 `error_kind`）。
- `run_json` 返回 MCP 契约 JSON。

#### `src/tui.rs`
- 事件循环重构：从阻塞 `event::read()` 改为 `event::poll(Duration::from_millis(50))` + `async_rx.try_recv()` 非阻塞轮询。
- 集成 `AsyncSingleJob`：
  - 切换选中项时自动后台获取 `RepoStatus`
  - 按键 `s` 触发异步 `FetchPreview`（不再卡顿）
  - 按键 `S`（大写）触发批量同步（通过 `SyncOrchestrator`）
- 仓库列表增加 inline spinner：`loading_repo_status` / `loading_preview` 中的仓库显示 `⏳` 前缀 + 青色高亮。
- Details 面板新增 `Language` 和 `Status`（dirty/ahead/behind）显示。
- **批量同步弹窗**：按键 `S` 后弹出居中模态窗口（60%×40%），实时显示各仓库同步进度，成功项绿色、错误项红色，`Esc`/`Enter` 关闭。
- Logs 面板保留彩色分级（INFO 绿色 / WARN 黄色 / ERROR 红色粗体）。
- 按键 `h` 帮助条、`t` 标签编辑、`Home/End` 快速跳转均保留。
- 已适配 `repo_tags` 新 schema：`RepoItem.tags` 改为 `Vec<String>`，过滤使用精确匹配。

#### `src/asyncgit.rs`（新增）
- 借鉴 `gitui` 的 `AsyncSingleJob` 模式，实现轻量异步任务基础设施。
- `AsyncRepoStatus`：后台检测 dirty / ahead / behind。
- `AsyncFetchPreview`：后台执行 fetch + ahead/behind 计算。
- `AsyncSyncProgress`：供 `SyncOrchestrator` 向 TUI 汇报批量同步进度。
- 通过 `crossbeam_channel::Sender<AsyncNotification>` 与 TUI 主循环通信。

#### `src/registry.rs`
- Schema 升级为多表：
  - `repos`：`id`, `local_path`, `language`, `discovered_at`
  - `repo_remotes`：`repo_id`, `remote_name`, `upstream_url`, `default_branch`, `last_sync`（支持 1:N）
  - `repo_health`：`repo_id`, `status`, `ahead`, `behind`, `checked_at`
  - `repo_tags`：`repo_id`, `tag`（关联表，带 `idx_repo_tags_tag` 索引）
  - `repo_summaries` / `repo_modules` / `repo_relations` / `ai_queries` / `ai_discoveries` / `repo_notes`
- 兼容迁移：
  - 启动时检测旧表 `repos` 是否包含 `upstream_url` 列
  - 若存在，重命名为 `repos_legacy`，并将数据导入新表
  - 检测 `repos` 是否仍有 `tags` CSV 列，如有则拆分导入 `repo_tags`，随后 `DROP COLUMN tags`
  - 已有 22 个仓库数据完整迁移，无丢失
- 新增 `RepoEntry::primary_remote()` 辅助方法（优先返回 `origin`，否则第一个 remote）。
- 新增数据访问方法：`list_repos`、`save_repo`、`save_health`、`get_health`。
- 新增增量查询方法：`list_repos_stale_health`、`list_repos_need_index`。
- `save_repo` 写入 tags 时通过事务先清空 `repo_tags` 再逐条插入。

#### `src/knowledge_engine.rs`（新增）
- `extract_readme_summary(path)`：提取首段摘要 + TF 关键词（规则模式，LLM 降级方案）。
- `extract_module_structure(path)`：对 Rust 项目调用 `cargo metadata` 解析模块结构。
- `run_index(path)`：遍历所有注册仓库并写入 `repo_summaries` / `repo_modules`。
- `index_repo(repo)`：单个仓库索引，供 Daemon 增量化使用。

#### `src/discovery_engine.rs`（新增）
- `discover_dependencies(repos)`：解析 `Cargo.toml` / `package.json` / `go.mod`，发现本地仓库间的依赖关系。
- `discover_similar_projects(conn)`：基于 `repo_summaries.keywords` 的 Jaccard 相似度计算。
- 结果写入 `repo_relations` 和 `ai_discoveries`。

#### `src/digest.rs`（新增）
- `generate_daily_digest(conn, config)`：聚合过去 N 小时（可配置）的新仓库、异常仓库、新发现，生成 AI/人类可读的日报。

#### `src/daemon.rs`（新增后优化）
- 实现自动化 tick 闭环：health check → re-index → discovery → digest。
- **增量策略**：
  - health 只处理 `checked_at` 超过 `health_stale_hours` 的仓库
  - re-index 只处理超过 24 小时未索引的仓库
  - 可通过 `config.daemon.incremental = false` 回退到全量模式
- 所有阻塞操作（SQLite、git2、文件系统）均包裹在 `tokio::task::spawn_blocking` 中。

#### `src/sync_protocol.rs`（新增）
- 借鉴 `syncthing` 的块索引与版本向量模型：
  - `VersionVector`：`update(local_id)` / `merge(other)` / `compare(other)`
  - `FileInfo`：name, size, mod_time, version, blocks_hash
  - `SyncIndex`：path + files 列表
- `scan_directory(path)`：使用 `walkdir` 遍历并生成轻量索引（跳过 `.git`）。

#### `src/watch.rs`（新增）
- 借鉴 `syncthing` 三层监控模型：
  - 底层 `FsWatcher`：封装 `notify = "7"` crate（Windows 使用 `ReadDirectoryChangesW`）
  - 聚合层 `WatchAggregator`：去重、事件数阈值判断（`max_files` 可配置）
  - 调度层 `FolderScheduler`：对比新旧 `SyncIndex`，生成 `SyncAction::Scan` 或 `SyncAction::Sync`
- CLI 暴露：`devbase watch <path> --duration <seconds>`

#### `src/syncthing_client.rs`（新增）
- 轻量 `reqwest` HTTP 客户端，封装 Syncthing REST API：
  - `POST /rest/config/folders`：动态创建/更新 folder（最小字段 `id` + `path`）
  - `GET /rest/db/status?folder=<id>`：查询 folder 同步状态
- 支持 `X-API-Key` 认证头。
- CLI 暴露：`devbase syncthing-push [--api-url] [--api-key] [--filter-tags]`
- 连接失败时打印中文友好提示。

#### `src/config.rs`（新增）
- 基于 `serde` + `TOML` 的配置系统。
- 默认路径：`~/.config/devbase/config.toml`。
- 支持 `daemon` / `cache` / `watch` / `digest` 四个配置段。
- 配置文件不存在时安全回退到 `Config::default()`。

#### `Cargo.toml`
- 新增/保留依赖：`clap`, `git2`, `tokio`, `serde`, `toml`, `tracing`, `walkdir`, `anyhow`, `serde_json`, `reqwest`, `dirs`, `chrono`, `rusqlite`, `ratatui`, `crossterm`, `crossbeam-channel`, `notify`
- 删除未使用依赖：`tokio-serde`、`async-trait`
- `tokio` 显式声明 `"time"` feature
- 新增 release profile：`lto = true`, `codegen-units = 1`

#### `src/main.rs`
- 新增模块声明：`mod mcp; mod asyncgit; mod sync_protocol; mod watch; mod syncthing_client; mod knowledge_engine; mod discovery_engine; mod digest; mod daemon; mod config;`
- 新增子命令：`Mcp`、`Watch`、`SyncthingPush`、`Index`、`Discover`、`Digest`、`Daemon`
- 所有命令入口均加载 `Config::load()` 并传递给下层模块。

---

## 三、新增仓库记录

| 仓库 | 路径 | 语言 | 上游 | Tags |
|------|------|------|------|------|
| AutoCLI | `dev\third_party\AutoCLI` | Rust | `https://github.com/nashsu/AutoCLI.git` | `third-party,reference` |

当前 Registry 中总仓库数：**22**

---

## 四、编译与测试

### 编译结果
```bash
cargo check         # ✅ 通过（1 个无关 unused warning）
cargo build --release  # ✅ 通过
cargo test          # ✅ 25 个测试全部通过，2 个 ignored
```

### 测试覆盖
- `scan::tests`：12 个（语言检测、ZIP 快照标签、嵌套子模块判断）
- `mcp::tests`：7 个（initialize、tools/list、stdio 格式、devkit_health、devkit_query、未知工具、未知方法）
- `knowledge_engine::tests`：5 个（摘要提取、模块结构解析）

---

## 五、端到端运行验证

### 5.1 MCP Server
```bash
devbase mcp --transport stdio
# 输入 initialize + tools/list + tools/call(devkit_health)
# → 返回结构正确，JSON 符合契约，stdio 输出含 Content-Length 头
```

### 5.2 Health（含缓存预热）
```bash
# 第 1 次（冷启动）
devbase health --detail
# → total_repos=22, dirty_repos=4, behind_upstream=0
# → 耗时约 21s（逐个 git2::Repository::open）

# 第 2 次（热缓存，5 分钟 TTL）
devbase health --detail
# → 耗时约 1.6s，提速约 13 倍
```

### 5.3 Query
```bash
devbase query "lang:rust"
# → 返回 7 个 Rust 项目，含 match 原因提示

devbase query "tag:discovered"
# → 返回 17 条精确匹配结果（不再误匹配子串）

devbase query "tag:rust"
# → 返回 No repositories matched（证明精确匹配生效）

devbase query "behind:>0"
# → 直接读取 health 缓存，0.24s 完成，无匹配
```

### 5.4 Sync
```bash
devbase sync --dry-run --filter-tags third-party
# → SyncOrchestrator ASYNC 模式并发执行
# → 汇总表格正常，无 panic
```

### 5.5 Watch
```bash
devbase watch . --duration 3
# → 空跑 3 秒正常结束，无 panic
```

### 5.6 Syncthing Push
```bash
devbase syncthing-push --filter-tags third-party
# → Syncthing 未运行时：友好提示 "无法连接到 Syncthing API..."
# → 无 panic
```

### 5.7 TUI
```bash
devbase tui
# → 键盘导航正常，切换仓库自动触发后台状态获取
# → `s` 异步 preview 不阻塞，`S` 弹出批量同步进度弹窗
# → `t` 标签编辑、`Esc` / `Enter` 可关闭弹窗
```

### 5.8 Daemon
```bash
devbase daemon --interval 5
# → 稳定运行多个 tick，health 和 re-index 均显示 0 stale（增量生效）
# → digest 正常生成并输出到日志
```

### 5.9 Digest
```bash
devbase digest
# → 正常输出知识日报，含新仓库、异常仓库、新发现、总体统计
# → 支持通过 config.toml 调整 window_hours
```

---

## 六、外部集成探索结论

### 6.1 Clarity MCP 集成
- **现状**：`clarity-core` 已实现完整的 `McpClient` 和 TOML 配置解析。
- **阻塞点**：`clarity-tui` 的 `request_completion()` 目前仍是 placeholder，**尚未接入 MCP 工具调用链**。配置文件中添加 `[mcp_servers.devbase]` 后暂时不会生效。
- **结论**：devbase 的 MCP Server 端已就绪（含 7 个工具、Content-Length 格式、安全 JSON 错误响应），但宿主侧（Clarity TUI）需要完成 Agent 循环改造后，才能真正实现端到端集成测试。

### 6.2 Syncthing REST API 对接
- **接口已明确**：
  - `POST /rest/config/folders` 最小只需 `id` + `path`
  - `GET /rest/db/status?folder=<id>` 可查询状态
- **实现状态**：`src/syncthing_client.rs` 和 `devbase syncthing-push` 已完成，可直接向本地 Syncthing 实例动态推送 folder。

---

## 七、已知问题与限制

1. **Clarity 集成未闭环**：需等待 Clarity TUI 层完成 LLM + MCP 桥接。
2. **LLM 语义提取降级**：因本地 Ollama 安装被网络限制阻挡，README 摘要和 `semantic:` 查询使用规则提取（首段 + TF 关键词），质量不如 LLM 生成。
3. **TUI 批量同步弹窗**：当前弹窗通过 `List` 显示进度，后续可进一步增加进度条百分比或取消按钮。
4. **VersionVector 预留方法未使用**：`sync_protocol.rs` 中的方法尚无调用方，待与 Syncthing 版本向量对齐后使用。
5. **FolderScheduler::new 未使用 warning**：`watch.rs` 中保留的旧 API，不影响运行。

---

## 八、第三方项目可参照性结论

| 项目 | 实际借鉴点 | 落地状态 |
|------|-----------|---------|
| gitui | `AsyncSingleJob` + channel 通知 + 显式组件组合 | ✅ `asyncgit.rs` / `tui.rs` |
| lazygit | `RefreshHelper` 模式、`CheckMergeOrRebase` 降级、`InlineStatus` | ✅ `sync.rs` / `tui.rs` |
| syncthing | 块级索引两层结构、版本向量、监控三层解耦 | ✅ `sync_protocol.rs` / `watch.rs` / `syncthing_client.rs` |
| kimi-cli | `wire/` 协议解耦、AGENTS.md 注入、三文件持久化 | 部分借鉴（MCP 协议层设计思路） |
| codex / claude-code-rust | MCP Server stdio 循环、工具 crate 化 | ✅ `mcp.rs` |
| desktop | `RepositoriesStore` + `GitStore` 分层、多表元数据 | ✅ `registry.rs` |
| iroh | 模块化协议栈 | 未直接落地，留待 P2P 层设计参考 |

---

## 九、下一步建议

1. **Clarity 宿主集成**：待 Clarity TUI 层完成 LLM + MCP 桥接后，在 `~/.config/clarity/config.toml` 中添加 `[mcp_servers.devbase]` 配置，进行真实端到端测试。
2. **SQLite 连接池**：引入 `deadpool-sqlite` 或 `r2d2_sqlite`，消除每个模块独立 `init_db()` 新建连接的反模式。
3. **TUI 弹窗增强**：为批量同步弹窗增加进度百分比、取消按钮、或结果导出功能。
4. **Syncthing 实战对接**：启动本地 Syncthing 实例（默认 `http://127.0.0.1:8384`），运行 `devbase syncthing-push --filter-tags third-party`，验证 folder 动态创建和状态查询。
5. **LLM 语义提取升级**：网络恢复后接入 Ollama，将规则摘要替换为 LLM 生成，并将 `semantic:` 查询升级为向量相似度搜索。
6. **CI / 发布流程**：引入 GitHub Actions，自动化 `cargo test`、clippy、release binary 构建。

---

**报告生成人**：Kimi Code CLI (devbase 优化轮次)  
**报告位置**：`C:\Users\22414\Desktop\devbase\STAGE_REPORT_2026-04-09.md`
