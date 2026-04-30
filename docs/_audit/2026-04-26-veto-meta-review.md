# Hard Veto 元审计报告：规则本身是否仍适用
日期: 2026-04-26
范围: 工作区 AGENTS.md (V3.1-EP) + devbase AGENTS.md (RF-1~RF-6)
目的: 审查 Veto/红线规则是否仍符合 v0.13.0 项目定位，识别过时/缺失/冲突项

---

## 一、工作区 Hard Veto 逐条审视

### HV-1: 禁止闭源 / 云端强制 / 数据外泄

**原始表述**: "禁止闭源 / 云端强制 / 数据外泄"

**当前项目状态**:
- devbase = "Local Context Compiler"，核心定位是本地感知→编码→持久化→检索
- 所有数据存储在 `%LOCALAPPDATA%/devbase/`，不进入版本控制
- 存在 3 个网络出口: arXiv API、GitHub API、LLM API（`knowledge_engine.rs`）

**适用性判定**: ✅ **仍然适用，且应强化**

**问题**:
- Veto 未明确区分"用户主动触发的网络调用"（arXiv/GitHub tools）与"系统自动触发的网络调用"（`index_repo` → `try_llm_summary`）
- `knowledge_engine.rs` 的 LLM 自动上传 README 触碰了此 Veto 的精神，但规则文本没有覆盖这种"配置后自动触发"的场景

**建议修订**:
```
禁止闭源 / 云端强制 / 数据外泄。
例外仅允许: (1) 用户显式触发的 tool/command 调用；
(2) 配置后自动触发必须经过用户明确授权（opt-in）。
```

---

### HV-2: 禁止 Docker / RAG(Qdrant) / GUI(Electron)

**原始表述**: "禁止 Docker / RAG(Qdrant) / GUI(Electron)"

**当前项目状态**:
- SQLite + Tantivy，无 Qdrant
- ratatui TUI，无 Electron
- 无容器化

**适用性判定**: ✅ **完全适用**

**问题**: 无

---

### HV-3: 禁止项目广度 > 5 核心工具

**原始表述**: "禁止项目广度 > 5 核心工具"

**当前项目状态**:
- 38 MCP tools / 22 top-level modules / 7 核心子系统
- devbase AGENTS.md 内部规则: "拆分 crate（50+ tools 后再评估）"

**适用性判定**: 🟡 **表述不清，与项目实际冲突**

**问题**:
1. "核心工具"定义模糊: 是 MCP tool 数量？top-level module 数量？还是外部依赖数量？
2. 若按"核心子系统"算（Registry/Search/MCP/TUI/Workflow/Skill Runtime/Semantic Index = 7），devbase 已违规
3. 若按"MCP tools"算（38），远超 5
4. devbase 内部规则（50+ tools 再评估）与工作区 Veto（>5）存在数量级冲突

**建议修订**:
```
禁止无节制扩张。核心外部依赖 ≤ 5（SQLite/Tantivy/git2/reqwest/tokio 等）。
功能模块（MCP tools / 子系统）不受此限，但新增核心子系统需经架构评审。
```

---

### HV-4: 本地 LLM 优先

**原始表述**: "本地 LLM 优先"

**当前项目状态**:
- devbase 本身不做 LLM inference
- `knowledge_engine.rs` 默认提供商列表: DeepSeek/Kimi/OpenAI/DashScope（全部云端）
- `config.toml` 默认 `llm.enabled = false`，但 provider 默认 `ollama`（本地）
- `generate_query_embedding` 依赖外部 Python（非本地 Rust）

**适用性判定**: 🟡 **适用但执行不到位**

**问题**:
- `knowledge_engine.rs` 的 LLM 提供商列表以云端为主，本地 Ollama 虽有配置但不在核心代码的 `try_llm_summary` match 分支中
- v0.14 的 candle 本地 embedding 尚未落地

**建议修订**:
```
本地 LLM 优先。任何外部 LLM API 调用必须:
(1) 默认关闭；
(2) 本地替代方案（Ollama/candle/llama.cpp）优先评估；
(3) 云端仅作为用户显式配置后的 fallback。
```

---

### HV-5: Rust 核心模块不可外包给子 Agent

**原始表述**: "Rust 核心模块不可外包给子 Agent"

**当前项目状态**:
- 审计阶段使用 3 个 explore subagent（只读分析）
- 所有代码修改由 root agent 直接执行

**适用性判定**: ✅ **适用**

