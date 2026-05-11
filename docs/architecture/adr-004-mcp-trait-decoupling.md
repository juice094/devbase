# ADR-004: MCP Tool Layer Trait Decoupling

- **状态**: accepted
- **日期**: 2026-05-11
- **作者**: devbase 架构优化会话

## 上下文

`src/mcp/tools/` 中的 MCP 工具实现直接内联调用 `crate::health::`、`crate::search::`、`crate::registry::` 等底层模块，导致：
- 工具层与业务层硬耦合，无法独立测试
- `repo.rs` 等文件 `crate::` 内联引用超过 10 处，违反架构红线
- 新增工具时容易引入隐式依赖

## 决策

为每个业务领域定义 trait（`ScanClient`、`HealthClient`、`RegistryClient`、`KnowledgeClient`、`SearchClient`、`RepoAnalyzer` 等），由 `AppContext` 统一实现，MCP 工具只依赖 trait。

## 后果

- **正面**: `repo.rs` `crate::` 引用从 11 降至 8（全部集中在 use 语句）；工具层可独立单元测试；新增领域只需扩展 trait
- **负面**: trait 定义与实现分属不同文件，跳转成本略增；简单查询也需 trait 封装
- **风险**: 过度抽象可能导致 trait 膨胀；需定期审查 trait 方法是否仍被使用

## 备选方案

| 方案 | 不选原因 |
|------|---------|
| 保持现状，仅清理 use 语句 | 未解决测试隔离问题 |
| 每个工具独立 service struct | 与现有 `AppContext` 模式冲突，引入更多类型 |

## 相关决策

- 依赖：ADR-001（单 crate 模型使 trait 定义零成本）
- 被依赖：ADR-005（AppContext Clone 是 trait 在 spawn_blocking 中使用的前提）
