# Agent 环境指引

`devbase` 是本地优先的 AI Skill 编排基础设施。

- **当前阶段**：阶段二 — AI Skill 编排基础设施（v0.8.0-alpha 进行中）
- **当前版本**：v0.8.0-alpha
- **下一里程碑**：v0.9.0（NLQ 结果可执行 + Loop Step + L0-L4 知识模型）
- **核心方向**：将 GitHub 项目转换为标准化、可发现、可组合的 Skill，供弱 AI 子代理执行
- **设计文档**：
  - `docs/architecture/workflow-dsl.md` — Workflow DSL 规范
  - `docs/architecture/workspace-as-schema.md` — 统一实体模型设计

Skill Runtime 全生命周期已落地（含依赖管理 Schema v15），Schema v16 统一实体模型（entities/relations）已落地，Skill 自动封装（`discover`）已落地。

- **技术栈**：Rust 2024, SQLite, tokio, ratatui, git2, reqwest, tantivy
- **Registry DB**：`%LOCALAPPDATA%\devbase\registry.db`（轻量索引，用户本地，永不进入版本控制）
- **Workspace**：`%LOCALAPPDATA%\devbase\workspace/` —— 文件系统 = source of truth
  - `vault/` —— PARA 结构：00-Inbox, 01-Projects, 02-Areas, 03-Resources, 04-Archives, 99-Meta
  - `assets/` —— 二进制资源
- **MCP Server**：stdio only（SSE 开发中），**35 个 tools**（含 5 个 vault tools + 8 个代码分析工具 + 4 个 embedding/搜索工具 + 4 个 Skill Runtime tools + 3 个 Workflow/评分 tools + 1 个报告工具 + 1 个 arXiv 工具）；配置见 `mcp.json`
- **统一节点模型**：`core::node::{Node, NodeType, Edge}` —— GitRepo / VaultNote / Asset / ExternalLink
- **当前测试**：266 passed / 0 failed / 3 ignored
- **编译状态**：0 warnings / 0 vulnerabilities（`cargo audit` 干净，除上游 `tokei` 的 `RUSTSEC-2020-0163`）
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

### 凭证管理

- GitHub token、LLM API key 存储在本地 `config.toml` 中
- `config.toml` 位于用户配置目录，**不在项目工作目录**，因此不会被意外 `git commit`
- 默认配置模板中的 token 字段使用占位符 `<YOUR_GITHUB_PAT>`，避免真实 token 格式泄露
- `.gitignore` 已覆盖 `*.db`、`.devbase/`、`.env*`、`*.local.toml`

### 审计与备份

- 所有 `scan`/`sync`/`health` 操作自动写入 OpLog（SQLite `oplog` 表）
- Schema 迁移前自动生成 `backup-YYYYMMDD-HHMMSS.db` 快照
- Registry 支持 `export`/`import` 用于用户自主备份

## 架构状态（Wave 15b 完成）

| 维度 | 状态 |
|------|------|
| 代码质量 | `rustfmt.toml` + `cargo fmt` + `clippy -D warnings` 全绿 |
| 模块拆分 | `sync`→5 / `registry`→7 / `mcp` 测试分离 / `search`→hybrid / `oplog_analytics` / `symbol_links` |
| 库/二进制 | `src/lib.rs` 导出全部 **30+** 个模块；`src/main.rs` 仅 CLI 入口 |
| TUI 架构 | `render/` 6 子模块 + `theme.rs` Design Token + `layout.rs` 响应式引擎 |
| 数据层 | Schema v17: repos + repo_tags + code_symbols + code_embeddings + code_call_graph + code_symbol_links + oplog + vault_notes + papers + experiments + **skills + skill_executions** + **entities + entity_types + relations** + **workflows + workflow_executions**（统一实体模型，渐进双写） |
| CI/CD | `.github/workflows/ci.yml`：check / test / fmt / clippy on Windows |
| 依赖安全 | `cargo audit` 0 漏洞（除上游 `tokei` 的 `RUSTSEC-2020-0163`） |

## 历史 Waves

