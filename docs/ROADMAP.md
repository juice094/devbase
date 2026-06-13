# devbase Roadmap

> **当前阶段**：阶段十一 — v0.20.1 知识完备性（已发布）
>
> **最后更新**：2026-06-13
>
> **版本状态**：`0.20.1`（Schema 36，71 MCP tools，605 tests）

---

## 历史阶段（已完成）

### 阶段一：产品化闭环（v0.3.0）— ✅

34 MCP tools 全量通过 MCP Inspector，README Quick Start 三步跑通，Tests 全绿。

### 阶段二：AI Skill 编排基础设施（v0.4.0–v0.9.0）— ✅

Schema v16 统一实体模型、Skill 自动封装、Workflow Engine、Mind Market 评分、NLQ 自然语言查询、Workflow Loop Step 硬化。

### 阶段三：自指知识库（v0.10.0–v0.11.0）— ✅

L0-L4 五层知识模型 MVP：entities 统一模型、known_limits 风险层、knowledge_meta 元认知层、PARA vault 结构。

### 阶段四：工程健康与解耦（v0.12.0–v0.14.0）— ✅

Registry God Object 拆解、AppContext Pool 化、生产 unwrap 清零、Workspace 骨架搭建（18 crates）、全模块耦合地图。

### 阶段五：数据层 + 可靠性 + Agent 体验（v0.15.0）— ✅

三维 embedding 主键、Tantivy-SQLite Saga 一致性扫描、`devbase status` + MCP streaming。

### 阶段六：分发就绪与 Embedding 外迁（v0.16.0–v0.17.0）— ✅

Workspace 扩展至 18 crates、Embedding Externalization（Candle/Ollama 降级为 opt-in）、Schema 34 向量存储 + `cosine_similarity` SQLite UDF、Agent Contexts / Session 记忆体系。

### 阶段七：ClaudeCode 集成与世界模型定位（v0.18.0）— ✅

`devkit_project_brief` / `impact_analysis`、Session 导出/导入、`devbase-claude.ps1` 启动器、World Model Compiler 定位升级、根目录治理、NotebookLM 生态消化（5 项目注册）、GreptimeDB 互补分析。

### 阶段十一：知识完备性（v0.20.0）— ✅

**核心目标**：从"能存"到"好用"，消除知识库能力缺口。

| Sprint | 主题 | 关键交付 | 状态 |
|--------|------|---------|------|
| **Sprint E — Vault 历史追踪** | Git-based 笔记历史 | `devkit_vault_history`：基于 git2 revwalk 的笔记变更追踪；blob diff 行级统计；`VaultClient::get_vault_history()` | ✅ |
| **Sprint F — 检索质量监控** | 混合检索可观测 | `devkit_search_quality`（第 68 个 tool）返回 `HybridSearchMetrics`：latency_ms / keyword_recall / vector_recall / rrf_overlap / keyword_source；RRF 参数可调 | ✅ |
| **Sprint G — Block 引用** | 笔记块级引用 | `WikiLink.anchor` 支持 `[[note#heading]]` 与 `[[note#^block-id]]`；`VaultNote.block_refs` 字段；导出时 broken block ref 检测 | ✅ |
| **Sprint C — 性能基线** | 查询延迟红线 | `#[ignore]` 性能回归测试：1k 文档 <200ms、10k 文档 <500ms；Redis 缓存 ADR 决策：放弃，现有栈已足够 | ✅ |
| **客户端泄漏清理** | 客户端无关原则 | `skill sync` 泛化接口（移除硬编码 `.clarity` 路径）；`AGENTS.md` 新增 Client-Agnostic Principle | ✅ |
| **双向链接图遍历** | BFS 知识图谱 | `devkit_vault_graph` 支持 `note_id` + `depth` 参数；DB-first 构建 + 双向 traversal（outgoing + incoming） | ✅ |

**v0.20.0 验收标准**：
1. ✅ `cargo test` 全绿（451 passed / 5 ignored）
2. ✅ Vault 笔记支持块级引用（heading + block-id）
3. ✅ 混合检索质量可观测（keyword/vector 召回、RRF 重叠、后端来源）
4. ✅ Vault 历史可回溯（Git-based，含行级 insertions/deletions）
5. ✅ 双向链接图遍历深度 1-3，BFS 实现
6. ✅ 客户端无关原则落地：核心能力零硬编码客户端路径

---

## 当前阶段：阶段十二 — v0.21.0 外部能力嫁接（规划中）

**核心目标**：消除"玩具感"，将 devbase 从"功能演示级"推进到"日常生产力级"。**存储可靠性 > AI 炫技**。

> **核心原则**：devbase 首先是一个可靠的本地知识基础设施，然后才是一个 World Model Compiler。详见 [AGENTS.md](../AGENTS.md) §知识库生产级缺口与补齐路线。

### v0.19.0 Sprint 规划

