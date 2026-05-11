# Architecture Decision Record (ADR) 模板

> 来源：架构治理方法论参考（Kimi 会话 `e9f2965f-b949-46a5-9d7c-afd6d4d9232c`）
> 用途：记录任何"为什么不那样做"的决策，一句话 + 一句话后果。

---

## ADR-XXX: [标题]

- **状态**: proposed / accepted / deprecated / superseded by ADR-YYY
- **日期**: YYYY-MM-DD
- **作者**: [名字/会话ID]

### 上下文

[背景：当时面临什么问题，有哪些可选方案]

### 决策

[明确陈述：选择了什么]

### 后果

- **正面**：[带来了什么好处]
- **负面**：[付出了什么代价]
- **风险**：[未来可能产生的问题]

### 备选方案

| 方案 | 不选原因 |
|------|---------|
| [方案A] | [一句话理由] |
| [方案B] | [一句话理由] |

### 相关决策

- 依赖：ADR-ZZZ
- 被依赖：ADR-WWW
- 替代：ADR-YYY

---

## 已完成的 ADR 索引

| 编号 | 标题 | 状态 | 日期 |
|------|------|------|------|
| ADR-001 | 单 crate 模型（defer split）| accepted | 2026-04-26 |
| ADR-002 | Candle CPU BERT 单条编码（batch 回滚）| accepted | 2026-05-04 |
| ADR-003 | Tantivy + SQLite 双写一致性策略 | proposed | 2026-05-11 |
| ADR-004 | MCP Tool Layer Trait Decoupling | accepted | 2026-05-11 |
| ADR-005 | AppContext Clone for Async Context Propagation | accepted | 2026-05-11 |

### ADR-001: 单 crate 模型（defer split）

- **状态**: accepted
- **日期**: 2026-04-26
- **作者**: devbase 架构审计

**上下文**: v0.2.4 时 22.7 KLOC，评估是否拆分为 workspace crates。

**决策**: Defer split。保持单 crate，触发条件：50+ MCP tools / clean build > 60s / binary > 20 MB。

**后果**:
- 正面：迭代更快，无跨 crate 版本协调
- 负面：编译缓存粒度粗，模块化约束靠自觉
- 风险：长期可能积累隐式耦合，需定期提取演习验证

### ADR-002: Candle CPU BERT 单条编码（batch 回滚）

- **状态**: accepted
- **日期**: 2026-05-04
- **作者**: devbase 性能优化会话

**上下文**: Index 冷启动 17s，embedding 占 98.5%。尝试 batch_size=32 降低 forward 次数。

**决策**: 回滚到 `rayon::par_iter()` 单条编码；保留 `encode_batch` trait 供未来 GPU/ONNX provider。

**后果**:
- 正面：恢复 16s 基线；新增 `--skip-embeddings` 路径（0.25s）
- 负面：Candle CPU 上无法利用 batch 加速
- 风险：未来切换 GPU provider 时需重新验证 batch 策略

---

*本文档遵循"一句话 + 一句话后果"原则，禁止长篇实现细节。*
