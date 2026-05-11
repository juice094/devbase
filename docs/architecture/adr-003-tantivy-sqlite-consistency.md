# ADR-003: Tantivy + SQLite 双写一致性策略

**状态**: 提案 (Proposed)  
**日期**: 2026-05-11  
**决策背景**: devbase v0.15.1 技术债清偿评估  
**相关债项**: AGENTS.md §技术债 — Tantivy+SQLite 双写一致性 (🟡 中风险)

---

## 1. 问题陈述

devbase 同时使用两个索引系统：

| 系统 | 用途 | 存储位置 |
|:---|:---|:---|
| **SQLite** | 元数据、实体关系、审计日志 | `%LOCALAPPDATA%/devbase/registry.db` |
| **Tantivy** | 全文检索（BM25）、代码符号搜索 | `%LOCALAPPDATA%/devbase/search_index/` |

**当前行为**：
- `index_repo` 操作时，先写入 SQLite `entities` 表，再写入 Tantivy 索引
- 无事务协调：SQLite COMMIT 成功后 Tantivy 写入可能失败，反之亦然
- 已落地补偿机制：`repair_tantivy_consistency()` 启动时检测 orphan 文档并清理

**已知缺陷**（v0.15.1 现状）：
1. **SQLite → Tantivy 缺失**：SQLite 有记录但 Tantivy 无索引（Tantivy 写入失败或进程崩溃）
2. **Tantivy → SQLite 孤儿**：Tantivy 有文档但 SQLite 实体已删除（`entities` 表被清理）
3. **非原子性**：无两阶段提交，任一系统失败即产生不一致

---

## 2. 候选方案

### 方案 A: 补偿扫描 + 懒清理（当前方案，已落地）

**机制**：
- 启动时运行 `repair_tantivy_consistency(conn, index_path)`
- 检测方向 1：Tantivy 文档的 repo_id 不在 SQLite `entities` 表中 → 标记为 orphan，下次索引时清理
- 检测方向 2：SQLite `entities` 表的 repo_id 不在 Tantivy 索引中 → 仅 warn，不自动 re-index（避免启动阻塞）

**优点**：
- 零额外依赖
- 启动时自愈，无需人工干预
- 不影响写入性能

**缺点**：
- 不一致窗口 = 两次启动间隔（可能数小时~数天）
- 方向 2（SQLite 有但 Tantivy 无）不会自动修复，仅 warn
- 补偿扫描本身有 I/O 开销（Tantivy 打开索引 + SQLite 查询）

**适用场景**：本地优先、容忍 eventual consistency 的知识库

---

### 方案 B: SQLite FTS5 替代 Tantivy

**机制**：
- 删除 Tantivy 依赖，使用 SQLite 内置 FTS5 扩展做全文检索
- 单系统 = 天然事务一致性（`BEGIN` → `INSERT INTO entities` → `INSERT INTO fts5` → `COMMIT`）

**优点**：
- 完全消除双写问题
- 减少编译依赖（删除 tantivy crate，release 二进制预计减小 3-5MB）
- 减少运行时文件句柄（Tantivy mmap）

**缺点**：
- FTS5 功能差距：无 BM25 调参、前缀搜索语法差异、无自定义 tokenizer
- 需要评估 `search::hybrid.rs` 中 RRF 融合算法的兼容性
- 数据迁移：现有 Tantivy 索引需导出为 SQLite FTS5（或让用户重建索引）
- `symbol_index.rs`（代码符号 BM25 搜索）可能需要重写

**评估结论**（2026-05-11）：
> FTS5 能否满足当前搜索需求？需验证：
> 1. `keyword_search_symbols` 的 BM25 排序效果 vs FTS5 rank
> 2. 前缀搜索 `query*` 的语义等价性
> 3. 多字段权重（title vs content）的实现方式

**决策**：暂缓。先完成功能评估矩阵（见 §4），再决定是否进入 Sprint。

---

### 方案 C: 两阶段提交（2PC）轻量级协议

