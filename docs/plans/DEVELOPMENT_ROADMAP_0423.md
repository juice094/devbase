# 三项目协同开发路线图（2026-04-23）

> 基于 0423 会议室结论，按依赖关系设计串并行推进计划。
> 
> **关键原则**：无依赖的任务并行推进；有依赖的任务按阻塞方优先；外部不可控任务设缓冲期。

---

## 项目总览

```
┌─────────────┐     MCP stdio/SSE      ┌─────────────┐
│   devbase   │ ◄────────────────────► │   clarity   │
│  (Server)   │     Vault 笔记格式     │  (Client)   │
└──────┬──────┘                        └─────────────┘
       │
       │ 数据同步
       ▼
┌─────────────────┐
│ syncthing-rust  │
│  (实体层/友军)   │
└─────────────────┘
```

---

## 一、依赖关系图

```
[devbase tool description audit] ──────┐
                                        │
[devbase 字段契约确认] ✅ ──────────────┼──► [clarity McpToolAdapter 解析]
                                        │
[clarity CI audit 对齐] ◄───────────────┘  (独立，无依赖)

[devbase Vault 格式约定草案] ──────────► [clarity Vault 解析测试]
                                        │
[devbase SSE Daemon W5-W8] ───────────► [clarity-gateway SSE 持久化]
                                        │
[syncthing-rust 格雷反馈 Step B] ──────► [端到端大规模压测]
```

---

## 二、波次规划

### 🔵 波次 1：立即并行（4/23 - 4/30，1 周）

三条路径完全独立，无交叉依赖。

#### 路径 A — devbase（独立）

| 任务 | 优先级 | 预计工时 | 产出 |
|------|--------|---------|------|
| Audit 19 个 tool description 重写 | P0 | 1d | 高质量 description 文档 |
| `devkit_scan` 描述优化 | P0 | 2h | 含 "when to use / when NOT to use" |
| `devkit_sync` 描述优化 | P0 | 2h | 同上 |
| `devkit_health` 描述优化 | P0 | 2h | 同上 |
| `devkit_project_context` 描述优化 | P0 | 2h | 同上 |
| `devkit_query_repos` 描述优化 | P0 | 2h | 同上 |
| 其余 13 个 tool 描述批量优化 | P1 | 4h | 统一模板套用 |
| **质量验证** | — | 2h | `cargo test` + 手动 spot check |

**阻塞方**：无（纯 devbase 内部工作）

#### 路径 B — clarity（独立，字段契约已确认 ✅）

| 任务 | 优先级 | 预计工时 | 产出 |
|------|--------|---------|------|
| `McpToolAdapter` 按 devkit 字段契约实现解析 | P1 | 1d | 支持 `success`/`repo`/`vault_notes`/`assets` 解析 |
| CI audit 步骤对齐 devbase 实践 | P2 | 4h | `.github/workflows/ci.yml` 更新 |
| `McpConfig` 文档更新（env 配置说明） | P2 | 2h | 用户配置指南 |

**阻塞方**：无（devbase 字段契约已在会议室确认）

#### 路径 C — syncthing-rust（被动等待）

| 任务 | 优先级 | 预计工时 | 产出 |
|------|--------|---------|------|
| 等格雷反馈 Step B（幻X离线原因） | P0 | — | 外部不可控 |
| 准备大规模测试数据集 | P1 | 4h | 100MB+ 混合文件集 |
| REST API 文档整理 | P1 | 4h | 供 clarity 侧参考 |

**阻塞方**：格雷（外部，不可控，设 3 天缓冲期）

---

### 🟡 波次 2：依赖解锁后并行（4/30 - 5/15，2 周）

#### 路径 A — devbase（Vault-Skill 同步 + SSE 前期准备）

| 任务 | 优先级 | 前置条件 | 预计工时 | 产出 |
|------|--------|---------|---------|------|
| Vault-Skill 同步原型设计 | P1 | 波次 1 tool audit 完成 | 2d | `devbase skill sync --target clarity` 原型 |
| Vault frontmatter 字段规范文档 | P1 | — | 4h | `docs/vault-format-spec.md` |
| SSE Daemon 前期：trait 设计 | P2 | — | 1d | `McpTool::invoke_stream()` trait 扩展 |
| SSE Daemon 前期：Axum handler | P2 | — | 1d | 流式消息 handler |

**阻塞方**：无（devbase 内部可独立完成设计）

#### 路径 B — clarity（Vault 解析 + 文档）

