# Devbase 架构实证评审报告

> ⚠️ **历史文档**：本报告基于 2026-04-15 的代码状态（v0.1.x，约 10 个 MCP tool）。当前项目已演进至 v0.2.3（19 个 tools，Registry 已拆分，MCP tools 已模块化）。部分结论已过时，仅供参考。

> 评审日期：2026-04-15  
> 评审范围：MCP 传输层、并发模型、Registry Schema、Query 引擎、功能边界  
> 方法论：对照业界成熟项目与协议规范，逐项评定

---

## 一、查询：可参照的成熟项目与规范

### 1.1 MCP 生态（传输层）

| 项目/规范 | 版本 | 关键事实 |
|-----------|------|----------|
| MCP 官方规范 | 2025-03-26 | **Streamable HTTP 取代 HTTP+SSE** |
| MCP 官方规范 | 2025-11-25 | 新增 Tasks（长运行操作）、Elicitation、OAuth 2.1 |
| Taskade MCP Survey | 2026-04 | SSE 标记为 Deprecated，Atlassian 2026-06 sunset |
| rust-mcp-schema | 2025-02 | 自动生成、全版本兼容的 Rust Schema 库 |
| mcpkit (Praxiom) | 2025-12 | `#[mcp_server]` 宏、Tower middleware、runtime-agnostic |
| rust-mcp-sdk | 2025+ | 基于 rust-mcp-schema 的高性能异步 toolkit |

**结论**：MCP 传输层在 2025-03 发生了根本性变革。SSE 已被废弃，Streamable HTTP 成为唯一推荐的远程传输方式。

### 1.2 并发模型（Tokio 死锁）

| 来源 | 日期 | 关键事实 |
|------|------|----------|
| Turso 博客 | 2024-09 | `std::sync::Mutex` + `tokio::spawn` + `block_on` = 已知死锁模式 |
| Tokio GitHub #1998 | 2019-12 | `Semaphore` permit 传入 spawned future 的 lifetime 问题 |
| Rust Users Forum | 2025-02 | `block_in_place` + `block_on` + I/O driver lock = 死锁 |

**正确解法**（Turso 验证）：
- 将 `std::sync::Mutex` 替换为 `tokio::sync::Mutex`
- blocking 上下文使用 `mutex.blocking_lock()`
- **不是**降级为 sequential execution

### 1.3 Registry / 知识库设计

| 项目 | 关键事实 |
|------|----------|
| **qmd** (tobi/qmd) | 2025-12 从 SQLite 迁移到 YAML："SQLite was overkill for config — you can't share it, and it's opaque" |
| **Kiro CLI** | 知识库使用 bm25（词法）+ all-MiniLM（语义）双索引，按 agent 隔离 |
| **ghq** | 只做一件事：管理远程仓库的本地克隆（~3k stars） |
| **gws** | 只做一件事：管理 git 工作区状态（~1k stars） |

### 1.4 工作区管理工具

| 工具 | 功能范围 | 设计哲学 |
|------|----------|----------|
| ghq | 远程仓库本地克隆管理 | Unix 哲学：Do one thing well |
| gws | 多仓库 git 状态检查 | Unix 哲学 |
| repo-man | 仓库元数据查询 | 极简 CLI |
| devbase (当前) | registry + sync + health + query + MCP + daemon + TUI | **功能爆炸** |

---

## 二、比较：Devbase vs 业界实践

### 2.1 MCP 传输层

| 维度 | Devbase (Sprint 2) | 业界标准 (2026) |
|------|-------------------|-----------------|
| 传输协议 | 自建 HTTP+SSE (`/sse` + `/sse/messages`) | **Streamable HTTP** (`/mcp` 单端点) |
| 流式机制 | 自建 `ToolEvent` enum + `invoke_stream()` trait | 官方 `Task` + `Progress` + `Cancelled` |
| Schema 兼容 | 无（手写 JSON） | `rust-mcp-schema` 自动生成 |
| 协议版本 | 2024-11-05 非正式实现 | 2025-11-25 正式规范 |
| 认证 | 无 | OAuth 2.1 + PKCE |

### 2.2 并发模型

| 维度 | Devbase (当前) | 业界最佳实践 |
|------|---------------|--------------|
| ASYNC 模式 | Sequential fallback (FIXME) | `tokio::sync::Mutex` + `blocking_lock()` |
| Semaphore 使用 | `acquire_owned()` + `tokio::spawn` | `acquire()` + 直接 await |
| 死锁处理 | 绕过（降级为同步） | 根因修复（更换 Mutex 类型） |

### 2.3 Registry Schema

| 维度 | Devbase (v5) | 业界实践 |
|------|-------------|----------|
| 表数量 | 12 张 | qmd: 0（已迁出 SQLite）；ghq: 0 |
| 实际使用率 | repos/health/snapshots/oplog = 高；其余 = 极低 | 按需设计 |
| Schema 演进 | 手动 migration | 无 Schema（YAML/JSON）或 ORM |
| 可共享性 | 二进制 SQLite 文件 | YAML 可版本控制 |

### 2.4 Query 引擎

| 维度 | Devbase (当前) | 业界标准 |
|------|---------------|----------|
| 索引类型 | 无（前缀字符串匹配） | bm25 + 语义嵌入 |
| 搜索质量 | 关键词精确匹配 | 语义理解 + 相关性排序 |
| 响应速度 | 全表扫描 | 倒排索引 |

---

## 三、评定：逐项打分