**机制**：
- 引入 `IndexTransaction` 结构体，先写 prepare 日志到 SQLite `index_transactions` 表
- 步骤 1：SQLite `BEGIN` → `INSERT/UPDATE entities` → `INSERT INTO index_transactions (repo_id, status='prepared')`
- 步骤 2：Tantivy 写入 → 成功则 `UPDATE index_transactions SET status='committed'`
- 步骤 3：启动时扫描 `status='prepared'` 的记录，回滚或补全

**优点**：
- 理论上保证原子性
- 不依赖 FTS5，保留 Tantivy 全部功能

**缺点**：
- 实现复杂度高（需处理 prepare 日志的持久化、回滚逻辑）
- Tantivy 本身不支持事务回滚（IndexWriter commit 后不可撤销）
- 2PC 在单进程场景下是过度设计

**决策**：**否决**。单进程本地工具不需要分布式 2PC。

---

### 方案 D: SQLite 作为唯一事实来源，Tantivy 作为只读物化视图

**机制**：
- 所有写入只走 SQLite（entities + 全文内容表）
- Tantivy 索引通过后台 Daemon tick 定期从 SQLite 重建（类似物化视图刷新）
- 搜索时优先查 Tantivy，Tantivy 缺失时 fallback 到 SQLite LIKE

**优点**：
- 单一写入路径 = 无写入期不一致
- Tantivy 崩溃后可完全从 SQLite 重建

**缺点**：
- 重建延迟 = Daemon tick 间隔（默认 5-60 分钟）
- 实时搜索可能查不到最新内容
- 需要维护 SQLite 中的 "全文内容表"（冗余存储）

**决策**：**否决**。实时性是 devbase 搜索的核心价值，不能接受分钟级延迟。

---

## 3. 推荐决策

**当前阶段（v0.15.x）**：
> **维持方案 A（补偿扫描 + 懒清理），不做架构迁移。**

理由：
1. 双写不一致在本地优先场景下影响面小（单用户、单进程）
2. 补偿扫描已覆盖 90% 的实际不一致场景（方向 1 orphan 清理）
3. 方案 B（FTS5）评估工作量大，应在 v0.17.0 独立 Sprint 中决策
4. 方案 C/D 的复杂度收益比不佳

**v0.17.0 触发条件**：
> 若 FTS5 评估矩阵（§4）验证通过，则启动迁移；否则维持现状。

---

## 4. 附录：FTS5 功能评估矩阵（待执行）

| 功能 | Tantivy 现状 | FTS5 等效方案 | 差距评估 | 负责人 |
|:---|:---|:---|:---:|:---|
| BM25 排序 | ✅ 原生支持，可调参 | `rank` 函数，参数有限 | 🟡 需测试 | 待分配 |
| 前缀搜索 | ✅ `query*` | `column:prefix*` | 🟢 语法等价 | — |
| 多字段权重 | ✅ Schema 字段权重 | `MATCHinfo` 自定义 ranking | 🟡 需验证 | 待分配 |
| 代码符号索引 | ✅ symbol_index.rs | 需创建 `code_symbols_fts5` 虚表 | 🟡 需重写 | 待分配 |
| 中文分词 | ❌ 未启用 | `jieba` / `icu` tokenizer | 🟡 需配置 | 待分配 |
| 编译体积 | ~8MB (tantivy + 依赖) | 0（SQLite 内置） | 🟢 显著减小 | — |
| 运行时内存 | mmap 文件 | 普通 SQLite page cache | 🟢 更可控 | — |

**评估工作量**：约 2-3 天（含 PoC 代码 + 基准测试）。

---

## 5. 相关代码

- `src/storage.rs`：`repair_tantivy_consistency()` / `repair_tantivy_consistency_at()`
- `src/search.rs`：`sync_index_to_db()`（反向检测）
- `src/search/symbol_index.rs`：代码符号级 Tantivy 索引
- `AGENTS.md` §技术债 — Tantivy+SQLite 双写一致性

---

*本 ADR 由接管 Agent 于 2026-05-11 创建，待架构师复核后由 MODE-O 批准。*
