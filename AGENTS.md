# Agent 环境指引

`devbase` 是 **本地情境编译器（Local Context Compiler）** —— AI agent 在本地数字世界中的海马体。

> 它将本地数字资产的原始数据（代码库、笔记、Skill、工作流）编译为 AI 可决策的结构化情境，不负责思考，不负责执行，只负责感知、编码、持久化、检索。

- **当前阶段**：阶段十一 — v0.20.0 已发布（知识完备性）
- **当前版本**：v0.20.1（Schema 36，71 MCP tools，495 tests）
- **已完成里程碑**：Registry God Object 完全拆解（10 子模块提取）+ 18 workspace crates 提取 + MCP Python SDK 1.16.0 兼容修复 + repo.rs trait 化 + flaky 测试根治（RF-2.1/2.2/2.3）+ 许可证迁移 + health 性能优化（-44%）+ index skip-embeddings + batch encoding 实验 + RF-6 清零 + 架构治理文档（ADR/不变量清单）+ Tantivy BM25 代码符号搜索（P1）+ AppContext 职责拆分 Phase 1/2（storage.rs 860→430 行）+ 架构不变量 CI（G5/T11/T12）+ Embedding 多后端（Candle/Ollama 配置切换, P3）+ EnvVersionCache 扩展（9 工具链检测, P4）+ **v0.16.0 Agent Contexts（P1/P2/P3）**：`agent_contexts`/`agent_memories`/`context_entity_links` Schema + 9 个 Session MCP tools + Context-aware Skill Runtime（`DEVBASE_ACTIVE_CONTEXT` 注入）+ **v0.16.1 Workflow-Session Binding**：`workflow_executions.context_id` + 执行自动绑定 Active Context + **v0.17.0 Embedding Externalization**：`embedding` 从 default features 移除（Candle/Ollama 降级为 opt-in `llm-backend`）+ Schema 34 向量存储 + `cosine_similarity` SQLite UDF + `devkit_session_recall` / `devkit_session_index`（60 tools）+ **v0.18.0 ClaudeCode Integration**：`devkit_project_brief`（Markdown 项目简报）+ `devkit_impact_analysis`（修改影响范围分析）+ `devkit_session_export` / `devkit_session_import` + `scripts/devbase-claude.ps1` 启动器（自动注入 `.claude/CLAUDE.md`）+ RFC `docs/RFC/claudecode-workflow-integration.md`（64 tools）+ **v0.18.0 发布收尾**：PR 合并 + 双平台二进制构建 + GitHub Release + 根目录治理 + 世界模型战略认知沉淀（Vault + AGENTS 双向联动）+ NotebookLM 生态消化（5 项目注册）+ GreptimeDB 互补分析 + **v0.19.0 知识基础设施硬化**：SQLite WAL 默认启用 + `devkit_index_health`（Beta）+ Vault 导出（`devkit_vault_export`）+ Redis ADR 决策（放弃引入）+ **v0.20.0 知识完备性**：Vault 双向链接 BFS 图遍历（`devkit_vault_graph` 扩展）+ Vault Git-based 历史追踪（`devkit_vault_history`，第 67 个 tool）+ 混合检索质量监控（`devkit_search_quality`，第 68 个 tool，`HybridSearchMetrics`）+ Block 引用支持（`WikiLink.anchor`：`[[note#heading]]` / `[[note#^block-id]]`）+ 性能回归基线（`#[ignore]` 1k/10k 阈值测试）+ 客户端无关原则（Client-Agnostic Principle）落地 + `skill sync` 泛化接口（零硬编码客户端路径）
- **核心方向**：让 Kimi CLI 在调用文件工具之前，先通过 devbase 获得"该读哪些文件、为什么读、它们之间的关系"
- **本质分析**：见 `vault/99-Meta/devbase-essence-analysis-20260430.md` 与 `docs/architecture/redefinition.md`
- **设计文档**：
  - [`docs/architecture/workflow-dsl.md`](docs/architecture/workflow-dsl.md) — Workflow DSL 规范
  - [`docs/architecture/workspace-as-schema.md`](docs/architecture/workspace-as-schema.md) — 统一实体模型设计
  - [`docs/RFC/agent-memory-vector-storage.md`](docs/RFC/agent-memory-vector-storage.md) — v0.17.0 Agent Memory 向量存储 RFC（Embedding 职责外迁设计）
  - [`docs/guides/mcp-integration-guide.md`](docs/guides/mcp-integration-guide.md) — MCP 集成指南
  - [`docs/README.md`](docs/README.md) — 完整文档导航