| # | 架构决策 | 状态 | 风险等级 | 技术债务 | 说明 |
|---|---------|------|----------|----------|------|
| 1 | **MCP SSE Transport** | ❌ 严重缺陷 | 🔴 高 | ~400 LOC | 协议已 deprecated，2026 后无客户端兼容 |
| 2 | **`ToolEvent` 流式 trait** | ❌ 非标实现 | 🔴 高 | ~200 LOC | 与任何 MCP SDK 不兼容 |
| 3 | **Sync sequential fallback** | ⚠️ 临时补丁 | 🟡 中 | ~20 LOC | 性能退化，未根因修复 |
| 4 | **Registry v5 (12 tables)** | ⚠️ 过度设计 | 🟡 中 | ~600 LOC | 维护成本高，6+ 表几乎未使用 |
| 5 | **clarity-core path dep** | ⚠️ 耦合 | 🟡 中 | ~150 LOC | 提取成本未评估 |
| 6 | **Query 前缀匹配** | ⚠️ 功能不足 | 🟡 中 | ~100 LOC | 搜索质量差，无量化评测 |
| 7 | Non-Git workspace 支持 | ✅ 合理 | 🟢 低 | - | 有实际需求，ghq 不支持 |
| 8 | `.syncdone` marker | ✅ 合理 | 🟢 低 | - | syncthing 等工具类似实践 |
| 9 | OpLog | ✅ 合理 | 🟢 低 | - | 审计追踪，标准实践 |
| 10 | Registry backup | ✅ 合理 | 🟢 低 | - | 迁移前自动备份 |

---

## 四、规划：重构路线图

### Phase A —— 止损（2-3 天，立即执行）

**目标**：消除最高风险的技术债务

1. **MCP 传输层**：
   - 方案 A1：移除 SSE 实现，保留 stdio 作为唯一传输方式
   - 方案 A2：迁移到 Streamable HTTP（单端点）
   - **建议 A1**：devbase 是本地 CLI 工具，stdio 足够；SSE 是错误的方向

2. **Sync 死锁**：
   - 将 `std::sync::Mutex` 或 `Semaphore` 模式替换为 `tokio::sync::Mutex`
   - 恢复 ASYNC 模式的真正并发能力
   - 参考 Turso 实证文章修复

### Phase B —— 精简（1 周）

**目标**：Registry 瘦身，降低维护成本

1. **Schema 审计**：
   - papers / experiments / agri_observations：当前无任何 CLI 命令或 MCP 工具调用
   - ai_queries / ai_discoveries / repo_notes：同上
   - repo_summaries / repo_modules / repo_relations：仅在 scan 时写入，无任何读取路径

2. **迁移方案**：
   - 方案 B1：删除未使用表，保留 core 5 表（repos, repo_tags, repo_remotes, repo_health, workspace_snapshots）+ oplog
   - 方案 B2：将 config 类数据（tags, remotes）迁移到 YAML，SQLite 只保留运行时数据（health, snapshots）
   - **建议 B1**：先删除，后续若需要再加

3. **clarity-core 解耦**：
   - 评估：当前 clarity-core 提供什么功能？
   - 若仅为 `PersonalityConfig`，考虑内联或移除

### Phase C —— 增强（2 周）

**目标**：Query 引擎和 MCP 工具的真正价值

1. **Query 引擎**：
   - 引入 `tantivy`（Rust 全文搜索库，bm25）作为可选依赖
   - 索引 README、 Cargo.toml、源码注释
   - 保留现有前缀查询作为 fallback

2. **MCP 工具精简**：
   - 当前 10 个工具，评估每个工具的调用频率
   - 合并或删除低频工具

3. **TUI 体验**：
   - ratatui 0.30 已稳定，评估是否需要更多交互功能

### Phase D —— 验证（持续）

1. 建立架构决策记录（ADR）目录
2. 每次重大架构变更前，强制要求：
   - 至少 2 个同类项目的实证参考
   - 风险评估文档
   - 回滚方案

---

## 五、推进：下一步行动

### 立即决策项

| # | 决策 | 选项 | 建议 |
|---|------|------|------|
| 1 | MCP SSE 处理 | A) 移除 B) 迁移 Streamable HTTP | **A) 移除** — 本地 CLI 不需要远程传输 |
| 2 | Sync 死锁修复 | A) 换 tokio::sync::Mutex B) 保持 sequential | **A) 修复** — 恢复并发能力 |
| 3 | Registry 瘦身 | A) 删未使用表 B) 迁移 YAML C) 保持现状 | **A) 删表** — 最小可用原则 |
| 4 | Query 增强 | A) 引入 tantivy B) 保持现状 C) 移除 query | **A) 引入 tantivy** — 提升核心价值 |

### 时间估算

- Phase A（止损）：2-3 天
- Phase B（精简）：3-5 天
- Phase C（增强）：7-10 天
- 总计：**约 2-3 周**

---

## 附录：参考链接

1. [Turso: How to deadlock Tokio with a single mutex](https://turso.tech/blog/how-to-deadlock-tokio-application-in-rust-with-just-a-single-mutex) — 2024-09
2. [Taskade: 15 Best MCP Servers 2026](https://www.taskade.com/blog/mcp-servers) — SSE deprecated
3. [qmd: SQLite → YAML migration](https://github.com/tobi/qmd/blob/main/CHANGELOG.md) — 2025-12
4. [Kiro CLI Knowledge Management](https://kiro.dev/docs/cli/experimental/knowledge-management/) — bm25 + semantic
5. [MCP Streamable HTTP Guide](https://auth0.com/blog/mcp-streamable-http/) — 2025-12
6. [rust-mcp-schema](https://github.com/rust-mcp-stack/rust-mcp-schema) — 类型安全 Schema
7. [mcpkit](https://github.com/praxiomlabs/mcpkit) — Rust MCP SDK 宏
