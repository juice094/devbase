# MCP Tools 参考

devbase MCP Server 提供 **68 个 tools**，通过 stdio 传输与 AI Agent 通信。工具按稳定性分为三级：

- **Stable** — 经过充分测试，schema 冻结。详见 [`stable-tools/`](stable-tools/README.md) 独立文档。
- **Beta** — 功能验证通过，schema 可能微调
- **Experimental** — 新功能，行为可能变化

---

## 仓库管理（5）

| 工具名 | Tier | 一句话描述 | 关键参数 |
|--------|------|-----------|----------|
| `devkit_scan` | Beta | 扫描目录发现 Git 仓库并注册 | `path`, `register` |
| [`devkit_health`](stable-tools/health.md) | Stable | 检查注册仓库的健康状态（dirty/behind/ahead） | `detail`, `limit`, `page` |
| `devkit_sync` | Beta | 安全同步仓库与上游（destructive gate） | `repo_id`, `dry_run` |
| `devkit_query_repos` | Stable | 查询已注册仓库列表，支持 tag/language 过滤 | `query`, `limit`, `page` |
| `devkit_index` | Beta | 索引仓库摘要、模块结构、代码符号 | `path` |

## 代码分析（6）

| 工具名 | Tier | 一句话描述 | 关键参数 |
|--------|------|-----------|----------|
| `devkit_code_metrics` | Beta | 统计代码行数、语言分布、测试覆盖率 | `repo_id` |
| `devkit_module_graph` | Beta | 获取仓库模块依赖图 | `repo_id` |
| `devkit_code_symbols` | Beta | 列出仓库中的代码符号（函数/结构体/枚举等） | `repo_id`, `file_path`, `symbol_type` |
| `devkit_dependency_graph` | Beta | 获取跨仓库依赖关系图 | `repo_id` |
| `devkit_call_graph` | Beta | 获取函数调用图 | `repo_id`, `symbol_name` |
| `devkit_dead_code` | Beta | 检测未被调用的私有函数 | `repo_id`, `include_pub` |

## 知识检索（8）

| 工具名 | Tier | 一句话描述 | 关键参数 |
|--------|------|-----------|----------|
| `devkit_semantic_search` | Beta | 基于 embedding 的语义代码搜索 | `repo_id`, `query`, `limit` |
| `devkit_hybrid_search` | Beta | 向量语义 + 关键词 RRF 混合搜索 | `repo_id`, `query`, `limit` |
| `devkit_cross_repo_search` | Beta | 跨仓库符号搜索（按 tag 过滤） | `tags`, `query`, `limit` |
| `devkit_related_symbols` | Experimental | 查找与指定符号相关的符号 | `repo_id`, `symbol_name` |
| `devkit_embedding_store` | Beta | 存储代码符号的 embedding 向量 | `repo_id`, `symbol_name`, `embedding` |
| `devkit_embedding_search` | Beta | 基于 embedding 的相似度搜索 | `repo_id`, `embedding`, `limit` |
| `devkit_natural_language_query` | Beta | 自然语言查询（NLQ） | `query`, `limit` |
| `devkit_knowledge_report` | Beta | 生成工作区知识覆盖报告 | `repo_id`, `activity_limit` |

## Vault 笔记（8）

| 工具名 | Tier | 一句话描述 | 关键参数 |
|--------|------|-----------|----------|
| [`devkit_vault_search`](stable-tools/vault_search.md) | Stable | 关键词搜索 Vault 笔记 | `query` |
| `devkit_vault_read` | Stable | 读取指定 Vault 笔记的完整内容 | `path` |
| `devkit_vault_write` | Beta | 写入或更新 Vault 笔记（destructive gate） | `path`, `content`, `frontmatter` |
| `devkit_vault_backlinks` | Beta | 查找指向指定笔记的反向链接 | `note_id` |
| `devkit_vault_daily` | Beta | 按日期列出 Vault 每日笔记 | `date`, `limit` |
| `devkit_vault_graph` | Beta | 获取 Vault 笔记链接图 | `repo_id`, `note_id`, `depth` |
| `devkit_vault_export` | Beta | 导出 Vault 笔记集合 | `query`, `format` |
| `devkit_vault_history` | Beta | 获取 Vault 笔记修改历史 | `path`, `limit` |

## Skill 运行时（4）

| 工具名 | Tier | 一句话描述 | 关键参数 |
|--------|------|-----------|----------|
| `devkit_skill_list` | Beta | 列出已安装的 Skill | `limit`, `tag` |
| `devkit_skill_search` | Beta | 语义搜索 Skill | `query`, `limit` |
| `devkit_skill_run` | Beta | 执行指定 Skill（destructive gate） | `skill_id`, `args` |
| `devkit_skill_discover` | Beta | 将当前项目封装为 Skill（destructive gate，dry_run 默认 true） | `path` |

## 项目上下文（3）

