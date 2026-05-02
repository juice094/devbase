# AI 实例上下文交接指南

> 本文档面向**后续接入 devbase 项目的 AI Agent 实例**（Claw / CLI / Web 架构）。
> 目标：在上下文压缩或会话切换后，新 AI 实例能在 60 秒内恢复有效工作上下文。

---

## 一、项目速览（30 秒）

| 属性 | 值 |
|------|-----|
| **名称** | devbase — 本地优先的 AI Skill 编排基础设施 |
| **技术栈** | Rust 2024, SQLite, tokio, ratatui, git2, tantivy, tree-sitter |
| **当前版本** | v0.16.0-dev (`main@93b0860`) |
| **测试** | 450 passed / 0 failed / 5 ignored（`cargo test --workspace --lib`） |
| **编译** | `cargo check --workspace` 0 errors；clippy 0 warnings |
| **仓库** | `https://github.com/juice094/devbase` |

### 快速自检（每次恢复上下文后执行）

```powershell
# 1. 确认编译健康
cd C:\Users\22414\dev\third_party\devbase
cargo check --workspace 2>&1 | Select-Object -Last 3

# 2. 确认测试全绿
cargo test --workspace --lib 2>&1 | Select-String "test result"

# 3. 确认 workspace 成员
cargo metadata --format-version 1 --no-deps 2>$null | ConvertFrom-Json | Select-Object -ExpandProperty workspace_members

# 4. 快速耦合扫描（Windows PowerShell）
Get-ChildItem src\*.rs | ForEach-Object { $count = (Select-String -Path $_.FullName -Pattern "crate::" -NoEmphasis).Count; "$($_.Name): $count refs" } | Sort-Object { [int]($_ -replace '.*: ', '' -replace ' refs', '') } | Select-Object -Last 10
```

---

## 二、Workspace 结构

### 已提取的独立 Crate（13 个）

| Crate | 路径 | 来源 | 测试 | 零耦合 |
|-------|------|------|------|--------|
| `devbase-core-types` | `crates/devbase-core-types` | `src/core/node.rs` | 3 | ✅ |
| `devbase-symbol-links` | `crates/devbase-symbol-links` | `src/symbol_links.rs` | 4 | ✅ |
| `devbase-sync-protocol` | `crates/devbase-sync-protocol` | `src/sync_protocol.rs` | 12 | ✅ |
| `devbase-syncthing-client` | `crates/devbase-syncthing-client` | `src/syncthing_client.rs` | 2 | ✅ |
| `devbase-vault-frontmatter` | `crates/devbase-vault-frontmatter` | `src/vault/frontmatter.rs` | 5 | ✅ |
| `devbase-vault-wikilink` | `crates/devbase-vault-wikilink` | `src/vault/wikilink.rs` | 5 | ✅ |
| `devbase-workflow-interpolate` | `crates/devbase-workflow-interpolate` | `src/workflow/interpolate.rs` | 9 | ✅ |
| `devbase-workflow-model` | `crates/devbase-workflow-model` | `src/workflow/model.rs` | 2 | ✅ |
| `devbase-registry-health` | `crates/devbase-registry-health` | `src/registry/health.rs` | 3 | ✅ |
| `devbase-registry-metrics` | `crates/devbase-registry-metrics` | `src/registry/metrics.rs` | 4 | ✅ |
| `devbase-registry-workspace` | `crates/devbase-registry-workspace` | `src/registry/workspace.rs` | 5 | ✅ |
| `devbase-embedding` | `crates/devbase-embedding` | `src/embedding.rs` | 5 | ✅ |
| `devbase-skill-runtime-types` | `crates/devbase-skill-runtime-types` | `src/skill_runtime/mod.rs` | 7 | ✅ |
| `devbase-skill-runtime-parser` | `crates/devbase-skill-runtime-parser` | `src/skill_runtime/parser.rs` | 3 | ✅ |
| `devbase-registry-entity` | `crates/devbase-registry-entity` | `src/registry/entity.rs` | 3 | ✅ |
| `devbase-registry-relation` | `crates/devbase-registry-relation` | `src/registry/relation.rs` | 1 | ✅ |
| `devbase-registry-call-graph` | `crates/devbase-registry-call-graph` | `src/registry/call_graph.rs` | 0 | ✅ |
| `devbase-registry-dead-code` | `crates/devbase-registry-dead-code` | `src/registry/dead_code.rs` | 0 | ✅ |
| `devbase-registry-code-symbols` | `crates/devbase-registry-code-symbols` | `src/registry/code_symbols.rs` | 0 | ✅ |