| Wave | 主题 | 关键产出 | Commit |
|------|------|---------|--------|
| 1 | 代码质量 | `rustfmt.toml`, clippy 清零 | `4efcd58` |
| 2 | 模块拆分 | `sync/`, `registry/`, `mcp/tests.rs` | `4efcd58` |
| 3 | 工程化 | `src/lib.rs`, CI workflow, `main.rs` 简化 | `4efcd58` |
| 4 | 依赖/审计 | `notify` 8.2.0, `tokei` 14.0.0 | `4efcd58` |
| 5 | TUI 美学与工程学 | 主题系统, Tabs, Help Overlay, Render 拆分 | `6b9be88` |
| 6 | 数据层深度能力 (MVP) | 语义索引、调用图、依赖图、死代码检测、Python 依赖解析 | `9fbf7c4` |
| 7 | 向量语义搜索 | `embedding.rs`, `code_embeddings` 表, `devkit_semantic_search` | `4d400b1` |
| 8 | 多语言符号提取 | tree-sitter-python/typescript/go, Rust/Python/JS/Go 符号 + Call Graph | `4f4911b` |
| 9 | scan panic 修复 + arXiv/CMake | `block_on_async` 安全封装, arXiv API 元数据, CMakeLists.txt 依赖解析 | `881cd32` |
| 10 | OpLog 结构化 | Schema v12, OplogEventType 枚举, JSON metadata, duration_ms | `7aa2a65` |
| 11 | 性能基准 | criterion benches: index_repo_full, cosine_similarity, extract_symbols, CMake | `8e0f236` |
| 12 | 混合检索核心 | `search::hybrid.rs`: RRF 归并, keyword_search, hybrid_search_symbols | `7fca714` |
| 13 | 外部 Embedding Provider | Python CLI `tools/embedding-provider/`, Ollama 批量生成, 字节兼容序列化 | `574fb96` |
| 14a | 跨 repo 语义聚合 | `cross_repo_search_symbols()` INTERSECT tag 过滤, `devkit_cross_repo_search` | `8e762c7` |
| 14b | 知识覆盖报告 | `oplog_analytics.rs`: 表存在性容错, 覆盖度/健康度/活动流, `devkit_knowledge_report` | `869bcbf` |
| 15a | 显式知识链接 | Schema v13 `code_symbol_links`, Jaccard 签名相似度, 同文件聚类, `devkit_related_symbols` | `d462209` |
| 15b | 混合检索 MCP Tool | `devkit_hybrid_search`: 向量+RRF+关键词自动降级, 推荐默认搜索入口 | `6df6106` |
| 16a | Skill Runtime Schema | `skills` + `skill_executions` 表, SKILL.md 解析器, Registry CRUD, 3 内置 skills | `e41eccb` |
| 16b | Skill 发现与搜索 | 文本搜索 + 语义搜索 (`--semantic`), skill embedding 生成脚本 | `48b96c6` |
| 17 | Skill 执行引擎 | Process-based executor, interpreter 自动解析, timeout, stdout/stderr 捕获, 执行审计 | `99d818e` |
| 18 | MCP Skill 集成 | `devkit_skill_list` / `devkit_skill_search` / `devkit_skill_run` 3 个 tools | `c80fdec` |
| 19a | Skill 生态（安装/发布） | `install_skill_from_git` (git2 clone), `publish` (validate + git tag + push remote) | `8120e4d` |
| 19b | Skill 生态（同步/TUI） | `sync --target clarity` (导出为 Clarity plan JSON), TUI Skill Panel (`k` keybinding) | `678c70c` |
| 20 | Skill 依赖管理 | Schema v15 `dependencies` 列，Kahn 拓扑排序，DFS 环检测，自动安装缺失依赖，`install`/`run`/`validate` 集成 | `75fed3c` |
| 21 | 统一实体模型 + 自动封装 | Schema v16 `entities/entity_types/relations`，渐进双写；`discover` 命令（Rust/Node/Python/Go/Docker/Generic 检测 + SKILL.md 自动生成 + entry_script 包装器）；分类推断（ai/dev/data/infra/communication） | — |

## 敏感文件清单（禁止提交）

| 文件/模式 | 原因 | .gitignore 覆盖 |
|-----------|------|----------------|
| `*.db` | SQLite 数据库含用户仓库元数据 | ✅ |
| `.devbase/` | 本地 sync 标记和工作区状态 | ✅ |
| `*.log` | 可能含路径或错误堆栈信息 | ✅ |
| `.env*` | 环境变量和 secrets | ✅ |
| `*.local.toml` | 本地覆盖配置 | ✅ |
| `target/` | 构建产物 | ✅ |

## 跨项目接口

- **clarity-core**：已解除路径依赖。devbase 不再被 clarity-core 调用，LLM 能力内联为纯 reqwest
- **syncthing-rust**：`.syncdone` 标记格式已对齐

## 架构讨论摘要（来自 2026-04-24 会话）

以下为本项目相关的粗粒度架构决策与待探索方向。

### 1. 自指知识库：五层知识模型

devbase 作为知识库存储层，需支持 L0-L4 五层索引：

| 层级 | 内容 | 生长信号 | 遗忘机制 |
|------|------|---------|---------|
| L0 对象 | 外部知识块（代码、文档、日志） | 检索频率、引用次数 | 版本冻结 |
| L1 方法 | 操作知识的方法（检索/分块/向量化） | 检索成功率、延迟分布 | A/B 测试 |
| L2 哲学 | 设计原则（本地优先、奥卡姆剃刀） | 架构决策事后验证 | 外部论文扰动 |
| L3 风险 | 系统弱点图谱 | 故障事件、异常日志 | 红队攻击 |
| L4 元认知 | 关于 L1-L3 的元知识 | 人类纠正、跨会话一致性 | 形式化验证 |

