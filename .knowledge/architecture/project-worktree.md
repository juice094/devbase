---
type: ArchitectureTopology
title: devbase 项目工作树（Project Worktree）
description: 按实际文件系统整理的 devbase 项目结构，用于 Agent 快速定位模块与文件。与 AGENTS.md 末尾的“完整项目结构参考”保持同步。
version: 0.20.1
schema_version: 36
mcp_tools: 71
crates: 12
tests: 616+
timestamp: 2026-06-25T11:28:04Z
tags: [architecture, worktree, project-structure, topology]
---

# devbase 项目工作树

> 本文件按实际目录列出 devbase 的关键文件与模块职责。它是 AGENTS.md 中“完整项目结构参考”的 OKF 归档版本。若目录结构发生变更，请同步更新两者。

---

## 顶层布局

```text
devbase/
├── .cargo/                  # 本地 Cargo 配置（RUST_TEST_THREADS=1）
├── .github/                 # CI / Release workflows
├── .knowledge/              # OKF Knowledge Bundle（架构、Registry、MCP、开发规范）
│   ├── index.md             # Bundle 总入口
│   ├── architecture/        # 架构概念文档
│   ├── registry/            # Registry Schema / 迁移策略
│   ├── mcp/                 # MCP 工具规范
│   ├── development/         # 构建、测试、代码风格
│   └── log.md               # Bundle 变更日志
├── benches/                 # Criterion 基准测试
├── crates/                  # 12 个独立 workspace crate（零内部耦合）
├── docs/                    # 人类可读文档导航
├── examples/                # 可运行示例与集成演示
├── scripts/                 # 安装脚本与 CI 辅助
├── skills/                  # 示例 Skill
├── src/                     # 主应用程序（30+ 模块）
├── tests/                   # 集成测试
├── AGENTS.md                # Agent 入口指引（本项目门面文件）
├── CLAUDE.md                # Claude Code 专用指引
├── Cargo.toml               # Workspace 配置
├── README.md                # 项目首页
└── ...                      # 许可证、贡献指南、CHANGELOG 等
```

---

## `src/` 主应用程序

### 交互层（Application / Protocol）

```text
src/
├── main.rs                  # CLI 入口：命令解析与分发（RF-4 ≤ 1000 行）
├── lib.rs                   # 导出 30+ 模块
├── commands/                # 9 类 CLI 子命令
│   ├── mod.rs
│   ├── analysis.rs
│   ├── knowledge.rs
│   ├── limit.rs
│   ├── ontology.rs
│   ├── repo.rs
│   ├── simple.rs
│   ├── skill.rs
│   ├── system.rs
│   └── workflow.rs
├── tui/                     # ratatui 终端仪表盘
│   ├── mod.rs
│   ├── event.rs             # 键盘/异步事件路由
│   ├── layout.rs            # 响应式布局
│   ├── theme.rs             # Design Token 与色彩语义
│   ├── render/              # 渲染组件（list/detail/popups/help/logs）
│   └── state/               # TUI 状态机
└── mcp/                     # MCP Server（stdio，71 个工具）
    ├── mod.rs               # McpTool trait、McpToolEnum、请求路由
    ├── clients.rs           # MCP 客户端适配
    ├── tests.rs             # MCP 单元测试
    └── tools/               # 71 个工具实现
        ├── mod.rs
        ├── repo.rs
        ├── query.rs
        ├── vault.rs
        ├── skill.rs
        ├── context.rs
        └── ...              # 其他工具分组
```

### 编译层（Compilation / Knowledge）

```text
src/
├── registry/                # SQLite Registry：schema、迁移、实体、关系
│   ├── migrate.rs           # CURRENT_SCHEMA_VERSION = 36
│   ├── test_helpers.rs      # SCHEMA_DDL（内存测试 schema）
│   ├── entity.rs
│   ├── relation.rs
│   ├── repo.rs
│   ├── workspace.rs
│   ├── knowledge.rs
│   ├── knowledge_meta.rs
│   ├── vault.rs
│   ├── code_symbols.rs
│   ├── call_graph.rs
│   ├── dead_code.rs
│   ├── links.rs
│   ├── metrics.rs
│   ├── known_limits.rs
│   ├── import_ontology.rs
│   ├── agent_context.rs
│   ├── health.rs
│   └── tests.rs
├── repository/              # 仓库抽象（对 registry 的业务封装）
│   ├── mod.rs
│   ├── repo.rs
│   ├── workspace.rs
│   ├── dependency.rs
│   ├── health.rs
│   ├── knowledge.rs
│   ├── search.rs
│   └── symbol.rs
├── search/                  # Tantivy BM25 + 向量混合检索
│   ├── mod.rs
│   ├── hybrid.rs            # 混合检索编排
│   └── symbol_index.rs      # 符号索引
├── semantic_index/          # tree-sitter 代码符号提取
│   ├── mod.rs
│   ├── symbol.rs
│   ├── call_graph.rs
│   ├── git_diff.rs
│   └── persist.rs
├── vault/                   # PARA 笔记系统
│   ├── mod.rs
│   ├── scanner.rs           # Vault 目录扫描
│   ├── indexer.rs           # Tantivy 索引
│   ├── frontmatter.rs       # YAML frontmatter 解析
│   ├── wikilink.rs          # [[wikilink]] 解析
│   ├── backlinks.rs         # 反向链接 + BFS 图遍历
│   ├── fs_io.rs             # Vault 文件原子操作
│   ├── history.rs           # 历史追踪
│   └── export.rs            # 导出功能
├── skill_runtime/           # Skill 生命周期
│   ├── mod.rs
│   ├── parser.rs            # SKILL.md 解析
│   ├── registry.rs          # Skill 注册表 CRUD
│   ├── discover.rs          # Skill 发现
│   ├── dependency.rs        # Skill 依赖拓扑
│   ├── executor.rs          # Skill 执行器
│   ├── scoring.rs           # Skill 评分
│   ├── publish.rs           # Skill 发布
│   ├── sources.rs           # Skill 源管理
│   ├── clarity_sync.rs      # Clarity 同步
│   └── sync_adapter.rs
├── skill_sync.rs            # Vault → Skill 导出桥接
├── workflow/                # YAML 工作流引擎
│   ├── mod.rs
│   ├── model.rs             # 数据模型
│   ├── parser.rs            # YAML 解析
│   ├── validator.rs         # 工作流验证
│   ├── scheduler.rs         # 拓扑调度
│   ├── executor.rs          # DAG 执行器
│   ├── interpolate.rs       # 变量插值
│   └── state.rs             # 执行状态
├── knowledge_engine/        # README 摘要、关键词、模块信息探测
│   ├── mod.rs
│   ├── readme.rs
│   ├── module.rs
│   ├── index.rs
│   ├── index_state.rs
│   ├── llm.rs
│   └── fallback.rs
└── sync/                    # 仓库同步编排
    ├── mod.rs
    ├── orchestrator.rs
    ├── policy.rs
    ├── tasks.rs
    └── tests.rs
```