**问题**: 无

**建议细化**:
```
Rust 核心模块的修改（包括新增/删除/重构）必须由 root agent 直接执行。
Subagent 仅允许只读分析（explore/plan），禁止写入代码。
```

---

## 二、devbase 架构红线 (RF-XX) 逐条审视

### RF-1: 依赖注入优于全局状态

**原始表述**: "禁止新增 `dirs::data_local_dir()` / `std::env::var_os` 硬编码路径。所有 IO 边界路径必须通过参数、构造函数或 trait 注入。例外：现有 3 处 grandfathered。"

**当前项目状态**:
- `AppContext` God Object（被 22+ 模块依赖）
- `WorkspaceRegistry` God Object（46+ 文件依赖）
- `i18n/mod.rs` 全局 `CURRENT: OnceLock<I18n>`（不在 grandfathered 列表中）

**适用性判定**: ✅ **适用，但 grandfathered 列表未更新**

**问题**:
- grandfathered 列表仍为 3 处（`backup_dir`, `db_path`, `index_path`），但 `i18n` 全局状态未被 grandfathered
- God Object 是全局状态的变体（通过单例模式传递）

**建议修订**:
```
 grandfathered 列表扩展至 4 处，增加 `i18n::CURRENT`（待 v0.15 注入式重构后移除）。
 God Object（WorkspaceRegistry/AppContext）纳入 RF-1 约束范围，v0.15 拆分前禁止新增依赖。
```

---

### RF-2: 测试密封性

**原始表述**: "所有测试禁止修改全局进程状态。文件系统测试必须使用 tempfile + 注入式路径。"

**当前项目状态**:
- `tempfile` + `DEVBASE_DATA_DIR` + `SEARCH_TEST_LOCK` 已到位
- 391 tests 全绿

**适用性判定**: ✅ **适用，执行良好**

---

### RF-3: Schema 单一事实来源

**原始表述**: "`SCHEMA_DDL` 与 `migrate.rs` 必须原子同步。"

**当前项目状态**:
- v25 的 `agent_symbol_reads` 在两者中存在
- 但 `migrate.rs` 1,214 行包含 25 个版本，维护成本极高

**适用性判定**: ✅ **适用，但执行机制不足**

**问题**:
- 当前仅靠人工审查保证同步，无 CI 自动校验
- `init_db_at` 的 1,214 行巨石使同步更容易出错

**建议强化**:
```
CI 增加 schema 一致性检查: 比较 `SCHEMA_DDL` 与 `migrate.rs` 最新版本的表结构。
init_db_at 超过 1000 行即触发重构警告（与 RF-4 对称）。
```

---

### RF-4: 二进制入口限界

**原始表述**: "`main.rs` 行数不得超过 1000 行。"

**当前项目状态**:
- `main.rs` 515 行

**适用性判定**: ✅ **适用，执行良好**

**建议扩展**:
```
新增: `init_db_at` 不得超过 1000 行（当前 1,214 行，已触发）。
```

---

### RF-5: 无循环依赖

**原始表述**: "禁止模块间双向 `use` 引用。"

**当前项目状态**:
- 无编译期 use 循环
- 存在逻辑循环: `storage` ↔ `registry` 初始化交叉

**适用性判定**: ✅ **适用，但需区分编译期与逻辑期**

**建议修订**:
```
编译期循环: 绝对禁止（现有 fitness function 覆盖）。
逻辑循环（初始化时交叉调用）: P2 技术债，纳入架构审计但不阻断编译。
```

---

### RF-6: 生产代码无 panic

**原始表述**: "生产代码禁止 `unwrap()`、`expect()`、`panic!()`。"

**当前项目状态**:
- `unwrap()`: 生产代码中 0 处（Wave 30 完成）
- `expect()`: 生产代码中 17 处（见 `2026-04-26-hard-veto-review.md`）
- `panic!()`: 生产代码中 0 处

**适用性判定**: 🟡 **适用，但 Wave 30 执行不完整**

**问题**:
- Wave 30 声称"生产代码 unwrap 清零"，但 `expect()` 未被清理
- 部分 `expect()` 是合理的（如静态正则编译、schema 字段 lookup），但规则无例外条款
- `sync/orchestrator.rs` 的 semaphore expect 在 shutdown 场景会 panic，属于真实风险