Skill Runtime 全生命周期已落地（含依赖管理 Schema v15），Schema v16 统一实体模型（entities/relations）已落地，Skill 自动封装（`discover`）已落地。

- **技术栈**：Rust 2024, SQLite, tokio, ratatui, git2, reqwest, tantivy
- **Registry DB**：`%LOCALAPPDATA%\devbase\registry.db`（轻量索引，用户本地，永不进入版本控制）
- **Workspace**：`%LOCALAPPDATA%\devbase\workspace/` —— 文件系统 = source of truth
  - `vault/` —— PARA 结构：00-Inbox, 01-Projects, 02-Areas, 03-Resources, 04-Archives, 99-Meta
  - `assets/` —— 二进制资源
- **MCP Server**：stdio only，**71 个 tools**（含 7 个 vault tools + 8 个代码分析工具 + 5 个 embedding/搜索工具 + 5 个 Skill Runtime tools + 3 个 Workflow/评分 tools + 1 个报告工具 + 1 个 arXiv 工具 + 2 个 KnownLimit tools + 3 个 Relation tools + 11 个 Agent Context tools + 2 个 ClaudeCode 集成工具 + 1 个 streaming index 工具 + 1 个 oplog 工具 + 1 个 Index Health 工具 + 1 个 Search Quality 工具 + 1 个 Evaluate 工具 + 1 个 DocumentConvert 工具 + 1 个 Ontology Import 工具 + 1 个 Skill Sync 工具）；配置见 `mcp.json`
- **Kimi CLI 集成**：MCP server 已通过 `kimi mcp add` 注册，端到端验证通过（`kimi --print` 成功调用 `devkit_health`）；项目级 skill 位于 `.kimi/skills/devbase-project/SKILL.md`
- **统一节点模型**：`core::node::{Node, NodeType, Edge}` —— GitRepo / VaultNote / Asset / ExternalLink
- **当前测试**：476 lib passed / 0 failed / 5 ignored + 7/7 integration passed + 11/11 workflow passed（共 494）
- **编译状态**：0 warning / 0 vulnerabilities（`cargo audit` 干净，除上游 `tokei` 的 `RUSTSEC-2020-0163`）
- **Workspace 结构**：`crates/` 目录已启用，19 个零耦合模块已提取为独立 crate（`devbase-symbol-links`, `devbase-sync-protocol`, `devbase-core-types`, `devbase-syncthing-client`, `devbase-vault-frontmatter`, `devbase-vault-wikilink`, `devbase-workflow-interpolate`, `devbase-workflow-model`, `devbase-registry-health`, `devbase-registry-metrics`, `devbase-registry-workspace`, `devbase-embedding`, `devbase-skill-runtime-types`, `devbase-skill-runtime-parser`, `devbase-registry-entity`, `devbase-registry-relation`, `devbase-registry-call-graph`, `devbase-registry-dead-code`, `devbase-registry-code-symbols`）
- **Workflow Engine**：YAML 解析 + 拓扑调度 + batch 并行执行 + 5 种 step 类型（skill/subworkflow/parallel/condition/loop）
- **NLQ 自然语言查询**：TUI `[:]` 触发 embedding 语义搜索，fallback 降级文本搜索
- **Mind Market 评分**：success_rate / usage_count / rating（0-5），`skill recalc-scores/top/recommend`

## 关键约定

1. **文件操作**：读取用 `ReadFile`，搜索用 `Grep`/`Glob`，修改用 `StrReplaceFile`，整文件重写用 `WriteFile`
2. **Shell**：Windows PowerShell；用 `;` 分隔命令
3. **Git**：提交前必须通过 `cargo test --all-targets` + `cargo clippy --all-targets -D warnings` + `cargo fmt --check`
4. **Schema 迁移**：`PRAGMA user_version` 安全升级；升级前自动调用 `backup::auto_backup_before_migration()`

## 安全原则

### 本地优先（Local-First）

