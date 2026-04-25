# Workspace as Schema：架构思考记录

> **状态**：思考中，未进入实现  
> **日期**：2026-04-25  
> **触发**：用户提出"devbase 的本质是不是一个用来定义 workspace 的架构？"

---

## 一、用户的核心洞察

devbase 不应该内置固定的概念（repos、skills、vault_notes、papers...），而应该是一种**workspace 架构的定义方式**。

用户的真实 workspace 不是"Git 仓库列表"，而是一个**"AI 基础设施研究实验室"**：
- 核心项目（自己在做的）：devbase、clarity、agri-paper
- 参考库（clone 来学习的）：zeroclaw、openclaw、burn、candle...
- 输入（读过的）：papers、docs、blog posts
- 输出（产出的）：notes、experiments、skills
- 关系：zeroclaw similar_to clarity、paper_A inspired experiment_B

---

## 二、当前 devbase 的问题

**把自己的 schema 强加给了用户的 workspace。**

当前隐式 schema：
```rust
struct DevbaseWorkspace {
    repos: Vec<GitRepo>,
    skills: Vec<Skill>,
    vault_notes: Vec<VaultNote>,
    papers: Vec<Paper>,
    code_symbols: Vec<CodeSymbol>,
}
```

但用户的真实 workspace 是：
```rust
struct AiResearchLab {
    projects: Vec<Project>,      // 自己在做的
    references: Vec<Reference>,  // clone 来学习的
    readings: Vec<Reading>,      // 读过的
    outputs: Vec<Output>,        // 产出的
    relations: Vec<Relation>,    // 之间的关系
}
```

每次新增概念（Memory、MCP Tool、Diary）都要加新表 → 不可持续。

---

## 三、"Workspace 架构定义引擎"方向

### 核心模型（3 张表）

```sql
CREATE TABLE workspaces (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    schema_json TEXT NOT NULL,  -- 定义 entity_type / relation_type / field_type
    created_at TEXT NOT NULL
);

CREATE TABLE entities (
    id TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL,
    entity_type TEXT NOT NULL,  -- 由 workspace.schema_json 定义，非固定
    name TEXT NOT NULL,
    fields_json TEXT NOT NULL,  -- 动态字段
    content TEXT,               -- 全文内容（用于搜索 + embedding）
    embedding BLOB,
    created_at TEXT,
    updated_at TEXT
);

CREATE TABLE relations (
    id INTEGER PRIMARY KEY,
    workspace_id TEXT NOT NULL,
    source_id TEXT NOT NULL,
    target_id TEXT NOT NULL,
    relation_type TEXT NOT NULL, -- 由 workspace.schema_json 定义
    strength REAL,
    metadata TEXT,
    created_at TEXT
);
```

### Workspace 定义示例

```toml
# ~/.config/devbase/workspaces/ai-research-lab.toml
[workspace]
name = "AI Research Lab"

[[entity_type]]
name = "project"
fields = [
    { name = "local_path", type = "path", required = true },
    { name = "language", type = "string" },
    { name = "status", type = "enum", values = ["active", "paused", "archived"] },
]

[[entity_type]]
name = "reference"
fields = [
    { name = "local_path", type = "path", required = true },
    { name = "learned", type = "bool", default = false },
    { name = "key_takeaways", type = "markdown" },
]

[[entity_type]]
name = "reading"
fields = [
    { name = "title", type = "string", required = true },
    { name = "source_url", type = "url" },
    { name = "format", type = "enum", values = ["pdf", "web", "book"] },
]

[[entity_type]]
name = "output"
fields = [
    { name = "title", type = "string", required = true },
    { name = "content", type = "markdown" },
    { name = "output_type", type = "enum", values = ["note", "experiment", "skill"] },
]

[[relation_type]]
name = "similar_to"
source_types = ["project", "reference"]
target_types = ["project", "reference"]

[[relation_type]]
name = "inspired_by"
source_types = ["project", "output"]
target_types = ["reference", "reading"]

[[relation_type]]
name = "about"
source_types = ["output"]
target_types = ["project", "reference", "reading"]
```

### 发现源（Adapters）

Git 仓库、Vault 笔记、Skill 目录、PDF 文件——这些是**发现源**，不是核心模型。

```
文件系统（Git / Vault / Skill / PDF）
    │
    ▼  Discovery Adapter（自动发现）
workspace.entities（统一存储）
    │
    ▼  Query Engine（统一查询）
devbase similar / compare / why / query
```

- Git 仓库 → 可能是 `project` 或 `reference`（根据 owner、activity 判断）
- Vault 笔记 → 可能是 `output`（note）或 `reading`（读书笔记）
- Skill 脚本 → 可能是 `output`（skill）
- PDF → 可能是 `reading`

---

## 四、与现有产品的区别

| 产品 | 模式 | 问题 |
|------|------|------|
| Notion | 在线数据库 | 不能管理本地 Git 仓库 |
| Obsidian | 本地 Markdown | 不懂 Git、不能解析代码结构 |
| Anytype | 分布式块 | 块模型不适合 Git 工作流 |
| **devbase** | **本地 + Git-aware + 代码语义** | 当前是固定 schema，不是可定义架构 |

**devbase 的 unique value prop**：
> "定义你的 workspace 架构，自动从文件系统发现实体，提供代码级语义理解。"

---

## 五、待回答的关键问题

1. **Entity 的发现逻辑**：Git 仓库怎么自动分类为 `project` vs `reference`？
2. **字段类型系统**：`path`、`string`、`enum`、`markdown`、`url` — 够吗？需要 nested object 吗？
3. **多 workspace**：一个 devbase 实例支持多个 workspace 吗？还是 workspace 之间完全隔离？
4. **现有数据的迁移**：repos/skills/vault_notes/code_symbols 怎么映射到新模型？
5. **查询语言**：`devbase query 'type=project status=active'` 这种 DSL 怎么设计？
6. **和当前 SQLite schema 的关系**：是重构（删掉旧表）还是叠加（新表 + 旧表共存）？

---

## 六、下一步等待用户提问

用户说"先存一下你的理解，我问几个问题"。本文档即保存的理解，等待用户提问。
