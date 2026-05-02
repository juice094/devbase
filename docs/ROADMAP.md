# devbase Roadmap

> **当前阶段**：阶段六 — v0.16.0 分发就绪（进行中）
>
> **最后更新**：2026-04-26
>
> **版本状态**：`0.16.0-dev`（P2 Workspace crate 第二批提取 #1 完成：`devbase-workflow-interpolate`）

---

## 历史阶段（已完成）

### 阶段一：产品化闭环（v0.3.0）— ✅

34 MCP tools 全量通过 MCP Inspector，README Quick Start 三步跑通，Tests 全绿。

### 阶段二：AI Skill 编排基础设施（v0.4.0–v0.9.0）— ✅

Schema v16 统一实体模型、Skill 自动封装、Workflow Engine、Mind Market 评分、NLQ 自然语言查询、Workflow Loop Step 硬化。

### 阶段三：自指知识库（v0.10.0–v0.11.0）— ✅

L0-L4 五层知识模型 MVP：entities 统一模型、known_limits 风险层、knowledge_meta 元认知层、PARA vault 结构。

### 阶段四：工程健康与解耦（v0.12.0–v0.14.0）— ✅ 已交付

| 任务 | 交付物 | 状态 |
|------|--------|------|
| Registry God Object 拆解 | 10 子模块提取为 free-function 模块 | ✅ v0.12.0 |
| AppContext Pool 化 | `r2d2::Pool` 替代单 Connection，22 处调用点迁移 | ✅ v0.12.0 |
| 生产 unwrap 清零 | 0 个生产 unwrap，`clippy -D warnings` 全绿 | ✅ v0.13.0 |
| 测试覆盖率收尾 | 437 workspace tests passed，零测试文件清零 | ✅ v0.13.0 |
| Workspace 骨架搭建 | `crates/` 目录 + 3 个零耦合模块提取 | ✅ v0.14.0 |
| 全模块耦合地图 | 按 `crate::` 引用数扫描，🟢/🟡/🔴 分级 | ✅ v0.14.0 |

### 阶段五：v0.15.0 数据层 + 可靠性 + Agent 体验 — ✅ 已交付

| Sprint | 核心交付 | 状态 |
|--------|---------|------|
| Sprint A — 数据层 + 性能 | v28 三维 embedding 主键；rayon 并行化（130s→~20s） | ✅ `dfdc1cc` |
| Sprint B — 可靠性 | Tantivy-SQLite Saga 一致性扫描 + orphan 懒清理 | ✅ `dcbe256` |
| Sprint C — Agent 体验 | `devbase status` + `DevkitStatusTool` + MCP streaming | ✅ `e8860ba` |

---

## 当前阶段：阶段六 — v0.16.0 分发就绪（进行中）

**核心目标**：让 devbase 的通用组件达到"可独立发布"标准，同时保持主 crate 的健康度。

> 分发 ≠ 必须发布到 crates.io。分发标准是耦合健康度的检验手段：能拆 = 健康，不能拆 = 有债。

---

## 待办清单（按优先级）

### P0 — Workspace 扩展（本周–本月）

提取 🟢 健康模块（0-3 个 `crate::` refs）为独立 crate。

| 候选模块 | 行数 | 测试 | 内部耦合 | 估计工时 |
|----------|------|------|----------|----------|
| `embedding` | 298 | 0 | 0 | 15 min |
| `syncthing_client` | 85 | 2 | 0 | 15 min |
| `registry/health` | 156 | 5 | 0 | 15 min |
| `registry/metrics` | 153 | 3 | 0 | 15 min |
| `registry/workspace` | 215 | 5 | 0 | 15 min |
| `vault/frontmatter` | 175 | 0 | 0 | 15 min |
| `vault/wikilink` | 130 | 0 | 0 | 15 min |
| ~~`workflow/interpolate`~~ | ~~239~~ | ~~9~~ | ~~0~~ | ~~✅ 已完成~~ |
| ~~`workflow/model`~~ | ~~330~~ | ~~2~~ | ~~0~~ | ~~✅ 已完成~~ |
| ~~`embedding`~~ | ~~299~~ | ~~5~~ | ~~0~~ | ~~✅ 已完成~~ |
| ~~`skill_runtime/parser`~~ | ~~417~~ | ~~3~~ | ~~0~~ | ~~✅ 已完成（需先提取 types）~~ |
| ~~`registry/health`~~ | ~~156~~ | ~~5~~ | ~~0~~ | ~~✅ 已完成~~ |
| ~~`registry/metrics`~~ | ~~153~~ | ~~3~~ | ~~0~~ | ~~✅ 已完成~~ |
| ~~`registry/workspace`~~ | ~~215~~ | ~~5~~ | ~~0~~ | ~~✅ 已完成~~ |
| `workflow/model` | 330 | 0 | 0 | 15 min |
| `skill_runtime/parser` | 417 | 0 | 0 | 15 min |

**目标**：workspace 成员达到 8-10 个。当前：13 个（✅ 已超额达成）。
**验收**：`cargo check --workspace` 0 errors，`cargo test --workspace` 全绿。

---