| 任务 | 优先级 | 前置条件 | 预计工时 | 产出 |
|------|--------|---------|---------|------|
| Vault 笔记解析测试 | P1 | devbase 提供规范文档 | 1d | `devkit_vault_read` 结果解析 |
| Skill 上下文注入原型 | P1 | Vault 解析通过 | 1d | Vault 笔记 -> system prompt 注入 |
| `McpConfig` 中 `tool_tiers` 字段设计 | P1 | — | 4h | 类型定义 + 配置验证 |

**阻塞方**：devbase Vault 格式规范（devbase 侧需 4/30 前产出）

#### 路径 C — syncthing-rust（若格雷反馈到达）

| 任务 | 优先级 | 前置条件 | 预计工时 | 产出 |
|------|--------|---------|---------|------|
| 端到端大规模压测 | P0 | 格雷 Step B 反馈 | 2d | 100MB+ 文件双向同步报告 |
| 幻X设备重新加入验证 | P0 | 格雷 Step B 反馈 | 4h | 三设备拓扑测试 |
| `reference/sketches/` 同步错误修复 | P1 | 幻X上线 | 4h | — |

**阻塞方**：格雷反馈（外部，不可控）

---

### 🔴 波次 3：串行关键路径（5/15 - 6/12，4 周）

这是唯一一条**不能并行**的链。

```
[devbase W5: invoke_stream trait] 
        │
        ▼
[devbase W6: SSE handler 流式适配]
        │
        ▼
[devbase W7-W8: Daemon 内置 SSE Server]
        │
        ▼
[clarity-gateway: SSE 持久化适配]     ← 依赖 devbase daemon 可用
        │
        ▼
[集成测试: devbase daemon + clarity-gateway 长连接]
```

| 周 | devbase 任务 | clarity 任务 | 里程碑 |
|----|-------------|-------------|--------|
| W5 (5/16-5/22) | `invoke_stream()` trait 扩展 | — | devbase 流式接口设计冻结 |
| W6 (5/23-5/29) | SSE handler 流式适配 | — | devbase 分段推送可用 |
| W7-W8 (5/30-6/12) | **Daemon 内置 SSE Server** | clarity-gateway SSE 配置准备 | `devbase daemon` 命令可用 |
| W9+ (6/12 后) | — | **SSE 持久化适配** | 端到端长连接验证 |

**关键风险**：devbase Daemon 延期会直接导致 clarity 侧阻塞。建议 devbase 在 W5 前完成 trait 设计 review，clarity 侧提前准备 SSE Client 代码。

---

## 三、人员/Agent 分工

| 窗口 | 负责项目 | 波次 1 任务 | 波次 2 任务 | 波次 3 任务 |
|------|---------|------------|------------|------------|
| Kimi CLI (本窗口) | **devbase** | tool description audit | Vault-Skill 同步原型 + SSE 前期 | SSE Daemon W5-W8 |
| 另一窗口 | **clarity** | McpToolAdapter + CI audit | Vault 解析 + tool_tiers 字段 | SSE 持久化适配 |
| 另一窗口 | **syncthing-rust** | 等格雷反馈 | 大规模压测（若反馈到达） | — |

---

## 四、关键决策点

| 时间 | 决策 | 影响 | 负责 |
|------|------|------|------|
| 4/30 | tool description audit 是否完成？ | 影响 clarity 侧 tool 选择准确率 | devbase |
| 4/30 | Vault 格式规范文档是否产出？ | 阻塞 clarity Vault 解析测试 | devbase |
| 5/1  | 格雷 Step B 是否反馈？ | 影响 syncthing-rust 波次 2 能否启动 | 格雷（外部）|
| 5/15 | `invoke_stream` trait 是否冻结？ | 影响 SSE 全链路开发节奏 | devbase |
| 6/12 | `devbase daemon` 是否可用？ | 阻塞 clarity-gateway SSE 适配 | devbase |

---

## 五、风险与缓冲

| 风险 | 概率 | 影响 | 缓解措施 |
|------|------|------|---------|
| 格雷反馈延迟 | 中 | syncthing-rust 波次 2 空转 | 提前准备测试数据集，格雷到达后立即执行 |
| devbase Daemon 延期 | 低 | clarity 波次 3 阻塞 | W5 前完成 trait review，预留 1 周缓冲 |
| clarity Vault 解析受阻 | 低 | Vault-Skill 同步延期 | devbase 提前产出规范文档，预留联调时间 |
| tool description 优化效果不及预期 | 中 | AI tool 选择准确率无提升 | A/B 测试：对比优化前后的 tool 调用准确率 |

---

*路线图版本：2026-04-23*
*基于 0423 会议室结论制定*
*下次 review：4/30（波次 1 结束节点）*
