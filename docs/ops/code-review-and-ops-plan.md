# 代码审查与运维工作规划

> **版本**: v0.10.0  
> **生成日期**: 2026-04-26  
> **理论依据**: SRE (Google), DevOps Handbook, Rust API Guidelines, C4 Model  
> **状态**: 草案，待人类确认后执行

---

## 一、当前基线数据

基于自动化审计（`src/` 共 93 文件 / 28,374 行）：

| 维度 | 指标 | 风险等级 |
|:---|:---|:---:|
| 测试覆盖 | 54/93 文件有 `#[cfg(test)]` (58%) | 🟡 中 |
| unwrap/expect/panic | ~474 处（生产代码） | 🔴 高 |
| unsafe 块 | 7 处（生产代码） | 🟡 中 |
| TODO/FIXME | 1 处 | 🟢 低 |
| Schema 迁移 | v1→v19，自动备份 | 🟢 低 |
| CI 状态 | 全绿，但配置有隐患 | 🟡 中 |
| 依赖数量 | 30 prod + 2 dev | 🟢 低 |
| 单文件体量 | `main.rs` 1,483 行 | 🟡 中 |

**高风险模块**（unwrap 密度 Top 5）：
1. `scan.rs` — 39 处
2. `skill_runtime/registry.rs` — 29 处
3. `skill_runtime/publish.rs` — 28 处
4. `workflow/executor.rs` — 26 处
5. `skill_runtime/clarity_sync.rs` — 25 处

---

## 二、Phase 1：代码审查硬化（4 周）

### 2.1 unwrap/expect 清零运动

**理论**: Google SRE — "Fail Fast, but Fail Gracefully." unwrap 在 CLI 工具中会造成用户可见的 panic，破坏信任。

**目标**: 474 → 200 处（降低 58%），高优先级模块清零。

| 优先级 | 模块 | 策略 |
|:---|:---|:---|
| P0 | `scan.rs` | `?` 传播 + `anyhow::Context` |
| P0 | `workflow/executor.rs` | `StepResult::Failed` 替代 panic |
| P1 | `skill_runtime/registry.rs` | `Result` 链式返回 |
| P1 | `skill_runtime/publish.rs` | git2 错误映射 |
| P2 | `knowledge_engine.rs` | IO 错误降级为 `None` |

**验收**: `cargo clippy --all-targets -D warnings -W clippy::unwrap_used` 对目标模块零报警。

### 2.2 unsafe 代码审计与文档化

**当前 7 处 unsafe**：
- `workflow/interpolate.rs` × 3: `std::env::set_var` / `remove_var`（Rust 2024 已弃用，需 unsafe）
- `main.rs` × 1
- `search.rs` × 3

**行动**:
1. 每处 unsafe 块前添加 `// SAFETY:` 注释，说明为何此处安全
2. 评估 `set_var` 是否可以移至程序启动时（当前已在 `main.rs` 中），减少 unsafe 表面
3. `search.rs` 的 unsafe 若为 FFI，需添加 `#[deny(unsafe_code)]` 模块级限制

### 2.3 测试覆盖率攻坚

**目标**: 58% → 75% 文件有测试。

**无测试文件 Top 10**（按行数排序）:
- `main.rs` (1,483) — CLI 集成测试
- `knowledge_engine.rs` (927) — 已有部分测试，补全边缘 case
- `semantic_index.rs` (920) — AST 提取测试
- `query.rs` (692) — 查询求值测试
- `dependency_graph.rs` (735) — 多语言解析测试

**策略**: 对 >500 行且无测试的文件，每 Wave 至少补充 1 个 smoke test。

---

## 三、Phase 2：运维体系构建（4 周）

### 3.1 CI/CD 优化

**当前问题清单**:

| 问题 | 理论依据 | 修复方案 |
|:---|:---|:---|
| 无缓存 | DORA — "Fast feedback loop" | 添加 `Swatinem/rust-cache@v2` |
| `--test-threads=1` | 并行计算浪费 | 恢复默认多线程（除非有共享状态冲突） |
| Defender 禁用需提权 | 最小权限原则 | 移除此步骤，改用排除目录 `Add-MpPreference -ExclusionPath` |
| 重复配置 | DRY | 提取 composite action 或共享步骤 |
| clippy `-W warnings` |  brittle 配置 | 改为 `-D warnings` 与本地一致，或固定 toolchain 版本 |
| 缺少 `cargo audit` | 供应链安全 | 新增 security job |

