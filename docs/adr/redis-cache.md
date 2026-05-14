# ADR: Redis 缓存评估

**状态**: 已拒绝（Rejected）  
**日期**: 2026-05-14  
**决策**: v0.19.0 阶段不引入 Redis，继续优化现有 SQLite + Tantivy 栈

---

## 背景

v0.19.0 Sprint C 要求评估 Redis 作为查询缓存层的必要性。devbase 当前查询路径：

1. **Registry 查询**: SQLite WAL 模式（本地文件）
2. **全文搜索**: Tantivy 内存映射索引（本地文件）
3. **混合检索**: SQLite LIKE / Tantivy BM25 + RRF 融合（内存计算）

## 瓶颈分析

基于 `HybridSearchMetrics` 和 OpLog 耗时埋点的观测数据：

| 查询类型 | 典型延迟 (10k docs) | 瓶颈 |
|---------|-------------------|------|
| Registry CRUD | < 5ms | 无瓶颈 |
| Keyword search (SQLite LIKE) | 30-80ms | SQLite 全表扫描 |
| Keyword search (Tantivy BM25) | 5-15ms | 磁盘 I/O |
| Hybrid search (RRF) | 10-50ms | 多路合并 + SQLite fallback |
| Vault graph (BFS) | 5-20ms | 内存遍历 |

**结论**: 10k 文档场景下 P99 < 200ms 已达成，无需外部缓存。

## Redis 能缓存什么？

| 缓存对象 | 命中率预期 | 收益评估 |
|---------|-----------|---------|
| 搜索查询结果 | 低（查询词高度多样） | 🟡 有限 |
| Registry 元数据 | 中（但 SQLite 已极快） | 🟢 微增益 |
| Tantivy Doc 内容 | 已由 OS page cache 覆盖 | ❌ 冗余 |
| Vault graph 子图 | 中（同一笔记多次遍历） | 🟡 中等 |

## 引入 Redis 的成本

| 维度 | 成本 |
|------|------|
| **依赖** | 新增外部服务，违反"本地优先"原则 |
| **部署** | 用户需安装/运行 Redis，Windows 体验差 |
| **数据一致性** | SQLite ↔ Redis 双写同步复杂度高 |
| **运维** | 内存限制、持久化策略、故障恢复 |
| **代码复杂度** | 需抽象 CacheBackend trait，增加 2-3 周工作量 |

## 替代方案（现有栈内优化）

1. **Tantivy reader 预热**: 启动时预加载 index reader，减少首次查询延迟
2. **SQLite query cache**: 利用 SQLite 自带的 `cache_size` PRAGMA（已默认启用）
3. **Vault graph 缓存**: BFS 子图结果在 AppContext 中缓存 5 分钟（已实现于 build_vault_graph）
4. **Index 常驻内存**: Tantivy 使用 MMAP，热点索引页由 OS 自动缓存

## 决策

**拒绝引入 Redis**。理由：

1. 单用户本地工具场景下，SQLite WAL + Tantivy MMAP 已满足 P99 < 200ms
2. 引入 Redis 的收益无法抵消其带来的依赖、部署、一致性成本
3. 现有栈内仍有优化空间（reader 预热、查询计划优化）

**重新评估触发条件**：
- >100k 文档场景下 P99 > 500ms（当前 10k 场景已达标）
- 多用户并发查询需求出现（与本地优先原则冲突）
- 跨网络分布式查询需求（v1.0+ 再评估）

---

*本 ADR 替代 `plans/redis-eval.md` 成为 Redis 决策的唯一活跃文档。*
