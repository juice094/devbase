# devbase 代码健康度与耦合度报告
日期: 2026-04-26
版本: v0.13.0 (Schema v25)
范围: 22,583 行生产代码 / 6,234 行测试代码 / 77 个 .rs 文件

---

## 一、健康度总览

| 指标 | 当前值 | 阈值 | 状态 |
|------|--------|------|------|
| 测试代码占比 | 21.6% | ≥30% | 🟡 偏低 |
| 零测试文件数 | 20 / 77 | 0 | 🔴 偏高 |
| 生产代码 >500 行文件 | 12 | ≤5 | 🔴 超标 |
| 函数 >100 行 | 28 | ≤10 | 🔴 超标 |
| 生产代码 unwrap | 0 | 0 | ✅ 达标 |
| unsafe 块 | 0 | 0 | ✅ 达标 |
| clippy warnings | 0 | 0 | ✅ 达标 |
| `cargo audit` 漏洞 | 0 | 0 | ✅ 达标 |

**基线**: 22.6k 生产行 / 6.2k 测试行 = **3.6:1 生产测试比**

---

## 二、文件规模分布

### 巨石文件（>500 行生产代码）

| 排名 | 文件 | 总行 | 生产 | 测试 | 测试率 | 风险 |
|------|------|------|------|------|--------|------|
| 1 | `tui/state.rs` | 1298 | 1239 | 59 | 4.5% | 🔴 职责过多（渲染+状态+事件+输入） |
| 2 | `registry/migrate.rs` | 1273 | 1253 | 20 | 1.6% | 🔴 25 个 schema 版本内联 |
| 3 | `semantic_index.rs` | 1133 | 794 | 339 | 29.9% | 🟡 AST 提取+索引写入+查询 |
| 4 | `knowledge_engine.rs` | 1029 | 818 | 211 | 20.5% | 🟡 多语言 manifest 解析+LLM 调用 |
| 5 | `skill_runtime/discover.rs` | 950 | 881 | 69 | 7.3% | 🟡 项目检测+分类推断+SKILL.md 生成 |
| 6 | `workflow/executor.rs` | 866 | 487 | 379 | 43.8% | 🟡 5 种 step 类型执行逻辑 |
| 7 | `registry/knowledge.rs` | 829 | 570 | 259 | 31.2% | 🟡 21 个 pub fn，CRUD 聚合 |
| 8 | `dependency_graph.rs` | 827 | 628 | 199 | 24.1% | 🟡 5 语言 manifest 解析 |
| 9 | `query.rs` | 750 | 598 | 152 | 20.3% | 🟡 查询求值+vault/repo 聚合 |
| 10 | `scan.rs` | 743 | 494 | 249 | 33.5% | 🟡 仓库检测+语言识别+标签推断 |
| 11 | `mcp/mod.rs` | 708 | 705 | 3 | 0.4% | 🔴 37-variant enum + tool 分发 |
| 12 | `commands/simple.rs` | 647 | 647 | 0 | 0% | 🔴 零测试，22 个命令聚合 |

### 零测试文件（20 个）

| 文件 | 行数 | 说明 |
|------|------|------|
| `commands/simple.rs` | 647 | 22 个 CLI 命令 |
| `sync/tasks.rs` | 622 | 同步任务执行 |
| `tui/event.rs` | 413 | 事件映射+渲染回调 |
| `commands/skill.rs` | 402 | Skill CLI 子命令 |
| `sync/orchestrator.rs` | 158 | 并发同步调度 |
| `daemon.rs` | 146 | 后台守护进程 tick |
| `workflow/scheduler.rs` | 138 | 拓扑排序+batch 调度 |
| `commands/workflow.rs` | 137 | Workflow CLI 子命令 |
| `commands/limit.rs` | 112 | KnownLimit CLI |
| `i18n/en.rs` | 137 | 英文翻译表 |
| `i18n/zh_cn.rs` | 137 | 中文翻译表 |
| `workflow/validator.rs` | 100 | Workflow YAML 校验 |
| `vault/backlinks.rs` | 95 | 反向链接解析 |
| `vault/wikilink.rs` | 85 | WikiLink 语法解析 |
| `skill_sync.rs` | 80 | Skill 同步到 clarity |
| `syncthing_client.rs` | 74 | Syncthing REST 客户端 |
| `sync_protocol.rs` | 72 | 同步协议（死代码） |
| `core/mod.rs` | 12 | 完全未使用 |
| `core/node.rs` | 55 | 完全未使用 |
| `watch.rs` | 248 | 文件监控调度 |

