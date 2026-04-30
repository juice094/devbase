# devbase Hard Veto 方向性审计报告
日期: 2026-04-26
版本: v0.13.0 (Schema v25)
审计依据: 工作区 `AGENTS.md` (V3.1-EP) + 项目 `AGENTS.md` (RF-1~RF-6)

---

## 一、Veto 框架对照

| Veto / 红线 | 来源 | 核心要求 |
|-------------|------|---------|
| **HV-1 闭源/云端强制/数据外泄** | 工作区 AGENTS.md | 禁止数据外泄；禁止强制依赖云端服务 |
| **HV-2 Docker/Qdrant/Electron** | 工作区 AGENTS.md | 禁止容器化、专用向量DB、Electron GUI |
| **HV-3 项目广度 > 5 核心工具** | 工作区 AGENTS.md | 控制核心子系统数量 |
| **HV-4 本地 LLM 优先** | 工作区 AGENTS.md | 优先本地推理，减少外部 LLM 依赖 |
| **HV-5 Rust 核心不可外包** | 工作区 AGENTS.md | 核心模块修改必须由 root agent 执行 |
| **RF-1 依赖注入 > 全局状态** | 项目 AGENTS.md | 禁止新增全局可变状态 |
| **RF-2 测试密封性** | 项目 AGENTS.md | 测试不修改全局进程状态 |
| **RF-3 Schema 单一事实来源** | 项目 AGENTS.md | DDL 与 migrate 原子同步 |
| **RF-4 入口限界** | 项目 AGENTS.md | `main.rs` ≤ 1000 行 |
| **RF-5 无循环依赖** | 项目 AGENTS.md | 禁止模块间双向 `use` |
| **RF-6 生产代码无 panic** | 项目 AGENTS.md | 禁止 `unwrap/expect/panic` |

---

## 二、触碰项（🔴 需 HALT 或立即修复）

### 🔴 T1: `knowledge_engine.rs` — 自动发送 README 到外部 LLM API（数据外泄风险）

**代码位置**: `src/knowledge_engine.rs:667-705` (`index_repo`) / `src/knowledge_engine.rs:707-759` (`run_index`)

**行为**:
```rust
let (summary, keywords) = config
    .and_then(|cfg| try_llm_summary(&repo.local_path, &cfg.llm))  // 自动调用
    .or_else(|| extract_readme_summary(...))
```

`try_llm_summary` 读取 repo 的 README（最多 3000 字符）或 manifest，通过 `reqwest` POST 到外部 LLM API：
- DeepSeek (`api.deepseek.com`)
- Kimi (`api.moonshot.cn`)
- OpenAI (`api.openai.com`)
- DashScope (`dashscope.aliyuncs.com`)

**触碰 Veto**: HV-1（数据外泄）

**风险分析**:
1. 虽然需要用户配置 `api_key`，但 `index_repo` 是**自动触发**的（`devbase index` / daemon 后台扫描），不是用户显式选择的发送操作
2. README 可能包含内部项目信息、用户名、内部 URL、凭证占位符等敏感内容
3. 项目 AGENTS.md 明确声明"代码内容不会被上传到任何云端服务（除非用户显式配置 GitHub token 用于 stars 查询）"——LLM API 调用超出了已记录的例外范围
4. **默认提供商列表全部为云端闭源 API**，与 HV-4 "本地 LLM 优先"直接冲突

**建议**:
- **立即**: 添加显式用户确认（`--llm-summary` flag 或交互式确认）
- **短期**: 默认关闭 `llm.enabled`，改为本地摘要提取（README 前 N 行）
- **中期**: 接入本地 LLM（Ollama / llama.cpp）作为默认提供商，云端仅作 fallback
- **长期**: 将 LLM 摘要迁移到 v0.14 的 candle 本地 embedding 管道（纯本地，零外泄）

---

### 🔴 T2: `i18n/mod.rs` — 全局可变状态（触碰 RF-1）

**代码位置**: `src/i18n/mod.rs:204-208`