### 向后兼容机制

每个已提取模块的原文件保持为 **纯 re-export 桥接**：

```rust
// src/workflow/interpolate.rs — RE-EXPORT ONLY
pub use devbase_workflow_interpolate::*;
```

> **禁止**在这些 re-export 文件中添加任何新代码。如需修改，直接编辑 `crates/*/src/lib.rs`。

---

## 三、关键调试备忘

### Windows Debug Build 栈溢出（已知问题，已缓解）

**症状**：`cargo run` 或 `cargo test` 在 debug profile 下崩溃，`STATUS_STACK_OVERFLOW` (`0xc00000fd`)。

**根因**：clap v4 `Subcommand` derive 宏为大型枚举（变体≥7）生成深层递归展开代码，Windows 默认 1MB 栈空间不足。release build 正常。

**缓解**：`Cargo.toml` 中已配置 `[profile.dev] opt-level = 1`，使 LLVM 内联/优化掉部分栈帧。

**诊断**：
```powershell
# 二分法定位：逐次在 main.rs 中删减 SkillCommands/VaultCommands 变体
# 确认阈值在 6→7 变体之间
cargo run -- <args>  # 若仍溢出，尝试 cargo run --release
```

### Windows Tantivy Flaky（已根治）

**症状**：测试偶发 `PermissionDenied`（Tantivy mmap 文件句柄未释放）。

**根因**：Windows 下 `Index` / `IndexWriter` drop 后 OS 未立即释放文件句柄。

**修复**：测试代码中显式 `drop(index)` + `drop(writer)` + `Start-Sleep -Milliseconds 50`。

### Integration Test 残余失败（预先存在，与提取无关）

```
test_sync_skips_unmanaged_repo    # 预期"没有处理任何仓库"，实际有输出
test_tag_enables_sync             # 同上
```

**处理**：单独运行通过；不影响 crate 提取验收标准。

---

## 四、Crate 提取标准作业程序（SOP）

若任务涉及提取新 workspace crate，按以下流程执行：

### 前置检查

```powershell
# 1. 确认目标模块零耦合（≤3 个 crate:: refs）
Select-String -Path "src/<module_path>.rs" -Pattern "crate::" -NoEmphasis

# 2. 确认无外部 devbase 路径引用
grep "devbase" src/<module_path>.rs  # 应为空（除 re-export 文件）
```

### 提取步骤

1. **创建 crate 目录**：`crates/devbase-<name>/`
2. **编写 `Cargo.toml`**：
   - `version = "0.15.0"`（与现有 workspace crate 一致）
   - `edition = "2024"`
   - 仅声明**直接使用的**外部依赖（serde, regex, anyhow 等）
   - **禁止**引用 `devbase` 主 crate 或任何 `path = "../..."` 的内部路径
3. **迁移实现**：将 `src/<module>.rs` 内容完整复制到 `crates/devbase-<name>/src/lib.rs`
4. **修正导入**：所有 `use crate::` 改为 `use crate::`（lib.rs 内部）或删除未使用的导入
5. **迁移测试**：将 `#[cfg(test)] mod tests { ... }` 一并迁移
6. **注册 workspace**：
   - 根 `Cargo.toml` `[dependencies]` 添加 `devbase-<name> = { path = "crates/devbase-<name>" }`
   - 根 `Cargo.toml` `[workspace] members = ["crates/*"]` 已通配，无需手动添加