---

## 三、函数规模分布

### 超长函数（>100 行，28 个）

| 排名 | 函数 | 文件 | 行数 | 问题 |
|------|------|------|------|------|
| 1 | `run_skill` | `commands/skill.rs` | 399 | CLI 命令+skill 执行+结果格式化全混 |
| 2 | `run_json` | `query.rs` | 341 | 查询解析+多种 repo 来源+过滤+排序 |
| 3 | `run_json` | `health.rs` | 198 | 健康检查+环境探测+表格输出 |
| 4 | `generate_report` | `oplog_analytics.rs` | 192 | 报表生成+多表 JOIN+格式化 |
| 5 | `run_stdio` | `mcp/mod.rs` | 173 | MCP 协议循环+tool 分发+错误处理 |
| 6 | `run_skill` | `skill_runtime/executor.rs` | 155 | Skill 进程执行+超时+输出捕获 |
| 7 | `sync_repo` | `sync/tasks.rs` | 150 | 同步策略判断+git 操作+冲突处理 |
| 8 | `parse_skill_frontmatter` | `skill_runtime/parser.rs` | 148 | YAML 解析+状态机+输入输出匹配 |
| 9 | `tick` | `daemon.rs` | 146 | 定时任务调度+健康检查+索引 |
| 10 | `build` | `i18n/zh_cn.rs` | 137 | 纯数据表，非逻辑函数（可接受） |
| 11 | `build` | `i18n/en.rs` | 137 | 同上 |
| 12 | `discover_and_install` | `skill_runtime/discover.rs` | 136 | 项目检测+封装+注册+安装 |
| 13 | `discover_dependencies` | `discovery_engine.rs` | 133 | 多语言依赖发现+图构建 |
| 14 | `run` | `sync.rs` | 127 | 同步命令+并发控制+进度输出 |
| 15 | `extract_keywords` | `knowledge_engine.rs` | 124 | 关键词提取+去重+截断 |
| 16 | `run_vault` | `commands/simple.rs` | 122 | vault 操作分发+格式化 |
| 17 | `generate_daily_digest` | `digest.rs` | 121 | 日报生成+多表聚合+输出 |
| 18 | `run_syncthing_push` | `commands/simple.rs` | 115 | Syncthing API 调用+错误处理 |
| 19 | `infer_category` | `skill_runtime/discover.rs` | 115 | 规则引擎+分类推断 |
| 20 | `deserialize` | `workflow/model.rs` | 114 | 自定义反序列化+默认值填充 |
| 21 | `execute_loop_step` | `workflow/executor.rs` | 114 | Loop 步骤执行+变量绑定+递归 |
| 22 | `run_workflow` | `commands/simple.rs` | 110 | workflow 运行+参数解析+输出 |
| 23 | `main` | `main.rs` | 110 | CLI 入口+命令分发（515 行文件中） |
| 24 | `run_index` | `knowledge_engine.rs` | 104 | 索引批量处理+Tantivy 写入 |

---

## 四、耦合度分析

### 4.1 模块依赖热力图（简化）

```
                  ┌─────────────────┐
                  │   main.rs (CLI)  │
                  └────────┬────────┘
                           │
        ┌──────────────────┼──────────────────┐
        │                  │                  │
   ┌────▼────┐       ┌────▼────┐      ┌─────▼─────┐
   │commands/│       │  mcp/   │      │   tui/    │
   │(22 cmd) │       │(37 tools)│     │(render+state)
   └────┬────┘       └────┬────┘      └─────┬─────┘
        │                 │                 │
        └─────────────────┼─────────────────┘
                          │
                   ┌──────▼──────┐
                   │ AppContext  │◄──────┐
                   │ (God Obj)   │       │
                   └──────┬──────┘       │
                          │              │
                   ┌──────▼──────┐      │
                   │WorkspaceReg.│◄─────┘  ←──── 15+ 模块依赖
                   │ (God Obj)   │
                   └──────┬──────┘
                          │
        ┌─────────────────┼─────────────────┐
        │                 │                 │
   ┌────▼────┐      ┌────▼────┐      ┌────▼────┐
   │registry/│      │ search  │      │ storage │
   │(15 文件)│      │(tantivy)│      │(SQLite) │
   └─────────┘      └─────────┘      └─────────┘
```