**目标 CI 流水线**:

```yaml
jobs:
  check:    # cargo check + fmt + clippy (cached, ~2min)
  test:     # cargo test --all-targets (parallel, ~5min)
  security: # cargo audit + cargo deny (nightly)
  docs:     # mdbook build + link check
```

### 3.2 可观测性增强

**当前状态**: 使用 `tracing` crate，但无结构化输出或外部收集。

**目标**: 
1. **结构化日志**: 所有 OpLog 写入同时输出 JSON 到 `~/.local/share/devbase/logs/`（按日轮转）
2. **关键指标**:
   - `devbase_skill_execution_total`（按 skill_id, status 标签）
   - `devbase_known_limit_veto_hits_total`
   - `devbase_registry_query_duration_ms`
3. **健康端点**: `devbase health --json` 输出机器可读状态（供外部监控调用）

### 3.3 数据治理

| 项 | 当前 | 目标 |
|:---|:---|:---|
| Registry 备份 | 迁移前自动备份 | 定期压缩 + 保留最近 10 份 |
| 日志保留 | 无限制 | 30 天轮转 |
| SQLite 优化 | 无 | `VACUUM` 月度任务 + WAL 模式评估 |
| 敏感数据 | Token 在 `config.toml` | 加密 at-rest（Windows DPAPI / macOS Keychain） |

---

## 四、Phase 3：架构健康度（持续）

### 4.1 模块拆分（main.rs 减重）

**当前**: `main.rs` 1,483 行，包含所有 CLI 命令匹配逻辑。

**目标**: 提取 `src/cli/commands.rs`，每个 subcommand 一个函数，main 只做 dispatch。

### 4.2 依赖更新策略

| 策略 | 频率 | 工具 |
|:---|:---|:---|
| 安全补丁 | 即时 | `cargo audit` CI job + Dependabot |
| Minor 更新 | 每月 | `cargo update` + 全量测试 |
| Major 升级 | 每季度 | 评估 breaking change 影响 |
| 淘汰评估 | 每半年 | 检查未使用依赖 (`cargo-udeps`) |

### 4.3 性能基准

建立 3 个基准测试：
1. **Registry 查询**: `list_known_limits` 10,000 条记录耗时
2. **Skill 执行**: 空 skill（`echo hello`）端到端耗时
3. **Tantivy 索引**: 1000 个 vault notes 索引耗时

使用 `criterion` 或 `iai-callgrind` 追踪回归。

---

## 五、执行日历（建议）

| 周 | 主题 | 交付物 |
|:---|:---|:---|
| W1 | unwrap 清零 P0 | `scan.rs` / `workflow/executor.rs` 零 unwrap |
| W2 | CI 优化 + unsafe 审计 | 缓存生效 + SAFETY 注释全覆盖 |
| W3 | 测试攻坚 + cargo audit | +5 个 smoke test + security job |
| W4 | 可观测性 MVP | 结构化日志 + 3 个指标 |
| W5 | main.rs 拆分 | `src/cli/commands.rs` 提取 |
| W6 | 数据治理 | 备份策略 + WAL 评估 |
| W7 | 性能基准 | 3 个 criterion benchmark |
| W8 | 回顾 + 文档 | 运维手册 v1.0 |

---

## 六、验收标准

| 指标 | 基线 | 目标 |
|:---|:---|:---|
| 生产 unwrap/expect | ~474 | <200 |
| 文件测试覆盖率 | 58% | 75% |
| CI 平均耗时 | ~5min | <3min |
| unsafe 注释率 | 0% | 100% |
| cargo audit 报警 | 未知 | 0 high/critical |
| 日志结构化率 | 0% | 100% (OpLog) |

---

## 七、Hard Veto 约束

本规划受以下 hard veto 约束：
- 不引入 Docker / 云端服务
- 不拆分 crate（当前单 crate 仍最优）
- 本地 LLM 优先（可观测性不绑定外部 APM）