7. **创建 re-export 桥接**：原文件替换为 `pub use devbase_<name>::*;`
8. **验证**：
   ```powershell
   cargo check --workspace
   cargo test --workspace --lib
   ```

### 验收标准

- [ ] `cargo check --workspace` 0 errors
- [ ] `cargo test --workspace --lib` 全绿
- [ ] 新 crate 内部 0 个 `crate::` 引用（指向 devbase 主库）
- [ ] 原 `src/` 文件仅含 re-export，无业务逻辑
- [ ] 新 crate 不依赖 `devbase` 主 crate

---

## 五、核心文档导航

| 文档 | 用途 | 何时读取 |
|------|------|---------|
| [`AGENTS.md`](../../AGENTS.md) | 架构红线、安全原则、历史决策 | 每次会话启动 |
| [`docs/ai-protocol.md`](../ai-protocol.md) | 架构快照、待办、耦合地图 | 每次架构变更后 |
| [`docs/ROADMAP.md`](../ROADMAP.md) | 路线图、版本规划、技术债 | 需要了解长期计划时 |
| [`ARCHITECTURE.md`](../../ARCHITECTURE.md) | 系统架构图、模块关系 | 新功能设计时 |
| [`CONTRIBUTING.md`](../../CONTRIBUTING.md) | 开发规范、Schema 迁移指南 | 提交 PR 前 |
| [`Cargo.toml`](../../Cargo.toml) | 依赖、workspace 成员、features | 依赖变更时 |

---

## 六、当前活跃任务（v0.16.0 P2）

**目标**：继续提取剩余 🟢 健康模块为独立 crate。

| 候选模块 | 行数 | 测试 | 内部耦合 | 状态 |
|----------|------|------|----------|------|
| ~~`registry/health`~~ | ~~156~~ | ~~3~~ | ~~0~~ | ~~✅ 已完成~~ |
| ~~`registry/metrics`~~ | ~~153~~ | ~~4~~ | ~~0~~ | ~~✅ 已完成~~ |
| ~~`registry/workspace`~~ | ~~215~~ | ~~5~~ | ~~0~~ | ~~✅ 已完成~~ |
| ~~`workflow/model`~~ | ~~330~~ | ~~2~~ | ~~0~~ | ~~✅ 已完成~~ |
| ~~`embedding`~~ | ~~299~~ | ~~5~~ | ~~0~~ | ~~✅ 已完成~~ |
| ~~`skill_runtime/parser`~~ | ~~417~~ | ~~3~~ | ~~0~~ | ~~✅ 已完成~~ |

> P2 全部候选提取完成。下一步：v0.16.0 P1 MCP trait 化收尾，或 v0.17 `migrate.rs` 拆分。

**阻塞项**：
- ~~`migrate.rs` 拆分~~ → ✅ 已完成（实际 487 行，迁移已拆分至 `migrations/` 29 个独立文件）
- MCP trait 化 → `mcp/tools/repo.rs` 仍有 13 个 `crate::` 引用（从 41 降下，ai-protocol 数据已过时）

---

## 七、环境特定信息（Windows）

- **Shell**：Windows PowerShell（`C:\WINDOWS\System32\WindowsPowerShell\v1.0\powershell.exe`）
- **Rust**：1.94.1
- **工作目录**：`C:\Users\22414\dev\third_party\devbase`
- **Git 用户**：`juice094 <160722440+juice094@users.noreply.github.com>`
- **编译优化**：debug profile 强制 `opt-level = 1`（缓解 Windows clap derive 栈溢出）

---

*本文档应与 `AGENTS.md`、`docs/ai-protocol.md` 一并阅读。如有冲突，以 `AGENTS.md` 为准。*