**建议修订**:
```
生产代码禁止 `unwrap()` 和 `panic!()`。
`expect()` 允许在满足以下条件时使用:
(1) 不变量由同一函数的先前逻辑严格保证（如 schema 初始化后立即 lookup）；
(2) 注释说明不变量来源；
(3) 不涉及并发资源（semaphore、channel 等）的 expect 必须改为 Result 传播。
```

---

## 三、缺失的 Veto/红线（迭代中暴露的新风险）

### 新增候选 1: 路径隐私红线

**动机**: `project_context` 返回 repo 绝对路径，`knowledge_engine` 读取 README，`embedding.rs` 硬编码个人路径。

**建议规则**:
```
任何向外部（MCP 响应、错误消息、日志）输出的路径必须脱敏:
- `dirs::home_dir()` 前缀替换为 `~`
- Windows 盘符绝对路径中的用户名部分替换为 `<user>`
```

### 新增候选 2: Feature 隔离红线

**动机**: v0.14 引入 `local-embedding` feature，涉及 candle/tokenizers/hf-hub 等重型依赖。

**建议规则**:
```
新增可选 feature 必须满足:
(1) 默认关闭不影响现有功能；
(2) `--no-default-features` 编译通过；
(3) feature 开启后的 binary 增量 < 10 MB（超出需拆分评估）。
```

### 新增候选 3: 网络出口白名单

**动机**: `reqwest` 有 3 个使用点（arXiv/GitHub/LLM），未来可能新增。

**建议规则**:
```
新增网络出口（HTTP/API 调用）必须:
(1) 在 AGENTS.md 中登记；
(2) 提供离线降级路径；
(3) 不得传输用户代码内容（仅元数据）。
```

---

## 四、规则间冲突分析

| 冲突对 | 表现 | 裁决 |
|--------|------|------|
| HV-3 (>5 核心工具) vs 项目实际 (7 子系统/38 tools) | 项目已超限制 | **HV-3 表述过时**，应放宽为"核心外部依赖 ≤ 5" |
| HV-3 vs devbase 内部规则 (50+ tools 再评估) | 数量级冲突 | **以 devbase 内部规则为准**（项目级决策优先于工作区通用约束） |
| HV-4 (本地 LLM 优先) vs `knowledge_engine.rs` (默认云端提供商) | 代码行为与 Veto 冲突 | **代码需修正**，非 Veto 问题 |
| RF-6 (无 expect) vs 静态正则 expect | 技术上合理的 expect | **RF-6 需增加例外条款** |
| RF-1 (无全局状态) vs `i18n::CURRENT` |  grandfathered 列表未更新 | **更新 grandfathered 列表**，v0.15 移除 |

---

## 五、修订建议汇总

### 立即修订（本周）

| 规则 | 修订内容 |
|------|---------|
| HV-1 | 增加"配置后自动触发需 opt-in"条款 |
| HV-3 | "核心工具"改为"核心外部依赖"，明确功能模块不受限 |
| RF-1 | grandfathered 列表增加 `i18n::CURRENT`，WorkspaceRegistry/AppContext 纳入约束 |
| RF-6 | 增加 `expect()` 例外条款（不变量 + 注释 + 无并发资源） |

### 中期增补（v0.14-v0.15）

| 新增规则 | 内容 |
|----------|------|
| RF-7 路径隐私 | 外部输出路径必须脱敏 |
| RF-8 Feature 隔离 | 新增 feature 的编译/binary 约束 |
| RF-9 网络白名单 | 新增网络出口需登记 + 离线降级 |

### 废止/放宽

| 规则 | 理由 |
|------|------|
| HV-3 "项目广度 > 5"原表述 | devbase 已 7 子系统/38 tools，历史形成且可控；应改为约束"核心外部依赖" |

---

## 六、裁决

**需要人类确认的事项**:

1. **HV-3 放宽**: 是否同意将 "项目广度 > 5 核心工具" 改为 "核心外部依赖 > 5"？
2. **RF-6 例外**: 是否同意为 "不变量 guaranteed expect" 增加例外条款？
3. **新增 RF-7/8/9**: 是否采纳路径隐私、Feature 隔离、网络白名单三条新红线？
4. ** grandfathered 更新**: 是否同意将 `i18n::CURRENT` 纳入 grandfathered 列表，目标 v0.15 移除？

**无需确认、可直接执行**:
- HV-1 增加 opt-in 条款
- `knowledge_engine.rs` 默认关闭 LLM + 增加 Ollama 本地支持
- `i18n` 注入式重构排期 v0.15
