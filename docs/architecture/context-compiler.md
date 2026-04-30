# devbase 架构定义：本地情境编译器

> **版本**：v0.13.0  
> **定位**：devbase 是 AI Agent 在本地数字世界中的海马体——情境编译器（Local Context Compiler）。  
> **核心功能**：将本地数字资产的原始数据（代码库、笔记、Skill、工作流）编译为 AI 可决策的结构化情境。  
> **明确不做**：不替代文件读取、不替代版本控制、不替代包管理。

---

## 一、为什么需要情境编译器

LLM 在本地开发环境中的核心困境：

1. **看不见**：不知道本地有哪些项目、笔记、Skill
2. **读不完**：面对数万行代码，无法判断"该读哪些、为什么读"
3. **记不住**：跨会话丢失上下文，每次从零开始
4. **关系盲**：知道有 A 和 B，不知道 A 调用 B、B 依赖 C

devbase 解决的是**前两个问题**（感知 + 编码），让 LLM 在调用文件工具之前，先获得"该读哪些文件、为什么读、它们之间的关系"。后两个问题（持久化 + 关系）在 v0.13.0 中部分落地。

---

## 二、五层架构

```
┌─────────────────────────────────────────┐
│  认知层：Kimi CLI / Claude / 其他 AI      │  ← 决策与执行（devbase 不介入）
├─────────────────────────────────────────┤
│  协议层：MCP (JSON-RPC 2.0 / stdio)      │  ← 神经突触，38 个 tools
├─────────────────────────────────────────┤
│  编译层：project_context / relations     │  ← 按目标过滤、生成相关结构
│  编码层：entities / entity_types         │  ← 统一模型、类型定义
│  感知层：scan / index / health           │  ← 扫描文件系统、感知存在性
├─────────────────────────────────────────┤
│  持久层：SQLite + Vault + OpLog          │  ← 跨会话记忆
│  资源层：本地文件系统                    │  ← 原始数据，唯一真相源
└─────────────────────────────────────────┘
```

### 各层职责

| 层级 | 模块 | 职责 |
|------|------|------|
| 感知层 | `scan`, `index`, `health` | 发现本地有哪些代码库、笔记、Skill；提取模块结构、代码符号、调用图 |
| 编码层 | `entities`, `entity_types`, `relations` | 统一实体模型：所有对象（repo/skill/paper/vault_note/workflow）以同一套 schema 存储 |
| 编译层 | `project_context`, `dependency_graph`, `symbol_links` | 将原始数据按目标过滤、组装为 AI 可消费的上下文切片 |
| 协议层 | `mcp` | MCP Server，stdio 传输，38 个工具，tier 分级（Stable/Beta/Experimental） |
| 持久层 | `registry` (SQLite), `vault`, `oplog` | 结构化索引 + 文件系统笔记 + 操作审计 |

---

## 三、统一实体模型（Schema v16+）

devbase 不再内置固定概念（repos、skills、vault_notes、papers...），而是通过**三张核心表**定义 workspace 结构：

```sql
-- 实体类型定义（可扩展）
CREATE TABLE entity_types (
    name         TEXT PRIMARY KEY,
    schema_json  TEXT NOT NULL,  -- 该类型允许的字段、校验规则
    description  TEXT,
    created_at   TEXT NOT NULL
);

-- 实体实例（所有对象的统一存储）
CREATE TABLE entities (
    id           TEXT PRIMARY KEY,
    entity_type  TEXT NOT NULL REFERENCES entity_types(name),
    name         TEXT NOT NULL,
    source_url   TEXT,
    local_path   TEXT,
    metadata     TEXT,           -- JSON，动态字段
    content_hash TEXT,
    created_at   TEXT NOT NULL,
    updated_at   TEXT NOT NULL
);

-- 实体间关系（有向图）
CREATE TABLE relations (
    id             TEXT PRIMARY KEY,
    from_entity_id TEXT NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
    to_entity_id   TEXT NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
    relation_type  TEXT NOT NULL,
    metadata       TEXT,
    confidence     REAL NOT NULL DEFAULT 1.0,
    created_at     TEXT NOT NULL
);
```

### 内置实体类型

