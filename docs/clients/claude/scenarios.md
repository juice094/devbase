# Claude Code x devbase 高频使用场景

> 本文档定义 5 个 Claude Code 与 devbase 集成的核心使用场景。
> 每个场景包含：目标、推荐的 tool 序列、Prompt 模板、预期输出、验收标准。
>
> **版本**: v0.20.0
> **Stable Tools** (5个): `devkit_health`, `devkit_query_repos`, `devkit_vault_search`, `devkit_vault_read`, `devkit_project_context`
> **Beta Tools** (2个): `devkit_project_brief`, `devkit_impact_analysis`

---

## 场景一：项目初始化（Project Onboarding）

**目标**：Claude 首次进入项目目录时，秒级建立全景认知，替代暴力文件扫描。

**Tool 序列**：
1. `devkit_health` — 确认 devbase 服务就绪
2. `devkit_project_brief` — 获取 Markdown 项目简报
3. `devkit_query_repos` — 列出已注册的仓库（如项目含子模块）

**Prompt 模板**：
```
我刚进入这个项目目录。请通过 devbase 获取项目全景：
1. 先检查 devbase 健康状态
2. 生成项目简报（format=markdown）
3. 列出已注册的所有仓库

基于这些信息，告诉我：
- 项目的技术栈和核心模块
- 当前活跃的 Agent Context（如果有）
- 有哪些已知限制（Known Limits）我应该注意
```

**预期收益**：
- Claude 启动扫描时间从数十秒降至 < 2 秒
- 文件读取次数减少 30%+

**验收标准**：
- `devkit_project_brief` 输出包含 ≥ 5 个关键模块
- 无重复读取同一文件 > 1 次

---

## 场景二：代码语义探索（Semantic Code Exploration）

**目标**：用自然语言搜索代码，替代关键词 grep，减少无关文件读取。

**Tool 序列**：
1. `devkit_hybrid_search` — 语义 + 关键词混合搜索
2. `devkit_project_context` — 获取特定文件/符号的上下文
3. `devkit_vault_search` — 搜索 Vault 笔记中的相关设计决策

**Prompt 模板**：
```
我需要理解这个项目的认证流程。请：
1. 用 hybrid_search 搜索 "authentication flow"（limit=10）
2. 对搜索结果中最重要的 3 个文件，用 project_context 获取详细上下文
3. 在 Vault 中搜索是否有相关的设计文档或决策记录

告诉我：
- 认证相关的核心文件有哪些
- 它们之间的调用关系
- Vault 中是否有相关的设计决策或已知限制
```

**预期收益**：
- 搜索结果 Top-3 命中率 ≥ 80%
- 减少 50% 的无关文件读取

**验收标准**：
- `hybrid_search` 返回结果中包含至少 1 个真正的认证相关文件
- `project_context` 输出能支撑代码理解（≥ 3 个相关符号）

---

## 场景三：修改前影响分析（Pre-Edit Impact Analysis）

**目标**：在 Claude 修改代码前，自动分析影响范围，降低回归风险。

**Tool 序列**：
1. `devkit_impact_analysis` — 分析指定修改的影响范围
2. `devkit_query_repos` — 确认目标仓库状态
3. `devkit_vault_read` — 读取相关设计文档（如有）

**Prompt 模板**：
```
我打算修改 src/registry/repo.rs 中的 save_repo 函数，给它增加一个
`force_update` 参数。请：
1. 用 impact_analysis 分析这个修改的影响范围
2. 列出所有调用 save_repo 的位置
3. 检查 Vault 中是否有关于 repo 保存逻辑的决策记录

告诉我：
- 需要同步修改哪些文件
- 哪些测试会受影响
- 是否有已知限制与此相关
```

**预期收益**：
- 修改前预知 ≥ 80% 的受影响文件
- 误报率（false positive）< 20%

**验收标准**：
- `impact_analysis` 返回的 affected_files 包含实际受影响的文件
- 无遗漏关键调用链（false negative = 0）

---

## 场景四：Vault 知识检索（Knowledge Retrieval）

**目标**：在编码过程中快速检索项目笔记、决策记录和已知限制。

**Tool 序列**：
1. `devkit_vault_search` — 搜索 Vault 笔记
2. `devkit_vault_read` — 读取特定笔记全文
3. `devkit_vault_graph` — 查看笔记的双向链接图谱

**Prompt 模板**：
```
我在处理一个关于 Tantivy 索引一致性的 bug。请：
1. 在 Vault 中搜索 "Tantivy consistency"
2. 读取最相关的笔记全文
3. 查看该笔记的反向链接（backlinks）和双向链接图谱

告诉我：
- 之前关于这个问题的决策或分析
- 相关的其他笔记有哪些
- 是否有已知的缓解措施
```

**预期收益**：
- 项目知识查询响应 < 500ms
- 跨笔记关联发现率提升

**验收标准**：
- `vault_search` 返回相关笔记的 Top-3 包含目标笔记
- `vault_graph` 能正确展示双向链接关系

---

## 场景五：会话恢复与上下文延续（Session Recovery）

**目标**：Claude 会话结束后，下次启动时能恢复关键上下文和决策。

**Tool 序列**：
1. `devkit_session_list` — 列出历史会话
2. `devkit_session_recall` — 恢复指定会话的记忆
3. `devkit_session_export` / `devkit_session_import` — 跨工具迁移

**Prompt 模板**：
```
昨天我和 devbase 一起分析了这个项目的架构问题，今天继续。
请：
1. 列出最近 5 个会话
2. 找到昨天关于 "架构拆分" 的会话并恢复其记忆
3. 如果我要切换到 Kimi CLI，请导出当前会话上下文

告诉我：
- 昨天做了哪些分析
- 关键决策或发现是什么
- 导出的文件路径
```

**预期收益**：
- 跨会话上下文不丢失
- 支持 Claude ↔ Kimi 的会话迁移

**验收标准**：
- `session_recall` 能正确注入历史决策到当前上下文
- `session_export` → `session_import` 链路数据完整

---

## 测量与反馈

每个场景执行后，Claude 应记录以下指标到 `mcp-oplog.ndjson`：

```json
{
  "scenario": "project_onboarding",
  "tools_called": ["devkit_health", "devkit_project_brief", "devkit_query_repos"],
  "total_duration_ms": 1250,
  "success": true,
  "file_reads_avoided": 12,
  "user_satisfaction": "high"
}
```

**Dogfooding 周期**：连续 7 天，每天至少执行 3 个场景，记录 tool 调用成功率和响应时间。
