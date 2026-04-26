# 被测试方须知事项

> **版本**: v0.10.0  
> **生效范围**: devbase 运行时使用者、Skill 开发者、代码贡献者  
> **最后更新**: 2026-04-26

---

## 一、运行时审计对象（Skill / AI Agent / Workflow）

devbase v0.10.0 引入 **Hard Veto 运行时守卫**。任何通过 `skill_runtime::executor::run_skill` 执行的 Skill，在启动前会被自动审计。

### 1.1 审计机制

| 阶段 | 行为 | 对执行的影响 |
|:---|:---|:---|
| 执行前 | 查询 `known_limits` 表中 `category='hard-veto'` 且 `mitigated=0` 的记录 | 无阻断 |
| 执行中 | Skill 正常启动，interpreter、timeout、stdin JSON 均不受影响 | 正常执行 |
| 执行后 | 若存在未解决 veto，`stderr` 头部注入 `[HARD-VETO-WARNING]` 前缀 | `status=Success` 不变 |

### 1.2 被测试方义务

- **AI Agent 调用者**：执行后必须检查 `ExecutionResult.stderr`，若包含 `HARD-VETO-WARNING`，需向人类操作员报告
- **Skill 开发者**：Skill 的 `stderr` 输出应预留头部位置，避免与守卫警告冲突
- **Workflow 编排者**：当前守卫仅在 `run_skill` 层级生效，Workflow 步骤间接调用 Skill 时同样触发

### 1.3 审计留痕

每次触发守卫，OpLog 自动写入：

```json
{
  "event_type": "known_limit",
  "status": "warning",
  "details": {
    "action": "skill_guard",
    "skill_id": "<skill-id>",
    "unresolved_vetoes": ["hard-veto-xxx", "hard-veto-yyy"],
    "veto_count": 5
  }
}
```

---

## 二、代码贡献者（CI 被测对象）

### 2.1 提交前检查清单

```bash
# 1. 测试全绿
cargo test --all-targets
# 期望: 288 passed; 0 failed; 3 ignored

# 2. 格式检查
cargo fmt --check

# 3. Clippy 零警告（本地严格模式）
cargo clippy --all-targets -D warnings
```

### 2.2 Schema 变更规范

新增或修改 Registry 表结构时，必须同步更新：

| 文件 | 更新内容 |
|:---|:---|
| `src/registry/migrate.rs` | 新增 `if user_version < N` 迁移块，`CURRENT_SCHEMA_VERSION` 递增 |
| `src/registry/test_helpers.rs` | `SCHEMA_DDL` 常量追加新表 DDL，添加 `test_xxx_table_exists` 测试 |

### 2.3 OpLog 集成规范

新增 Registry CRUD 方法时，必须配套 OpLog 写入：

- `event_type`: 使用 `OplogEventType::KnownLimit` 或新增变体
- `details`: JSON 格式，包含 `action` 和关键字段
- `status`: `"success"` 或 `"warning"`

---

## 三、Hard Veto 清单（当前生效）

以下 5 条 hard veto 在 `devbase limit seed` 时自动填充，**不可删除**，只能 `resolve`（标记为 mitigated）：

| ID 前缀 | 内容 |  severity |
|:---|:---|:---:|
| `hard-veto` | 禁止闭源 / 云端强制 / 数据外泄 | 5 |
| `hard-veto` | 禁止 Docker / RAG(Qdrant) / GUI(Electron) | 5 |
| `hard-veto` | 禁止项目广度 > 5 核心工具 | 5 |
| `hard-veto` | 本地 LLM 优先 | 5 |
| `hard-veto` | Rust 核心模块不可外包给子 Agent | 5 |

> **来源**: `AGENTS.md` Hard Veto 列表  
> **种子命令**: `devbase limit seed`

---

## 四、快速参考

### AI Agent 调用 Skill 后的检查模板

```rust
let result = run_skill(&skill, &args, Duration::from_secs(30))?;
if result.stderr.contains("HARD-VETO-WARNING") {
    // 向人类报告：当前系统存在未解决的 hard veto
    println!("⚠️  Skill executed with unresolved hard vetoes. Please review.");
}
```

### 人类操作员查看当前约束

```bash
devbase limit list --mitigated false
```

---

## 五、变更记录

| 日期 | 版本 | 变更 |
|:---|:---|:---|
| 2026-04-26 | v0.10.0 | 初始文档，覆盖 Hard Veto 运行时守卫 + CI 规范 |
