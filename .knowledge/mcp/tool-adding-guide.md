---
type: HowToGuide
title: 添加 MCP Tool 的标准路径
description: 在 devbase 中新增一个 MCP Tool 必须遵循的 6 步流程。
timestamp: 2026-06-25T11:15:50Z
tags: [mcp, tools, howto, onboarding]
---

# 添加 MCP Tool 的标准路径

## 6 步流程

1. **新建模块**
   - 在 `src/mcp/tools/` 新建模块（如 `my_feature.rs`）。

2. **实现 `McpTool` trait**
   ```rust
   pub trait McpTool: Send + Sync + Clone {
       fn name(&self) -> &'static str;
       fn schema(&self) -> serde_json::Value;
       async fn invoke(&self, args: Value, ctx: &mut AppContext) -> Result<Value>;
       async fn invoke_stream(...) -> Result<Vec<ToolStreamEvent>> { ... }
   }
   ```

3. **注册并 `pub use`**
   - 在 `src/mcp/tools/mod.rs` 注册模块并 `pub use`。

4. **加入路由枚举**
   - 在 `src/mcp/mod.rs` 的 `McpToolEnum` 中加入该工具。

5. **添加单元测试**
   - 在 `src/mcp/tests.rs` 或工具模块的 `#[cfg(test)]` 块中添加测试。

6. **更新文档**
   - 更新 `README.md` Tool 矩阵。
   - 更新 `.knowledge/index.md` 的 `mcp_tools` 数字。

## 核心原则

- **所有状态变更操作必须幂等**（`ON CONFLICT ... DO UPDATE/NOTHING`）。
- **批量操作包裹在 SQLite transaction 中**。
- **不得直接调用 `rusqlite::Connection`**，必须通过 `registry` 封装（T11，已知例外除外）。
- **Breaking change 只能新增 tool，不能修改现有 schema**（G4）。

## 稳定性分级

添加新 tool 时选择初始 tier：

| Tier | 含义 | 测试要求 |
|------|------|----------|
| `Stable` | 已长期验证，schema 冻结 | 完整单元测试 + 集成测试 |
| `Beta` | 已验证但可能微调 | 核心路径测试 |
| `Experimental` | 新功能，行为可能变化 | 至少 smoke test |

新 tool 默认从 `Beta` 或 `Experimental` 开始，不要直接标记 `Stable`。
