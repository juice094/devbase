# MCP Tools 参考

devbase MCP Server 提供 **38 个 tools**，通过 stdio 传输与 AI Agent 通信。工具按稳定性分为三级：

- **Stable** — 经过充分测试，schema 冻结
- **Beta** — 功能验证通过，schema 可能微调
- **Experimental** — 新功能，行为可能变化

---

## 仓库管理（5）

| 工具名 | Tier | 一句话描述 | 关键参数 |
|--------|------|-----------|----------|
| `devkit_scan` | Beta | 扫描目录发现 Git 仓库并注册 | `path`, `register` |
| `devkit_health` | Stable | 检查注册仓库的健康状态（dirty/behind/ahead） | `detail`, `limit`, `page` |
| `devkit_sync` | Beta | 安全同步仓库与上游（destructive gate） | `repo_id`, `dry_run` |
| `devkit_query_repos` | Stable | 查询已注册仓库列表，支持 tag/language 过滤 | `query`, `limit`, `page` |
| `devkit_index` | Beta | 索引仓库摘要、模块结构、代码符号 | `path` |

## 代码分析（6）

| 工具名 | Tier | 一句话描述 | 关键参数 |
|--------|------|-----------|----------|
| `devkit_code_metrics` | Experimental | 统计代码行数、语言分布、测试覆盖率 | `repo_id` |
| `devkit_module_graph` | Experimental | 获取仓库模块依赖图 | `repo_id` |
| `devkit_code_symbols` | Beta | 列出仓库中的代码符号（函数/结构体/枚举等） | `repo_id`, `file_path`, `symbol_type` |
| `devkit_dependency_graph` | Beta | 获取跨仓库依赖关系图 | `repo_id` |
| `devkit_call_graph` | Experimental | 获取函数调用图 | `repo_id`, `symbol_name` |
| `devkit_dead_code` | Experimental | 检测未被调用的私有函数 | `repo_id`, `include_pub` |

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

## Vault 笔记（4）

| 工具名 | Tier | 一句话描述 | 关键参数 |
|--------|------|-----------|----------|
| `devkit_vault_search` | Stable | 关键词搜索 Vault 笔记 | `query` |
| `devkit_vault_read` | Stable | 读取指定 Vault 笔记的完整内容 | `path` |
| `devkit_vault_write` | Beta | 写入或更新 Vault 笔记（destructive gate） | `path`, `content`, `frontmatter` |
| `devkit_vault_backlinks` | Beta | 查找指向指定笔记的反向链接 | `note_id` |

## Skill 运行时（4）

| 工具名 | Tier | 一句话描述 | 关键参数 |
|--------|------|-----------|----------|
| `devkit_skill_list` | Beta | 列出已安装的 Skill | `limit`, `tag` |
| `devkit_skill_search` | Beta | 语义搜索 Skill | `query`, `limit` |
| `devkit_skill_run` | Beta | 执行指定 Skill（destructive gate） | `skill_id`, `args` |
| `devkit_skill_discover` | Beta | 将当前项目封装为 Skill（destructive gate，dry_run 默认 true） | `path` |

## 项目上下文（1）

| 工具名 | Tier | 一句话描述 | 关键参数 |
|--------|------|-----------|----------|
| `devkit_project_context` | Stable | 获取项目统一上下文（repo + vault + assets + modules + symbols + calls） | `project` |

## 其他（10）

| 工具名 | Tier | 一句话描述 | 关键参数 |
|--------|------|-----------|----------|
| `devkit_query` | Beta | 通用查询（repo/tag/keyword） | `query`, `limit`, `page` |
| `devkit_note` | Beta | 为仓库添加 AI 发现笔记 | `repo_id`, `text`, `author` |
| `devkit_digest` | Experimental | 生成每日知识摘要 | — |
| `devkit_paper_index` | Experimental | 索引学术论文 | `title`, `authors`, `venue` |
| `devkit_experiment_log` | Experimental | 记录实验结果 | `repo_id`, `paper_id`, `status` |
| `devkit_github_info` | Beta | 查询 GitHub 仓库信息 | `owner`, `repo` |
| `devkit_arxiv_fetch` | Beta | 从 arXiv 获取论文元数据 | `query`, `max_results` |
| `devkit_known_limit_store` | Beta | 记录已知限制（Hard Veto / Known Bug） | `id`, `category`, `description` |
| `devkit_known_limit_list` | Beta | 列出已知限制 | `category`, `mitigated` |
| `devkit_oplog_query` | Beta | 查询操作日志 | `limit`, `repo_id` |

---

## Destructive Gate

以下工具受 `DEVBASE_MCP_ENABLE_DESTRUCTIVE=1` 环境变量控制，默认禁用：

- `devkit_sync`
- `devkit_skill_run`
- `devkit_skill_discover`
- `devkit_vault_write`

---

## Tier 过滤

通过 `DEVBASE_MCP_TOOL_TIERS` 环境变量控制暴露哪些 tier 的工具：

```json
{"DEVBASE_MCP_TOOL_TIERS": "stable,beta"}
```

默认值：`stable,beta,experimental`（暴露全部）。