### 4.2 God Object 耦合矩阵

**WorkspaceRegistry** (`src/registry.rs` + `src/registry/*.rs`):

| 引用方 | 引用次数 | 用途 |
|--------|---------|------|
| `commands/simple.rs` | 16 | 几乎所有 CLI 命令 |
| `health.rs` | 8 | 健康查询+状态输出 |
| `tui/state.rs` | 8 | TUI 数据加载+事件处理 |
| `daemon.rs` | 8 | 定时索引+健康检查 |
| `backup.rs` | 7 | 备份+恢复 |
| `skill_runtime/registry.rs` | 7 | Skill CRUD+搜索 |
| `scan.rs` | 7 | 扫描结果保存 |
| `digest.rs` | 6 | 日报生成查询 |
| `vault/scanner.rs` | 6 | Vault 扫描入库 |
| `query.rs` | 11 | 查询求值 |
| `knowledge_engine.rs` | 10 | 摘要/关键词保存 |
| `registry/*.rs` 内部 | 150+ | 定义文件自身引用 |

**结论**: WorkspaceRegistry 被 **15+ 外部模块** 依赖，是典型的 God Object。任何 registry 内部修改都有编译级联风险。

**AppContext** (`src/storage.rs`):

| 引用方 | 引用次数 | 用途 |
|--------|---------|------|
| `commands/simple.rs` | 19 | 所有命令获取 DB pool + 配置 |
| `mcp/mod.rs` | 6 | MCP tool 获取 context |
| `tui/state.rs` | 2 | TUI 获取配置+DB |
| `storage.rs` 自身 | 2 | 定义 |

**结论**: AppContext 虽然引用次数少于 WorkspaceRegistry，但它是**所有命令和 MCP tools 的上下文根**，传递路径更长。

### 4.3 模块 fan-in / fan-out

**高 Fan-out（依赖多个模块）**:

| 模块 | 外部依赖数 | 依赖列表 |
|------|-----------|---------|
| `vault/indexer` | 3 | registry, search, vault |
| `tui/state` | 2 | asyncgit, registry |
| `vault/scanner` | 2 | vault::frontmatter, vault::wikilink |

**结论**: 整体 fan-out 较低（最大 3），说明模块间调用不密集。但这掩盖了 WorkspaceRegistry 的集中耦合——大部分模块只依赖 registry，registry 内部再扩散。

**高 Fan-in（被多个模块依赖）**:

| 模块 | 被引用数 | 引用方 |
|------|---------|--------|
| `registry` | 21 | 几乎所有业务模块 |
| `storage` | 3 | backup, search, migrate |
| `vault::frontmatter` | 2 | skill_sync, vault/scanner |

---

## 五、公共 API 表面

### 5.1 `lib.rs` 过度暴露

`src/lib.rs` re-export **32 个 pub mod**，包括内部实现模块：
- `registry/test_helpers` — 仅测试使用
- `mcp/tests` — 仅测试使用
- `core/mod`, `core/node` — 完全未使用
- `sync_protocol` — 仅测试使用，且标记为死代码

**建议**: 降级为 `pub(crate)` 的候选模块:
- `registry/test_helpers`, `registry/tests`
- `mcp/tests`
- `sync/tests`
- `core/*`
- `sync_protocol`
- `symbol_links`

### 5.2 模块内部 pub 项过多

| 模块 | pub 项总数 | pub fn | pub struct | 说明 |
|------|-----------|--------|-----------|------|
| `i18n/mod` | 23 | 16 | 5 | 翻译函数全部 pub |
| `skill_runtime/mod` | 22 | 6 | 6 | 8 个子模块全部 pub |
| `registry/knowledge` | 21 | 21 | 0 | 纯函数 CRUD 层 |
| `registry` (root) | 17 | 3 | 10 | WorkspaceRegistry + 子 registry re-export |
| `config` | 17 | 5 | 12 | 配置结构体全部 pub |
| `mcp/mod` | 14 | 6 | 2 | 37-variant enum + tool trait |

---

## 六、重复代码热点

### 6.1 MCP Tool Schema 样板重复

`mcp/tools/repo.rs` 2,376 行中，25+ tools 每个都有:
```rust
fn schema(&self) -> serde_json::Value {
    serde_json::json!({
        "description": "...",
        "inputSchema": { "type": "object", "properties": { ... } }
    })
}
```

