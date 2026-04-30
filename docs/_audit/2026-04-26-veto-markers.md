# Veto 风险点标记汇总
日期: 2026-04-26
标记格式: `TODO(veto-audit-2026-04-26): [HV/RF-编号] [描述]`
编译状态: ✅ 391 passed / 0 failed / 5 ignored

---

## 🔴 HV-1 数据外泄（2 处标记）

| 文件 | 行 | 标记内容 | 修复优先级 |
|------|-----|---------|-----------|
| `src/knowledge_engine.rs` | 539 | `call_llm` — 将用户 README 发送到外部 LLM API，缺 Ollama 本地支持 | P0 |
| `src/knowledge_engine.rs` | 572 | `try_llm_summary` — `index_repo`/`run_index` 自动调用，非用户显式触发 | P0 |

---

## 🔴 RF-1 全局状态（2 处标记）

| 文件 | 行 | 标记内容 | 修复优先级 |
|------|-----|---------|-----------|
| `src/i18n/mod.rs` | 197 | `static CURRENT: OnceLock<I18n>` — 不在 grandfathered 列表 | P1 |
| `src/i18n/mod.rs` | 210 | `CURRENT.get().expect("i18n not initialized")` — 与全局状态同源 | P1 |

---

## 🟡 RF-6 生产代码 expect()（15 处标记）

### 高风险（需立即修复）

| 文件 | 行 | 标记内容 | 风险说明 |
|------|-----|---------|---------|
| `src/sync/orchestrator.rs` | 73 | `semaphore` acquire expect | shutdown 时 panic |
| `src/sync/orchestrator.rs` | 127 | `semaphore` acquire expect | shutdown 时 panic |
| `src/skill_runtime/publish.rs` | 189 | `git init` expect | 权限/路径失败时 panic |

### 中风险（建议修复）

| 文件 | 行 | 标记内容 | 风险说明 |
|------|-----|---------|---------|
| `src/discovery_engine.rs` | 179 | `keywords_map.get().expect` | Map 内部状态可能不一致 |
| `src/discovery_engine.rs` | 180 | `keywords_map.get().expect` | Map 内部状态可能不一致 |
| `src/skill_runtime/parser.rs` | 144 | `current_input.take().expect` | 解析状态机内部 |
| `src/skill_runtime/parser.rs` | 156 | `current_output.take().expect` | 解析状态机内部 |
| `src/skill_runtime/parser.rs` | 188 | `current_input.take().expect` | 解析状态机内部 |
| `src/skill_runtime/parser.rs` | 193 | `current_output.take().expect` | 解析状态机内部 |
| `src/query.rs` | 23 | `value.chars().next().expect` | 前置检查非类型级保证 |

### 低风险（不变量保证，可保留或渐进修复）

| 文件 | 行 | 标记内容 | 风险说明 |
|------|-----|---------|---------|
| `src/search.rs` | 89 | `schema.get_field("id").expect` | init_index 保证 |
| `src/search.rs` | 91 | `schema.get_field("title").expect` | init_index 保证 |
| `src/search.rs` | 95 | `schema.get_field("content").expect` | init_index 保证 |
| `src/search.rs` | 97 | `schema.get_field("tags").expect` | init_index 保证 |
| `src/search.rs` | 101 | `schema.get_field("doc_type").expect` | init_index 保证 |
| `src/search.rs` | 120 | `schema.get_field("id").expect` | init_index 保证 |
| `src/search.rs` | 164 | `schema.get_field("title").expect` | init_index 保证 |
| `src/search.rs` | 168 | `schema.get_field("content").expect` | init_index 保证 |
| `src/search.rs` | 169 | `schema.get_field("tags").expect` | init_index 保证 |
| `src/search.rs` | 173 | `schema.get_field("doc_type").expect` | init_index 保证 |
| `src/search.rs` | 195 | `schema.get_field("id").expect` | init_index 保证 |
| `src/search/hybrid.rs` | 92 | `lists.into_iter().next().expect` | 前置 len==1 检查 |
| `src/search/hybrid.rs` | 158 | `lists.into_iter().next().expect` | 前置 len==1 检查 |
| `src/workflow/scheduler.rs` | 17 | `in_degree.get_mut().expect` | topo sort 内部不变量 |
| `src/workflow/scheduler.rs` | 35 | `queue.pop_front().expect` | while 条件保证 |
| `src/workflow/scheduler.rs` | 36 | `wf.steps.iter().find().expect` | HashMap 初始化保证 |
| `src/workflow/scheduler.rs` | 42 | `in_degree.get_mut().expect` | topo sort 内部不变量 |
| `src/workflow/interpolate.rs` | 10 | `Regex::new().expect` | 静态正则编译时验证 |
| `src/workflow/interpolate.rs` | 24 | `cap.get(0).expect` | capture group 0 必然存在 |
| `src/workflow/interpolate.rs` | 25 | `cap.get(1).expect` | group 1 在此正则必然存在 |

---

## 🟡 RF-7 路径隐私（2 处标记）

| 文件 | 行 | 标记内容 | 修复优先级 |
|------|-----|---------|-----------|
| `src/mcp/tools/context.rs` | 135 | `project_context` 返回 `path` 字段，可能为绝对路径 | P1 |
| `src/embedding.rs` | 96 | 硬编码 `C:\Users\22414\...` 个人环境路径 | P1 |

---

## 标记统计

| 类别 | 数量 | 文件数 |
|------|------|--------|
| HV-1 数据外泄 | 2 | 1 |
| RF-1 全局状态 | 2 | 1 |
| RF-6 expect (高风险) | 3 | 3 |
| RF-6 expect (中风险) | 7 | 4 |
| RF-6 expect (低风险) | 18 | 5 |
| RF-7 路径隐私 | 2 | 2 |
| **总计** | **34** | **10** |

---

## 批量移除标记脚本（修复完成后执行）

```bash
# 移除所有 veto-audit TODO 注释
cd devbase
sed -i '/TODO(veto-audit-2026-04-26)/d' src/knowledge_engine.rs
sed -i '/TODO(veto-audit-2026-04-26)/d' src/i18n/mod.rs
sed -i '/TODO(veto-audit-2026-04-26)/d' src/sync/orchestrator.rs
sed -i '/TODO(veto-audit-2026-04-26)/d' src/mcp/tools/context.rs
sed -i '/TODO(veto-audit-2026-04-26)/d' src/embedding.rs
sed -i '/TODO(veto-audit-2026-04-26)/d' src/discovery_engine.rs
sed -i '/TODO(veto-audit-2026-04-26)/d' src/query.rs
sed -i '/TODO(veto-audit-2026-04-26)/d' src/search.rs
sed -i '/TODO(veto-audit-2026-04-26)/d' src/search/hybrid.rs
sed -i '/TODO(veto-audit-2026-04-26)/d' src/skill_runtime/parser.rs
sed -i '/TODO(veto-audit-2026-04-26)/d' src/skill_runtime/publish.rs
sed -i '/TODO(veto-audit-2026-04-26)/d' src/workflow/scheduler.rs
sed -i '/TODO(veto-audit-2026-04-26)/d' src/workflow/interpolate.rs
```