| Sprint | 主题 | 关键交付 | 目标日期 |
|--------|------|---------|----------|
| **Sprint A — SQLite 可靠性** | WAL 模式 + 并发安全 | `PRAGMA journal_mode=WAL` 默认启用；并发写入测试覆盖；迁移回滚硬化 | ✅ 2026-05 |
| **Sprint B — 索引健康度** | Tantivy 可观测与自愈 | `devkit_index_health` tool（健康评分 0-100）；`--repair` 自动修复；损坏检测 | ✅ 2026-05 |
| **Sprint C — 性能基线** | 查询延迟可观测 | OpLog 查询耗时埋点 ✅；性能回归测试（1k/10k 文档阈值）✅；Redis ADR 放弃决策 ✅ | ✅ 2026-05 |
| **Sprint D — 数据自由** | Vault 导出与互操作 | `devkit_vault_export` 完整 PARA 导出；frontmatter 兼容性验证；Vendor Lock-in 消除 | ✅ 2026-05 |

**v0.19.0 验收标准**：
1. ✅ `cargo test` 全绿 + CI 通过（性能回归红线移至 Sprint C）
2. ✅ `devkit_index_health` 可返回所有注册仓库的索引健康评分，支持 `--repair` 自动修复
3. ✅ SQLite WAL 模式在所有新创建/迁移的数据库上默认启用
4. ✅ Vault 导出可通过标准 Markdown 工具链（如 Obsidian）无损重新导入（38 文件验证通过）

**v0.19.0–v0.20.0 约束（已解除）**：
- ✅ v0.19.0：禁止新增非可靠性相关的 MCP Tool（已解除）
- ✅ v0.19.0：禁止引入外部数据库依赖（GreptimeDB、Redis、PostgreSQL 仅评估，不集成）→ Redis ADR 已决策：放弃引入
- ✅ 世界模型研究继续独立仓库推进

---

## 技术债务（清偿中）

| 债项 | 严重 | 当前值 | 目标 | 清理路径 | 版本 |
|------|------|--------|------|----------|------|
| Tantivy+SQLite 双写一致性 | 🔴 | 无事务协调，反向检测已落地 | 补偿机制 + 健康评分 | `devkit_index_health` + WAL | v0.19.0 |
| SQLite 单文件并发锁定 | 🔴 | DELETE journal_mode | WAL mode | `PRAGMA journal_mode=WAL` | v0.19.0 |
| 查询性能不可观测 | 🟢 | `#[ignore]` 回归测试已落地（1k<200ms, 10k<500ms）；OpLog 延迟埋点 | P99 < 200ms @ 10k | CI 性能回归 + OpLog 指标 | v0.20.0 |
| tree-sitter 编译成本 | 🟡 | ~15-20s | <10s | ccache 或 grammar 预编译 | v0.20.0 |
| Vault 无版本历史 | 🟢 | `devkit_vault_history` + git2 revwalk + blob diff | Git 追踪或增量表 | vault 目录作为 Git 子模块（用户侧） | v0.20.0 |
| Feature flags 完善 | 🟡 | 4 个（tui, watch, mcp, embedding） | ≥5 | `llm-backend` feature 细分 | v0.20.0 |
| `init_db()` 全局路径 | 🟢 | 5 处 grandfathered | 0 新增 | StorageBackend trait 已奠基 | 持续 |

---

## 版本规划

| 版本 | 主题 | 关键交付 | 预计时间 |
|------|------|----------|----------|
| v0.19.0 | ✅ **知识基础设施硬化** | SQLite WAL + Tantivy 健康评分 + Vault 导出 + Redis 决策 | 2026-05 |
| v0.20.0 | ✅ **知识完备性** | 双向链接图遍历 + 笔记历史追踪 + 混合检索质量监控 + block 引用 + 性能回归基线 | 2026-05 |
| v0.21.0 | **外部能力嫁接** | GreptimeDB 观测层评估 + Open Notebook 管道对接 + SurfSense Agent 参考 | 2026-08 |
| v0.22.0 | **规模化验证** | >100 仓库场景测试 + 索引分片评估 + 查询缓存 | 2026-Q3 |
| v0.25.0 | **分发发布** | 首个 crate (`devbase-mcp` 或 `devbase-core`) 发布到 crates.io | 2026-Q4 |

---

## Future / Icebox（无排期，但已注册参考项目）

- **GreptimeDB 集成**：时序观测层、Flow Engine 流式知识加工、向量索引统一评估（待 v1.1 向量索引成熟）
- **Open Notebook 嫁接**：多说话人播客/测验生成管道作为外部 MCP Tool
- **SurfSense 参考**：Agent 协作与多 LLM 路由模式融入 Clarity 三角色
- **跨设备注册表同步**：syncthing-rust 集成（REST API 待就绪）
- **形式化验证 / TEE 集成**：长期，无排期
- **生长信号与遗忘机制**：L0-L4 知识模型的自动衰减

---

## 明确不做（Deferred / 已排除）

| 功能 | 原因 | 状态 |
|------|------|------|
| SSE transport | stdio 已覆盖主流 Client，维护负担高 | ❌ 排除 |
| `.devbase` 目录规范 | 无外部采纳者 | ❌ 排除 |
| MCP 协议扩展提案 | Star = 0，不会被采纳 | ❌ 排除 |
| 商业化 / 付费版 | 与本地优先原则冲突 | ❌ 排除 |
| 主仓库引入 Spark/Flink | 研究性质，独立仓库处理 | ❌ 排除（红线） |
| v0.19.0 引入 Redis/GreptimeDB | 可靠性加固需在现有栈内完成 | ❌ 排除（阶段约束） |

---

*本 Roadmap 替代 `plans/roadmap-2026.md` 成为唯一活跃主路线图。*
*历史计划见 `docs/_archive/`。*