- **Registry DB** 始终存储在用户的本地配置目录（`dirs::config_dir()/devbase/`），绝不向远程传输
- **代码内容** 不会被上传到任何云端服务（除非用户显式配置 GitHub token 用于 stars 查询）
- **MCP Server** 仅通过 stdio 本地进程通信，不暴露网络端口

### 客户端无关（Client-Agnostic）

> devbase 的核心能力（编排、注册、索引、搜索、同步）必须在不依赖任何特定 AI 客户端的前提下独立运行。

- ✅ **允许**：向通用目录输出数据，由用户自行分发给任意客户端（如 `skill sync --output-dir ./plans`）
- ✅ **允许**：实现标准协议（MCP）供任意客户端连接
- ❌ **禁止**：核心能力硬编码特定客户端的路径、API、或配置格式（如 `C:\Users\xxx\.claude`）
- ❌ **禁止**：核心能力的可用性取决于某个客户端是否安装
- 🟡 **适配层**：`scripts/claude/`、`docs/clients/` 等目录下的客户端适配脚本属于配套示例，不归入核心版本控制

### 凭证管理

- GitHub token、LLM API key 存储在本地 `config.toml` 中
- `config.toml` 位于用户配置目录，**不在项目工作目录**，因此不会被意外 `git commit`
- 默认配置模板中的 token 字段使用占位符 `<YOUR_GITHUB_PAT>`，避免真实 token 格式泄露
- `.gitignore` 已覆盖 `*.db`、`.devbase/`、`.env*`、`*.local.toml`

### 审计与备份

- 所有 `scan`/`sync`/`health` 操作自动写入 OpLog（SQLite `oplog` 表）
- Schema 迁移前自动生成 `backup-YYYYMMDD-HHMMSS.db` 快照
- Registry 支持 `export`/`import` 用于用户自主备份

## 许可证策略

- **主许可证**: AGPL-3.0-or-later (`LICENSE`)
- **商业授权**: 双许可模式，闭源/专有 SaaS 使用需联系作者 (`LICENSE-COMMERCIAL.md`)
- **Cargo.toml**: `license = "AGPL-3.0-or-later"`
- **SPDX 头**: 新增源文件应在顶部包含 AGPL-3.0 声明（见 `LICENSE` 末尾 "How to Apply" 部分）

## 架构状态（Wave 15b 完成）

| 维度 | 状态 |
|------|------|
| 代码质量 | `rustfmt.toml` + `cargo fmt` + `clippy -D warnings` 全绿 |
| 模块拆分 | `sync`→5 / `registry`→11 / `mcp` 测试分离 / `search`→hybrid / `oplog_analytics` / `symbol_links` / **workspace: 3 crates extracted** |
| 库/二进制 | `src/lib.rs` 导出全部 **30+** 个模块；`src/main.rs` 仅 CLI 入口 |
| TUI 架构 | `render/` 6 子模块 + `theme.rs` Design Token + `layout.rs` 响应式引擎 |
| 数据层 | Schema v23: `repos`/`vault_notes`/`papers`/`workflows`/`repo_modules_legacy` 表已删除；`entities` 为唯一数据源；`repo_tags/repo_remotes/repo_health/...` 为独立 JOIN 表（无 FK）；仅 `skills` 保留独立表（embedding BLOB） |
| CI/CD | `.github/workflows/ci.yml`：check / test / fmt / clippy on Windows |
| 依赖安全 | `cargo audit` 0 漏洞（除上游 `tokei` 的 `RUSTSEC-2020-0163`） |

## 架构红线（Architecture Guardrails）

> 基于第一性原理的工程约束。违反任意一条 = HALT，转交人类裁决或回滚。
> 规则编号 `RF-XX`（Red-line / Fitness function），带客观测量标准，非主观描述。

### RF-1: 依赖注入优于全局状态（Global State Anti-Pattern）

**理论锚定**：全局可变状态使组件隐式耦合，破坏可测试性与可复用性（参考：Pure Function / DI 原则）。

**规则**：
- 禁止新增 `dirs::data_local_dir()` / `std::env::var_os` 硬编码路径。
- 所有 IO 边界路径（DB、索引、备份、配置）必须通过参数、构造函数或 `trait` 注入。
- **例外（Grandfathered）**：现有 3 处（`backup_dir`、`db_path`、`index_path`）在重构前不得新增第 4 处。