| 类型 | 说明 | 典型 metadata 字段 |
|------|------|-------------------|
| `repo` | Git 代码库 | language, workspace_type, data_tier, stars |
| `skill` | 可执行 Skill | author, version, tags, entry_script, skill_type |
| `paper` | 学术文献 | authors, venue, year, bibtex, tags |
| `vault_note` | Vault Markdown 笔记 | tags, outgoing_links, linked_repo, frontmatter |
| `workflow` | 工作流定义 | steps_json, inputs_schema, outputs_schema |

### 优势

- **新增概念无需改表**：新增 `entity_type` 记录即可，无需 `ALTER TABLE`
- **统一查询语言**：所有对象通过 `entities` + `entity_type` 过滤，JOIN `relations` 获取关联
- **关系即数据**：代码依赖、论文引用、笔记反链都以 `relations` 存储，支持图遍历

---

## 四、六维信息模型与当前供给

LLM 决策需要六维结构化情境：

| 维度 | 人类等效 | v0.13.0 供给状态 |
|------|---------|-----------------|
| **Situation** | "我房间里有什么书？" | ✅ `scan` + `query_repos` + `vault_search` 提供全景 |
| **State** | "哪些书还没读完？" | ✅ `health` + `index` 状态 + Git dirty/behind/ahead |
| **Relations** | "这本书引用了哪本？" | 🟡 `relations` 表已激活（v24 迁移），`project_context` 包含 `related_symbols` |
| **Capability** | "我可以用笔划线、用书签标记" | ✅ 38 个 MCP tools，`project_context` 聚合 |
| **History** | "上次读这本书到哪一章？" | 🟡 `project_context` 包含 `activity`（最近 10 条 oplog）；`agent_symbol_reads` 表已激活（v25），用于行为信号 boosting |
| **Relevance** | "我现在要写论文，哪些书相关？" | 🟡 `project_context` 支持 `goal` 参数，通过 `hybrid_search_symbols` 关联排序 |

---

## 五、与 AI Agent 的契约

### devbase 的承诺

- **协议稳定**：MCP 消息格式符合规范（Content-Length 头，无尾随字节），notification 静默处理
- **默认安全**：destructive 工具（sync/skill_run）需 `DEVBASE_MCP_ENABLE_DESTRUCTIVE=1` 显式启用；vault 路径锁定 workspace 根目录；skill 环境白名单隔离
- **结构可消费**：返回 JSON，含 `success` / `error` 统一字段，schema 自描述

### AI Agent 的最佳实践

1. **先问 devbase，再读文件**：复杂任务先调用 `project_context` 获取模块树 + 符号 + 调用关系，再按需读文件
2. **利用 Vault 做跨会话记忆**：关键决策写入 Vault（`vault_write`），下次会话通过 `vault_search` 召回
3. **通过 OpLog 审计**：重要操作后查询 `devkit_oplog_query` 确认执行记录

---

## 六、数据流示例："分析 sync 模块"

```
1. 感知层：scan 发现 devbase 项目
   └─ index 提取模块结构（cargo metadata）
   └─ semantic_index 提取符号 + 调用图

2. 编码层：entities 存储 repo 元数据
   └─ repo_modules 表存储 cargo targets
   └─ code_symbols 表存储函数/结构体/枚举
   └─ code_call_graph 表存储调用边

3. 编译层：project_context("devbase", goal="sync policy")
   └─ 返回 repo 元数据 + vault 笔记 + assets
   └─ 返回 modules（cargo targets）
   └─ 返回 symbols（ relevance-ranked Top 50）+ calls（filtered Top 50）
   └─ 返回 activity（最近 10 条 oplog）+ related_symbols（概念关联符号）

4. 协议层：MCP 将 JSON 传给 Kimi CLI

5. 认知层：Kimi CLI 决定读取 src/sync/policy.rs
```

---

## 七、相关文档

- [`reference/schema-v23.md`](../reference/schema-v23.md) — 数据库 Schema 完整定义
- [`reference/entities-model.md`](../reference/entities-model.md) — 统一实体模型查询模式
- [`reference/mcp-tools.md`](../reference/mcp-tools.md) — 38 个 MCP 工具速查
- [`guides/quickstart.md`](../guides/quickstart.md) — 5 分钟上手指南