**决策**：粗粒度与细粒度知识保留独立索引；细粒度存 SQLite（快速查询），粗粒度存 Vector DB（语义检索）。

### 2. 审计日志（OpLog）

- P3 不可靠交付的使用追踪写入 OpLog，实现事后追溯
- 边界图谱版本历史、探索任务结果写入 OpLog
- 所有验证消息（请求+响应+共识）写入 OpLog

### 3. 外部资源调度器

devbase 承载外部资源调度的抽象接口：

- **形式化工具**：TLA+/Coq/Lean（本地路径或远程地址）
- **人类专家**：异步审批，不阻塞夜间批处理
- **P2P 节点**：复用 syncthing-rust 的 Device ID 与传输层
- **文献检索**：arXiv / Semantic Scholar API

**决策**：定义资源请求的抽象接口与排队策略；具体调度算法不进当前 scope。

### 4. 边界图谱存储

- `BoundaryMap` 存储已知限制（KnownLimit）的版本历史
- `ExplorationTask` 队列记录边界外待探索任务
- 跨实例同步：通过 syncthing-rust P2P 网络同步边界快照

### 5. 安全计算（MPC/TEE）

- 当前四个项目中无密码学层归属
- **短期**：devbase MCP 接口可封装外部 TEE 服务（如 Azure Confidential Computing）
- **长期**：如需自建，新建 `clarity-tee` 或 `devbase-secure` 子项目

## 当前粗粒度待办

### 阶段二任务（v0.4.0 AI Skill 编排基础设施）

| 波次 | 任务 | 状态 | 交付物 |
|------|------|------|--------|
| Wave 21 | Schema v16 + 自动封装 | ✅ 已完成 | `entity_types/entities/relations` + `devbase skill discover` |
| Wave 22 | discover 硬化 | ✅ 已完成 | `--install` 真正注册 + Git URL 直接克隆封装 |
| Wave 23 | Workflow 预留 | ✅ 规范已完成 | `docs/architecture/workflow-dsl.md` |
| Wave 24 | Workflow Engine v0.5.0 | ✅ 已完成 | YAML 解析 + 拓扑调度 + batch 并行执行 + 5 step 类型 |
| Wave 25 | TUI Workflow 可执行 | ✅ 已完成 | `[w]` 详情页 `r/Enter` 运行 + 结果弹窗 |
| Wave 26 | NLQ 自然语言查询 v0.7.0 | ✅ 已完成 | `[:]` 触发 embedding 语义搜索 + fallback 降级 |
| Wave 27 | Mind Market 评分 v0.6.0 | ✅ 已完成 | `success_rate`/`usage_count`/`rating` + `recalc-scores`/`top`/`recommend` |
| Wave 28 | 7 个风险点修复 v0.7.1 | ✅ 已完成 | EnvGuard、NLQ fallback、StepType 显式标签、跨平台解释器探测 |
| Wave 29 | Workflow 子类型执行 v0.8.0 | ✅ 已完成 | Subworkflow 递归 + Parallel 聚合 + Condition 表达式求值 |

### 明确不做（已排除）

- SSE transport（stdio 已覆盖主流 Client）
- `.devbase` 目录规范（无外部采纳者）
- MCP 协议扩展提案（Star = 0，不会被采纳）
- 商业化 / 付费版
- 拆分 crate（50+ tools 后再评估）

### Future / Icebox（无排期）

1. ~~输出 L0-L4 五层知识的 TOML/JSON Schema 草案~~（保持开放，非阻塞）
2. ~~输出 OpLog 审计事件类型清单~~（已有基础枚举，保持增量）
3. ~~输出外部资源调度的请求格式草案~~（保持开放）
4. **不做**：调度算法、边界图谱引擎、哲学规则库内容、密码学协议

### Post-Wave 19  triage 结论（2026-04-25）

| 优先级 | 事项 | 状态 |
|--------|------|------|
| P1 | SSE 传输状态与 README 一致性 | ✅ 已完成 — README 修正为 "stdio only; SSE in development"，见 commit `935dd61` |
| P2 | 架构预拆分评估 | ✅ 已完成 — 评估报告位于 `docs/architecture/pre-split-evaluation.md`，结论：22.7 KLOC 单 crate 仍最优， defer 至 50+ tools 或编译 > 60s |
| P3 | 竞品定位标语 | ✅ 已完成 — README 顶部标语更新为 "AI 无法识别你的 GUI，devbase 是它的眼镜。" |
| P4 | 开发者 onboarding 文档 | ✅ 已完成 — `CONTRIBUTING.md` + README Contributing 章节（devbase + clarity） |

