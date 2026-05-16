# 生产就绪检查清单 (Production Readiness Checklist)

> 本清单定义 devbase 从"实验项目"升级为"Agent 可依赖基础设施"的门槛。
> 所有条目必须客观可验证，禁止主观描述。

---

## 一、稳定性门槛 (Stability)

| 编号 | 检查项 | 验收标准 | 当前状态 | 阻塞 Issue |
|------|--------|---------|---------|-----------|
| S-1 | 测试通过率 | `cargo test --all-targets` 连续 7 天 0 failed | ⬜ | |
| S-2 | 编译零警告 | `cargo clippy --all-targets -D warnings` 全绿 | ✅ | |
| S-3 | 无已知崩溃 | Issue 列表中无 `panic`/`unwrap` 导致的 crash | ⬜ | |
| S-4 | Schema 冻结 | stable tools 的输入/输出 schema 30 天无变更 | ⬜ | |
| S-5 | 回归测试 | 每次 PR 必须通过 integration tests (`tests/cli.rs`) | ✅ | |

## 二、性能门槛 (Performance)

| 编号 | 检查项 | 验收标准 | 当前状态 | 备注 |
|------|--------|---------|---------|------|
| P-1 | MCP Server 冷启动 | `devbase mcp` 从执行到 ready < 1s | ⬜ | 需测量 |
| P-2 | 内存占用 | MCP Server 常驻内存 < 128MB | ⬜ | 需测量 |
| P-3 | 查询延迟 | `devkit_project_context` 平均响应 < 500ms | ⬜ | 需基准测试 |
| P-4 | Binary 大小 | release binary < 50MB | ⬜ | 需编译后测量 |
| P-5 | 并发安全 | 同时运行 `devbase tui` + `devbase mcp` 无锁竞争 | ⬜ | 需压测 |

## 三、MCP 门槛 (MCP Integration)

| 编号 | 检查项 | 验收标准 | 当前状态 |
|------|--------|---------|---------|
| M-1 | Stable tools 固化 | 5 个 stable tools 签名冻结，文档完整 | ⬜ |
| M-2 | 错误处理 | 所有 tools 返回结构化错误（非 panic） | ⬜ |
| M-3 | Schema 版本化 | tools 声明 `schemaVersion`，支持向后兼容 | ⬜ |
| M-4 | Graceful 降级 | MCP Server 异常退出时，Agent 能继续工作 | ⬜ |
| M-5 | 健康检查 | `devkit_health` 能在 100ms 内返回状态 | ⬜ |

## 四、Agent 集成门槛 (Agent Integration)

| 编号 | 检查项 | 验收标准 | 当前状态 |
|------|--------|---------|---------|
| A-1 | Kimi CLI 验证 | 连续 1 周 daily use，tool 调用成功率 > 95% | ⬜ |
| A-2 | Claude Code 验证 | 连续 1 周 daily use，无 `api.anthropic.com` 干扰 | ⬜ |
| A-3 | Session 持久化 | `devkit_session_export` → 切换工具 → `devkit_session_import` 上下文不丢失 | ⬜ |
| A-4 | Project Brief 质量 | `devkit_project_brief` 输出能被 Agent 直接用于决策 | ⬜ |
| A-5 | Vault 检索质量 | NLQ 自然语言查询 Top-3 命中率 > 80% | ⬜ |

## 五、文档门槛 (Documentation)

| 编号 | 检查项 | 验收标准 | 当前状态 |
|------|--------|---------|---------|
| D-1 | AGENTS.md 精简 | < 300 行，Agent 读取成本可控 | ✅ |
| D-2 | 工具文档 | 每个 stable tool 有独立的 `.md` 文档 | ⬜ |
| D-3 | 架构决策记录 | ADR 覆盖所有重大设计选择 | ⬜ |
| D-4 | 故障排查指南 | 常见错误有 Runbook | ⬜ |

## 六、发布流程 (Release Process)

| 编号 | 检查项 | 验收标准 | 当前状态 |
|------|--------|---------|---------|
| R-1 | 版本号语义 | 遵循 SemVer，v1.0 为生产就绪标志 | ⬜ |
| R-2 | 变更日志 | CHANGELOG.md 记录所有 breaking changes | ✅ |
| R-3 | 二进制分发 | GitHub Release 提供 Windows/Linux/macOS binary | ⬜ |
| R-4 | 回滚方案 | 新版本导致 regression 时，5 分钟内回退到旧版本 | ⬜ |

---

## Phase 推进计划

```
Phase 0: 当前 (v0.20.x)
  └─ 完成 S-2, S-5, D-1, R-2
  └─ Focus: 精简文档 + 编译优化 + 崩溃修复

Phase 1: 稳定化 (v0.30.x)
  └─ 达成: S-1, S-3, S-4, P-1~P-5, M-1~M-5
  └─ Focus: 性能基准测试 + MCP schema 冻结

Phase 2: Agent 试点 (v0.40.x)
  └─ 达成: A-1, A-2, A-4, A-5
  └─ Focus: 单 Agent 接入 + dogfooding

Phase 3: 生产就绪 (v1.0.0)
  └─ 达成: A-3, D-2~D-4, R-3, R-4
  └─ Focus: 多 Agent 共享 + 二进制分发 + 故障恢复
```

---

*本清单随项目演进更新。每次修改需经作者审核。*