**Fitness Function**：
```bash
# 新增 PR 中不得出现新的全局路径硬编码
grep -rn "dirs::data_local_dir\|std::env::var_os\|std::env::var(\"LOCALAPPDATA\"" src/ \
  | grep -v "backup.rs\|migrate.rs\|search.rs"
# 预期输出：空
```

### RF-2: 测试密封性（Hermetic Testing）

**理论锚定**：测试失败必须仅因被测代码缺陷，不因外部因素、测试顺序或并行调度（参考：Google Test Blog — Hermetic Servers）。

**规则**：
- 所有测试禁止修改全局进程状态（`std::env::set_var`、`static mut`、全局文件系统句柄）。
- 文件系统测试必须使用 `tempfile` + 注入式路径，禁止直接操作 `%LOCALAPPDATA%` 或 `~/.config`。
- Tantivy / SQLite 文件系统测试必须获取 `SEARCH_TEST_LOCK`（或同等级串行化机制）。

**子规则（来自 PR #4 教训）**：
- **R2.1 禁止 `DEVBASE_DATA_DIR` 全局注入**：并行测试中 `std::env::set_var("DEVBASE_DATA_DIR", ...)` 导致竞态；必须使用 `TempStorageBackend` 注入式替代。
- **R2.2 Windows 路径双端规范化**：`TempDir` 可能返回短文件名（`TEMP~1`），而 `dunce::canonicalize` 返回长文件名；路径比较前必须对**双方**调用 `dunce::canonicalize`。
- **R2.3 `git2` 测试显式身份 + 显式分支**：
  - CI runner 无全局 `user.name`/`user.email` → `repo.signature()` 会 panic；必须改用 `git2::Signature::now("Test", "test@example.com")`。
  - `git2::Repository::init` 的默认分支在不同平台可能为 `master` 或 `main`；必须显式 `repo.set_head("refs/heads/main")` 并 commit 到 `"refs/heads/main"`。

**Fitness Function**：
```bash
# 高并发下 100% 通过，无 flaky
cargo test --test-threads=16
```

### RF-3: Schema 单一事实来源（Single Source of Truth）

**理论锚定**：重复信息必然 drift（参考：DRY 原则 + Evolutionary Architecture 的版本一致性约束）。

**规则**：
- `SCHEMA_DDL`（`registry/test_helpers.rs`）与 `migrate.rs` 必须原子同步。
- 新增表、索引、列必须同时出现在两者中；禁止仅更新其一。

**Fitness Function**：
- CI 运行 `test_in_memory_schema_version` + schema 结构比对脚本（可手动运行 `cargo test registry::test_helpers::tests` 验证）。

### RF-4: 二进制入口限界（Bounded Context）

**理论锚定**：CLI 入口应仅做命令分发，业务逻辑应在 lib 模块中（参考：Hexagonal Architecture / Ports & Adapters）。

**规则**：
- `main.rs` 行数不得超过 **1000 行**。
- 新增 CLI 命令必须先拆分为 `commands/` 子模块或独立函数，禁止在 `main.rs` 中堆积业务逻辑。

**Fitness Function**：
```bash
# 当前 515 行（Phase 1/2/3 已削减 1003 行），远超目标
[ $(wc -l < src/main.rs) -le 1000 ] || exit 1
```

### RF-5: 无循环依赖（Acyclic Dependencies）

**理论锚定**：循环依赖破坏模块化，使增量编译和独立复用不可能（参考：John Lakos — Large-Scale C++ Software Design）。

**规则**：
- 禁止模块间双向 `use crate::` 引用。
- 新增模块必须通过脚本验证无循环（当前已满足，未来 PR 保持）。

**Fitness Function**：
```bash
# 文件级双向依赖检测（当前输出应为空）
for f in src/**/*.rs; do
  name=$(basename "$f" .rs)
  refs=$(grep -o 'use crate::[a-z_]*' "$f" | sed 's/use crate:://')
  for r in $refs; do
    if [ -f "src/$r.rs" ] && grep -q "use crate::$name\b" "src/$r.rs"; then
      echo "CYCLE: $name <-> $r"
    fi
  done
done
```