- **Tag**: `v0.2.4` 已打标（commit `935dd61`）
- **Roadmap**: `docs/ROADMAP.md` 已建立两步走框架

## Embedding 策略长期规划（已决策）

**方向**：混合方案 — 模型向量语义搜索 + tantivy BM25 降级

| 层级 | 触发条件 | 技术方案 | 状态 |
|------|----------|----------|------|
| L1 向量语义 | `code_embeddings` 表有数据 | Ollama/OpenAI-compatible 生成 768-dim embedding，余弦相似度 Top-K | 已实现，待激活（需 Ollama 运行） |
| L2 全文搜索 | `code_embeddings` 为空或服务不可用时 | tantivy 索引代码符号（function name + signature + doc comment），BM25 评分 | 基础设施就绪，待接入 `semantic_search_symbols` |
| L3 纯符号匹配 | 查询为精确标识符 | SQLite `LIKE '%name%'` 快速匹配 | 已有 |

**关键决策**：不绑定 Ollama 为唯一 provider。未来可能替换 embedding 生成层为：
- 本地 C++ 推理引擎（如 llama.cpp / onnxruntime）
- 纯 Rust 推理引擎（如 rust-bert / candle）
- 外部 MCP / Skill 封装（embedding 作为独立服务）

**Embedding 状态**：
- `code_embeddings`: **56,722** 行（37.0% 覆盖率），覆盖 10 个仓库
- `skills.embedding`: 3 个 builtin skill 已有 384-dim 向量
- 生成工具：`tools/embedding-provider/skills.py`（sentence-transformers `all-MiniLM-L6-v2`）
- 激活路径：启动 Ollama + `devbase index <repo>` 生成 embedding，或配置远程 provider 于 `config.toml [embedding]` 段

## 上下文安全机制（Context Safety Mechanism）

> 长期架构原则：在多 Agent / 子代理协作场景下，保证工作区状态的一致性与可恢复性。

### 1. 子代理执行隔离

**教训**（2026-04-25 实际发生）：多个子代理在同一 Git 工作目录并行执行 `git checkout`/`git commit` 会导致严重的分支混乱。`agent-publish` 和 `agent-tui` 的修改互相覆盖，最终 commit 被错误地放置到对方分支， stash 中混入了不相关的代码。

**规则**：
- **串行优先**：多个子代理任务必须串行执行，每次 commit 后切回 main 再启动下一个
- **目录隔离**：若必须并行，每个子代理在独立的 `git clone` 临时目录工作，完成后由主会话 cherry-pick
- **禁止共享工作目录**：多个 Agent 绝不能同时操作同一个 `.git` 目录
- **编译检查**：任何子代理返回前必须通过 `cargo test --lib`，否则标记为脏状态

### 2. MCP 工具幂等性

**原则**：所有通过 MCP 暴露的状态变更操作必须是幂等的。

**实现**：
- `save_embeddings` — `ON CONFLICT(repo_id, symbol_name) DO UPDATE`
- `save_symbol_links` — `ON CONFLICT(source_repo, source_symbol, target_repo, target_symbol, link_type) DO NOTHING`
- `index_repo` — 先 `DELETE` 旧数据再 `INSERT`（而非追加）
- 所有批量操作包裹在 SQLite transaction 中

### 3. 状态变更审计追踪

**原则**：任何对 registry 的写入都必须留下不可变的审计痕迹。

**实现**：
- OpLog Schema v12+：`event_type` 枚举 + JSON metadata + `duration_ms`
- 所有 `scan`/`sync`/`health`/`index` 操作自动记录
- Schema 迁移前自动生成 `backup-YYYYMMDD-HHMMSS.db` 快照
- `registry export --format json` 支持用户自主备份

### 4. 知识库一致性契约

**原则**：存储层（devbase）与计算层（Clarity/Skill）之间的接口契约必须显式、可版本化。

**当前契约**：
| 方向 | 接口 | 版本 |
|------|------|------|
| 外部 → devbase | `devkit_embedding_store(repo_id, symbol_name, embedding[])` | v1 |
| devbase → 外部 | `devkit_hybrid_search(repo_id, query_text, query_embedding?, limit)` | v1 |
| devbase → 外部 | `devkit_knowledge_report(repo_id?, activity_limit)` | v1 |

**变更规则**：MCP tool schema 的 breaking change 必须通过新增 tool（如 `devkit_hybrid_search_v2`）而非修改现有 tool。

---

## 禁止事项

- 不得修改 `dev\third_party\*` 外部仓库
- 不得在没有迁移逻辑的情况下修改 registry schema
- 不得引入已 deprecated 的协议
- **不得在任何源码文件中硬编码真实 token、api_key 或密码**（包括注释和测试数据）