**重复度**: 每个 tool ~40 行 schema 样板 × 25 = ~1,000 行纯重复。

### 6.2 SQL Join 模式重复

`entities` + `repo_tags` LEFT JOIN 模式出现在:
- `registry/repo.rs`
- `mcp/tools/repo.rs`
- `query.rs`

### 6.3 i18n 翻译表结构重复

`i18n/en.rs` 和 `i18n/zh_cn.rs` 是 137 行的结构完全相同的映射表。

---

## 七、架构清晰度评估

### 7.1 层次结构

```
┌─────────────────────────────────────────┐
│  Presentation Layer                     │
│  commands/  mcp/tools/  tui/            │
├─────────────────────────────────────────┤
│  Application Layer                      │
│  workflow/  skill_runtime/  daemon/     │
├─────────────────────────────────────────┤
│  Domain Layer                           │
│  scan  query  health  knowledge_engine  │
├─────────────────────────────────────────┤
│  Infrastructure Layer                   │
│  registry/  search/  storage/  vault/   │
└─────────────────────────────────────────┘
```

**问题**:
- **边界渗透**: `commands/simple.rs` 直接调用 `WorkspaceRegistry`（应通过 service 层）
- **双向依赖**: `storage::AppContext` 初始化时调用 `registry::WorkspaceRegistry::init_db_at()`，而 registry 又依赖 `storage::StorageBackend`
- **领域逻辑泄露**: `knowledge_engine.rs` 包含 LLM API 调用（基础设施），应与领域摘要逻辑分离

### 7.2 模块内聚度

| 模块 | 内聚度 | 问题 |
|------|--------|------|
| `registry/migrate.rs` | 🔴 低 | 25 个不相关的 schema 版本挤在一起 |
| `tui/state.rs` | 🔴 低 | 渲染+状态+事件+输入处理全混 |
| `mcp/mod.rs` | 🟡 中 | 37 tools + 协议处理 + 错误处理 |
| `knowledge_engine.rs` | 🟡 中 | manifest 解析 + LLM 调用 + 摘要生成 |
| `semantic_index.rs` | 🟡 中 | AST 提取 + 索引写入 + 查询接口 |
| `scan.rs` | 🟢 高 | 专注仓库检测，子函数清晰 |
| `embedding.rs` | 🟢 高 | 向量运算 + provider 路由，职责单一 |

---

## 八、健康度评分卡

| 维度 | 权重 | 得分 | 说明 |
|------|------|------|------|
| 编译安全 | 20% | 95 | unwrap=0, unsafe=0, clippy=0 |
| 测试覆盖 | 20% | 55 | 21.6% 测试率，20 个零测试文件 |
| 模块规模 | 15% | 50 | 12 个巨石文件，28 个超长函数 |
| 耦合度 | 20% | 45 | God Object ×2，15+ 模块依赖 |
| API 表面 | 10% | 55 | 32 pub mod 过度暴露 |
| 内聚度 | 10% | 60 | 3 个低内聚模块，其余良好 |
| 重复度 | 5% | 60 | ~1,000 行 schema 样板重复 |
| **加权总分** | 100% | **58.5** | 🟡 及格偏下 |

---

## 九、改进路线图

### v0.15 优先级（架构债务清偿）

| 优先级 | 事项 | 健康度影响 | 预估工作量 |
|--------|------|-----------|-----------|
| P0 | `WorkspaceRegistry` → facade + 子 registry 拆分 | 耦合度 +20 | 2-3 天 |
| P0 | `init_db_at` 1,214 行 → per-version migration 模块 | 内聚度 +15 | 4h |
| P1 | `lib.rs` pub mod 降级为 `pub(crate)` | API 表面 +15 | 2h |
| P1 | `mcp_schema!` 宏提取，消除 ~1,000 行样板 | 重复度 +20 | 1 天 |
| P1 | 20 个零测试文件补 smoke test | 测试覆盖 +15 | 2-3 天 |
| P2 | `tui/state.rs` 拆分为 `state/` 子模块 | 内聚度 +10 | 1 天 |
| P2 | 28 个超长函数拆分（>100 行） | 模块规模 +10 | 1-2 天 |

**目标**: 加权总分 58.5 → 75+（v0.16 末）