| 工具名 | Tier | 一句话描述 | 关键参数 |
|--------|------|-----------|----------|
| `devkit_project_context` | Stable | 获取项目统一上下文（repo + vault + assets + modules + symbols + calls） | `project` |
| `devkit_project_brief` | Beta | 生成 Markdown 项目摘要（架构 + 活动 + 限制），供 LLM 注入 | `repo_id`, `max_tokens` |
| `devkit_impact_analysis` | Beta | 分析代码变更影响范围 | `repo_id`, `file_path` |

## Session 管理（13）

| 工具名 | Tier | 一句话描述 | 关键参数 |
|--------|------|-----------|----------|
| `devkit_session_save` | Beta | 保存当前会话上下文 | `name`, `tags` |
| `devkit_session_list` | Beta | 列出已保存的会话 | `limit` |
| `devkit_session_resume` | Beta | 恢复指定会话 | `session_id` |
| `devkit_session_attach` | Beta | 附加到运行中的会话 | `session_id` |
| `devkit_session_detach` | Beta | 从当前会话分离 | `session_id` |
| `devkit_session_activate` | Beta | 激活会话上下文 | `session_id` |
| `devkit_session_search` | Beta | 搜索会话历史 | `query`, `limit` |
| `devkit_session_capture` | Beta | 捕获当前会话快照 | `name` |
| `devkit_session_workflows` | Beta | 获取会话关联的工作流 | `session_id` |
| `devkit_session_recall` | Experimental | 基于 embedding 的语义记忆召回 | `context_id`, `query_embedding`, `limit` |
| `devkit_session_index` | Experimental | 索引会话内容用于搜索 | `session_id` |
| `devkit_session_export` | Experimental | 导出会话为文件 | `session_id`, `format` |
| `devkit_session_import` | Experimental | 从文件导入会话 | `path` |

## Index 管理（3）

| 工具名 | Tier | 一句话描述 | 关键参数 |
|--------|------|-----------|----------|
| `devkit_index` | Beta | 索引仓库摘要、模块结构、代码符号 | `path` |
| `devkit_index_health` | Beta | 检查索引健康状态 | `repo_id` |
| `devkit_index_stream` | Beta | 流式索引进度 | `path` |

## Workflow（3）

| 工具名 | Tier | 一句话描述 | 关键参数 |
|--------|------|-----------|----------|
| `devkit_workflow_list` | Beta | 列出可用工作流 | `limit` |
| `devkit_workflow_run` | Beta | 执行工作流 | `workflow_id`, `args` |
| `devkit_workflow_status` | Beta | 查询工作流执行状态 | `workflow_id` |

## Relation 图谱（3）

| 工具名 | Tier | 一句话描述 | 关键参数 |
|--------|------|-----------|----------|
| `devkit_relation_store` | Beta | 存储实体间关系 | `from`, `to`, `relation_type` |
| `devkit_relation_query` | Beta | 查询实体关系 | `entity_id`, `relation_type` |
| `devkit_relation_delete` | Beta | 删除实体关系 | `from`, `to`, `relation_type` |

## Known Limit（2）

| 工具名 | Tier | 一句话描述 | 关键参数 |
|--------|------|-----------|----------|
| `devkit_known_limit_store` | Beta | 记录已知限制（Hard Veto / Known Bug） | `id`, `category`, `description` |
| `devkit_known_limit_list` | Beta | 列出已知限制 | `category`, `mitigated` |

## 其他（6）

| 工具名 | Tier | 一句话描述 | 关键参数 |
|--------|------|-----------|----------|
| `devkit_query` | Beta | 通用查询（repo/tag/keyword） | `query`, `limit`, `page` |
| `devkit_note` | Beta | 为仓库添加 AI 发现笔记 | `repo_id`, `text`, `author` |
| `devkit_status` | Beta | 检查 devbase 服务状态 | — |
| `devkit_digest` | Experimental | 生成每日知识摘要 | — |
| `devkit_paper_index` | Experimental | 索引学术论文 | `title`, `authors`, `venue` |
| `devkit_search_quality` | Beta | 评估搜索质量指标 | `repo_id`, `query` |
| `devkit_experiment_log` | Beta | 记录实验结果 | `repo_id`, `paper_id`, `status` |
| `devkit_github_info` | Beta | 查询 GitHub 仓库信息 | `owner`, `repo` |
| `devkit_arxiv_fetch` | Beta | 从 arXiv 获取论文元数据 | `query`, `max_results` |
| `devkit_oplog_query` | Beta | 查询操作日志 | `limit`, `repo_id` |
| `devkit_evaluate` | Beta | 评估工具调用结果 | `tool_name`, `result` |

---

## Destructive Gate

以下工具受 `DEVBASE_MCP_ENABLE_DESTRUCTIVE=1` 环境变量控制，默认禁用：

- `devkit_sync`
- `devkit_skill_run`
- `devkit_skill_discover`
- `devkit_vault_write`
- `devkit_relation_store`
- `devkit_relation_delete`
- `devkit_known_limit_store`
- `devkit_workflow_run`

---

## Tier 过滤

通过 `DEVBASE_MCP_TOOL_TIERS` 环境变量控制暴露哪些 tier 的工具：

```json
{"DEVBASE_MCP_TOOL_TIERS": "stable,beta"}
```

默认值：`stable,beta,experimental`（暴露全部）。