```rust
let _ = CURRENT.set(i18n);
pub fn current() -> &'static I18n {
    CURRENT.get().expect("i18n not initialized")
}
```

**触碰 Veto**: RF-1（依赖注入优于全局状态）

**风险分析**:
- `CURRENT` 是全局 `OnceLock<I18n>`，在 `initialize()` 中设置后不可更改
- 虽然 `OnceLock` 是线程安全的，但它仍然是全局状态
- RF-1 明文禁止"新增 `dirs::data_local_dir()` / `std::env::var_os` 硬编码路径"以外的全局状态，且 grandfathered 仅限 3 处（`backup_dir`, `db_path`, `index_path`）
- `i18n` 全局状态不在 grandfathered 列表中

**建议**:
- 将 `I18n` 注入 `AppContext`，通过 `ctx.i18n()` 访问
- 或降级为 `thread_local` + 构造函数注入

---

## 三、黄灯项（🟡 需关注，列入技术债）

### 🟡 Y1: 生产代码 `expect()` — RF-6 触碰（多处）

RF-6 要求"生产代码禁止 `unwrap()`、`expect()`、`panic!()`"。Wave 30 声称已完成 unwrap 清零，但 `expect()` 仍在生产代码中：

| 文件 | 行 | 代码 | 风险等级 |
|------|-----|------|---------|
| `discovery_engine.rs` | 178-179 | `keywords_map.get(a).expect(...)` | 中（Map 内部状态可能不一致） |
| `query.rs` | 22 | `value.chars().next().expect(...)` | 低（前置检查存在，但非类型级保证） |
| `search.rs` | 88-96,114,158-165,186 | `schema.get_field(...).expect(...)` | 低（schema 初始化时定义） |
| `search/hybrid.rs` | 91,156 | `into_iter().next().expect(...)` | 低（前置 `len==1` 检查） |
| `skill_runtime/parser.rs` | 143,155,187,192 | `current_input.take().expect(...)` | 中（解析状态机内部不变量） |
| `sync/orchestrator.rs` | 72,125 | `semaphore.try_acquire().expect(...)` | **高**（semaphore 可能在 shutdown 时被关闭） |
| `workflow/scheduler.rs` | 16,33,34,40 | topo sort 内部 expect | 低（算法不变量） |
| `workflow/interpolate.rs` | 9,23,24 | regex / capture group expect | 低（静态正则） |

**建议**:
- `sync/orchestrator.rs` 的 semaphore expect 应立即改为 `?` 传播或 `match` 处理（shutdown 场景会 panic）
- 其余 `expect()` 按优先级逐步替换为 `ok_or` + `?` 或 `if let`

### 🟡 Y2: `embedding.rs` — 硬编码个人环境路径

**代码位置**: `src/embedding.rs:92-94`

```rust
std::path::PathBuf::from(
    "C:\\Users\\22414\\AppData\\Roaming\\uv\\tools\\pip\\Scripts\\python.exe",
),
```

**问题**:
- 生产代码中硬编码开发者个人路径
- 在其他 Windows 机器上完全不可用
- 泄露开发者用户名 `22414`

**建议**:
- 移除硬编码路径，依赖 `PATH` 中的 `python` / `python3`
- v0.14 后用 candle 本地 embedding 完全替代 Python 回退，消除此路径

### 🟡 Y3: `project_context` — 返回 repo 绝对路径

**代码位置**: `src/mcp/tools/context.rs`

`project_context` 返回的 JSON 包含 `repo.path`，该路径通常是绝对路径（如 `C:\Users\22414\dev\third_party\devbase`）。通过 MCP 暴露给外部 LLM，导致：
1. 用户目录结构泄露
2. 用户名泄露

**建议**:
- 路径脱敏：将 `dirs::home_dir()` 前缀替换为 `~`
- 或返回相对路径（相对于 workspace root）

### 🟡 Y4: `knowledge_engine.rs` — 默认云端 LLM 提供商