### P1 — MCP trait 化（本月–下月）

**问题**：`mcp/tools/repo.rs` 有 **70 个 `crate::` 引用**，是 devbase 最大耦合黑洞。

**方案**：
1. 定义 `RegistryClient` trait：
   ```rust
   pub trait RegistryClient {
       fn list_repos(&self, conn: &rusqlite::Connection, filter: &str) -> Vec<RepoEntry>;
       fn get_repo(&self, conn: &rusqlite::Connection, id: &str) -> Option<RepoEntry>;
       // ... 其他 repo 相关操作
   }
   ```
2. 定义 `SearchClient` trait：
   ```rust
   pub trait SearchClient {
       fn hybrid_search(&self, query: &str, limit: usize) -> Vec<SearchResult>;
   }
   ```
3. `mcp/tools` 从 `crate::registry::*` 改为 `trait` 调用
4. devbase 主 crate 实现这些 trait

**阻塞**：需要确定 trait 边界——哪些操作属于 RegistryClient，哪些属于 SearchClient。
**验收**：`mcp/` 目录的 `crate::` 引用数从 70 降至 <10。

---

### P2 — registry 子模块拆分（本月）

`registry/health`, `registry/metrics`, `registry/workspace`, `registry/entity`, `registry/relation` 已零耦合，可直接提取为 workspace crate 或保持为子模块但消除 `crate::` 引用。

**决策点**：registry 子模块是否值得独立为 crate？
- 若作为独立 crate：`devbase-registry-health` 等
- 若保持子模块：确保它们只对 `rusqlite::Connection` 有依赖

**推荐**：暂时保持子模块结构（避免 crate 数量爆炸），但消除所有 `crate::` 引用，使它们达到"随时可提取"状态。

---

### P3 — migrate.rs 拆解（长期，阻塞）

| 属性 | 值 |
|------|-----|
| 行数 | 1273 |
| 耦合 | 4 `crate::` refs（低） |
| 阻塞原因 | 含 schema 迁移 + DDL + 数据转换，风险极高 |
| 解耦策略 | 按 schema 版本切分：`migrate/v16.rs`, `migrate/v17.rs`... |
| 所需架构 | Claw（多轮持久化会话） |

**当前状态**：⏳ 等待 Claw 架构就绪。

---

## 技术债务（清偿中）

| 债项 | 严重 | 当前值 | 目标 | 清理路径 |
|------|------|--------|------|----------|
| Tantivy+SQLite 双写一致性 | 🟡 | 无事务协调 | 补偿机制或 FTS5 替代 | 评估 `sync_index_to_db()` 两阶段提交 |
| tree-sitter 编译成本 | 🟡 | ~15-20s | <10s | ccache 或 grammar 预编译 |
| Feature flags 缺失 | 🟡 | 2/3（tui, watch） | ≥3 | 评估 mcp 是否独立 feature |
| `init_db()` 全局路径 | 🟢 | 5 处 grandfathered | 0 新增 | StorageBackend trait 已奠基，迁移中 |
| `SortMode` unused import | 🟢 | 1 warning | 0 | `cargo fix` 或手动移除 |

---

## Future / Icebox（无排期）

- 跨设备注册表同步（syncthing-rust 集成，REST API 待就绪）
- 形式化验证 / TEE 集成（长期，无排期）
- Workflow 引擎细化（Loop body Retry/Fallback、TUI 执行进度条）
- 生长信号与遗忘机制（L0-L4 知识模型的自动衰减）
- `devbase-mcp` 独立发布（待 MCP trait 化完成后）

---

## 明确不做（Deferred / 已排除）

| 功能 | 原因 | 状态 |
|------|------|------|
| SSE transport | stdio 已覆盖主流 Client，维护负担高 | ❌ 排除 |
| `.devbase` 目录规范 | 无外部采纳者 | ❌ 排除 |
| MCP 协议扩展提案 | Star = 0，不会被采纳 | ❌ 排除 |
| 商业化 / 付费版 | 与本地优先原则冲突 | ❌ 排除 |
| ~~拆分 crate~~ | ~~22.7 KLOC 单 crate 仍最优~~ | ~~→ 已推翻，v0.14.0 已启动拆分~~ |

---

## 版本规划

| 版本 | 主题 | 关键交付 | 预计时间 |
|------|------|----------|----------|
| v0.15.0 | 数据层 + 可靠性 + Agent 体验 | Workspace 成员 6 个，三维 embedding + Saga 一致性 + MCP Streaming | ✅ 2026-05 |
| v0.16.0 | Workspace 扩展 Phase 2 | Workspace 成员 8-10 个，debug 稳定性修复 | 2026-05 |
| v0.17.0 | MCP 解耦 + registry 清洁 | mcp/tools/repo.rs `crate::` 引用 <10，registry 子模块零耦合 | 2026-06 |
| v0.20.0 | 分发发布 | 首个 crate (`devbase-mcp`) 发布到 crates.io | 2026-07+ |

---

*本 Roadmap 替代 `plans/roadmap-2026.md` 成为唯一活跃主路线图。*
*历史计划见 `docs/archive/`。*