### 可靠层与基础能力（Reliability / Storage / Utilities）

```text
src/
├── storage.rs               # StorageBackend trait + AppContext（依赖注入容器）
├── config.rs                # 配置结构体
├── i18n/                    # 国际化
│   ├── mod.rs
│   ├── en.rs
│   └── zh_cn.rs
├── core/                    # 原子类型（Node / Edge / NodeType）
│   ├── mod.rs
│   └── node.rs
├── asyncgit.rs              # 异步 Git 通知通道
├── scan.rs                  # 仓库扫描入口
├── query.rs                 # 结构化查询表达式
├── health.rs                # 健康状态计算
│   └── env_cache.rs
├── oplog_analytics.rs       # 操作日志与覆盖率分析
├── backup.rs                # Schema 迁移前自动快照
├── watch.rs                 # 目录监控
├── dependency_graph.rs      # 跨仓库依赖图
├── symbol_links.rs          # 符号链接（RE-EXPORT ONLY）
├── discovery_engine.rs      # 跨仓库发现
├── embedding.rs             # 向量嵌入封装
├── digest.rs                # 摘要/哈希工具
├── arxiv.rs                 # arXiv 元数据抓取
├── greptime.rs              # GreptimeDB 可选集成
├── syncthing_client.rs      # Syncthing 客户端
├── sync_protocol.rs         # 同步协议基础类型
├── clients.rs               # 通用客户端
├── daemon.rs                # 守护进程入口
├── test_utils.rs            # 测试辅助
└── lib.rs / main.rs
```

---

## `crates/` — 12 个 Workspace Crate

```text
crates/
├── devbase-core-types            # 最底层：Node / Edge / NodeType 核心类型
├── devbase-registry              # Registry 核心逻辑
├── devbase-embedding             # 本地文本嵌入（Candle + Ollama）
├── devbase-vault-wikilink        # WikiLink 解析器
├── devbase-vault-frontmatter     # Vault Frontmatter 解析
├── devbase-skill-runtime-parser  # Skill 运行时解析器
├── devbase-skill-runtime-types   # Skill 运行时类型
├── devbase-symbol-links          # 符号链接
├── devbase-sync-protocol         # 同步协议
├── devbase-syncthing-client      # Syncthing 客户端
├── devbase-workflow-model        # Workflow 数据模型
└── devbase-workflow-interpolate  # Workflow 变量插值
```

---

## 依赖方向

```text
devbase-core-types （零内部耦合）
    ↓
{ devbase-registry, devbase-embedding, devbase-vault-*, devbase-skill-runtime-*,
  devbase-symbol-links, devbase-sync-protocol, devbase-syncthing-client,
  devbase-workflow-* }
    ↓
src/ 各模块（commands, tui, mcp, registry, search, vault, skill_runtime, workflow, ...）
```

- `devbase-core-types` 为最底层基础 crate，禁止依赖任何 devbase 内部 crate。
- `crates/` 内各 crate 禁止直接调用主 crate (`src/`) 的 `crate::` 路径。
- `src/` 各模块可聚合所有 crate 与内部模块能力，`main.rs` 为唯一二进制入口。

---

## 维护提示

- 新增/删除 `src/` 顶层模块时，同步更新 `src/lib.rs` 的导出。
- 新增/删除 `crates/` 成员时，同步更新 `Cargo.toml` workspace members 与本文档。
- 工具数、Schema 版本、测试数等关键数字变更时，同步更新 `README.md`、`AGENTS.md`、`CLAUDE.md`、`.knowledge/index.md` 与本文件 frontmatter。