`try_llm_summary` 的默认提供商列表全部为云端 API（DeepSeek/Kimi/OpenAI/DashScope），无本地 LLM 默认选项。

**触碰 Veto**: HV-4（本地 LLM 优先）

**建议**:
- 添加 `ollama` 作为默认本地提供商
- 或默认 `enabled = false`，强制用户显式选择提供商

---

## 四、绿灯项（✅ 通过）

| 检查项 | 结论 | 说明 |
|--------|------|------|
| HV-2 Docker/Qdrant/Electron | ✅ | 使用 Tantivy + ratatui，无触碰 |
| HV-3 项目广度 | ✅ | 38 tools / 22 modules 为功能数量，非"核心外部系统"；7 核心子系统略超 5，但属历史形成，v0.16 评估拆分 |
| HV-5 Rust 核心外包 | ✅ | 审计阶段使用只读 subagent，代码修改由 root agent 执行 |
| RF-2 测试密封性 | ✅ | `tempfile` + `DEVBASE_DATA_DIR` + `SEARCH_TEST_LOCK` |
| RF-3 Schema SSOT | ✅ | v25 的 `agent_symbol_reads` 在 `SCHEMA_DDL` 和 `migrate.rs` 中同步存在 |
| RF-4 入口限界 | ✅ | `main.rs` 515 行 < 1000 |
| RF-5 无循环依赖 | ✅ | 无编译期双向 `use` |
| `reqwest` 网络调用 | ✅ | arXiv/GitHub/Syncthing 均为用户显式触发；无后台静默上传 |
| `cargo audit` | ✅ | 0 漏洞（除上游 `tokei` 的 `RUSTSEC-2020-0163`） |
| 凭证管理 | ✅ | `config.toml` 在用户配置目录，不在项目目录；`.gitignore` 覆盖完整 |
| Schema 迁移安全 | ✅ | 每次迁移前自动 `backup::auto_backup_before_migration()` |

---

## 五、裁决与行动

### 需立即行动（本周内）

| 优先级 | 事项 | 文件 | 预估 |
|--------|------|------|------|
| P0 | `knowledge_engine.rs` 默认关闭 LLM 摘要，改为本地 README 提取 | `knowledge_engine.rs` | 1h |
| P0 | `knowledge_engine.rs` 添加 Ollama 本地提供商作为默认选项 | `knowledge_engine.rs` | 2h |
| P1 | `i18n/mod.rs` 全局状态改为注入式 | `i18n/mod.rs` + `AppContext` | 2h |
| P1 | `sync/orchestrator.rs` semaphore expect 改为安全处理 | `sync/orchestrator.rs` | 30min |
| P1 | `project_context` 返回路径脱敏 | `mcp/tools/context.rs` | 30min |
| P2 | `embedding.rs` 移除硬编码 Python 路径 | `embedding.rs` | 15min |
| P2 | 其余生产代码 `expect()` 逐步清零 | 8 个文件 | 1 天 |

### 长期方向修正

- **HV-1 数据外泄**: v0.14 的 candle 本地 embedding 落地后，完全消除 Python/Ollama/云端 API 依赖
- **HV-4 本地 LLM 优先**: `knowledge_engine` 的 LLM 摘要功能应评估是否值得维护；若价值有限，考虑移除以简化架构

---

## 六、Veto 状态总览

```
HV-1 数据外泄    🔴 触碰 (knowledge_engine LLM 自动上传)
HV-2 Docker/...  ✅ 通过
HV-3 项目广度    🟡 关注 (7 核心子系统 > 5，历史债务)
HV-4 本地 LLM    🟡 关注 (默认云端提供商，无本地默认)
HV-5 Rust 外包   ✅ 通过
RF-1 全局状态    🔴 触碰 (i18n CURRENT)
RF-2 测试密封    ✅ 通过
RF-3 Schema SSOT ✅ 通过
RF-4 入口限界    ✅ 通过
RF-5 无循环      ✅ 通过
RF-6 无 panic    🟡 触碰 (17 处生产 expect，1 处高风险)
```