### RF-7: Workspace 拆分约束（Module Distribution Readiness）

**理论锚定**：模块能否独立发布是耦合健康度的金标准；不能拆分的模块 = 耦合不健康的模块。

**规则**：
- 新增模块若对 devbase 内部其他模块的 `crate::` 引用超过 **5 个**，禁止提取为 workspace crate。
- 已提取 crate 的重新导出文件（`src/symbol_links.rs` 等）**禁止添加新代码**——顶部有 `RE-EXPORT ONLY` 注释作为守卫。
- 子 crate 的依赖版本必须与 workspace 统一，禁止独立 bump。

**Fitness Function**：
```bash
# 扫描所有 src/*.rs，统计 crate:: 引用数
for f in src/*.rs; do
  count=$(grep -c 'crate::' "$f")
  if [ "$count" -gt 15 ]; then
    echo "HIGH COUPLING: $f ($count refs)"
  fi
done
# 预期输出：空（或仅已标记的高耦合文件如 mcp/tools/repo.rs）
```

### RF-6: 生产代码无 panic（Crash-only Software）

**理论锚定**：Rust 的 `Result` 类型将错误显式化；`unwrap` 是将运行时崩溃隐藏在类型系统背后（参考：Joe Armstrong — Let it crash，但 Rust 中崩溃 = 进程终止，不可接受）。

**规则**：
- 生产代码（`src/**/*.rs` 中不在 `#[cfg(test)]` 块内的代码）禁止 `unwrap()`、`expect()`、`panic!()`。
- 测试代码不受此限，但鼓励使用 `?` 传播。

**Fitness Function**：
```bash
# 生产代码 unwrap 计数（排除 #[cfg(test)] 块及 tests.rs 文件）
for f in $(find src -name "*.rs"); do
  test_line=$(grep -n "#\[cfg(test)\]" "$f" | head -1 | cut -d: -f1)
  if echo "$f" | grep -qE "tests?\.rs$|_test\.rs$|/tests/"; then continue; fi
  if [ -n "$test_line" ]; then
    head -n "$((test_line - 1))" "$f" | grep -n "\.unwrap()"
  else
    grep -n "\.unwrap()" "$f"
  fi
done
# 预期输出：空
```

**状态**：🟢 **已完成**（v0.20.1 复核：生产代码 unwrap = 0；此前 1090 为测试模块误统计）。

### 架构治理框架（Architecture Governance）

> 参考：外部架构治理方法论（Kimi 会话 `e9f2965f-b949-46a5-9d7c-afd6d4d9232c`）

**已制度化实践**：

| 实践 | devbase 落地形式 | 文档位置 |
|------|-----------------|---------|
| ADR（架构决策记录） | ADR-001（单 crate defer）、ADR-002（batch encoding 回滚） | [`docs/architecture/adr-template.md`](docs/architecture/adr-template.md) |
| 不变量清单（Invariants） | RF-1~RF-7 + 分层模块约束（T01–T12） | [`docs/architecture/invariants.md`](docs/architecture/invariants.md) |
| 模块提取演习 | RF-7 的 5 个 `crate::` 引用阈值 + 已提取 18 workspace crates | 本文件 §RF-7 |
| 三层摘要 | `crates/*/README.md` 要求：一句话 + 一页纸 + 深度链接 | 各 crate README |
| 定期架构回顾 | 每次 Wave 结束时的架构审计（见 `docs/_audit/`） | `docs/_audit/2026-04-26-*.md` |

**待增强**：
- 三层摘要：部分已提取 crate 的 README 尚未达到"一页纸"标准
- 定期架构回顾：当前按 Wave（功能迭代周期）触发，建议每 2–4 周增加一次纯架构 review（不看 feature 进度，只看不变量违反和隐式依赖）

---

## 禁止事项

- 不得修改 `dev\third_party\*` 外部仓库
- 不得在没有迁移逻辑的情况下修改 registry schema
- 不得引入已 deprecated 的协议
- **不得在主仓库引入 Spark/Flink 依赖**（研究性质代码必须置于独立仓库，保持主仓库轻量）
- **不得在任何源码文件中硬编码真实 token、api_key 或密码**（包括注释和测试数据）

> 完整版（含历史记录、路线图、详细讨论）：见 docs/AGENTS-full.md
