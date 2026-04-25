# 个人知识库跃迁：Repo 语义理解与跨仓库知识图谱

> **方向**：devbase 从"仓库列表"进化为"仓库关系网络"  
> **目标用户**：你自己（单人维护者，50+ AI 项目参考库）  
> **核心问题**："和 clarity 相似的项目有哪些？""zeroclaw 比 clarity 多实现了什么？""我为什么 clone 这个项目？"  
> **版本**：v0.4.0 规划  
> **预计工期**：4 个波次，总计 1-2 周

---

## 0. 设计哲学

> 不是给外部用户做的功能，是给你自己用的外置大脑。

- **每个 repo 不是一行记录，而是一个知识节点**
- **repo 之间的关系和 repo 本身同等重要**
- **你的笔记是数据的一部分，不是附属品**
- **查询要支持自然语言，不是 SQL**

---

## 1. 架构总览

```
┌─────────────────────────────────────────────────────────────┐
│  查询层（Query Layer）                                       │
│  ─────────────────────                                       │
│  • devbase similar <repo>      → 相似仓库列表               │
│  • devbase compare <a> <b>     → 技术栈对比报告             │
│  • devbase why <repo>          → 显示你的笔记               │
│  • devbase stack <tech>        → 使用某技术的所有仓库       │
│  • devbase query "llm provider in rust" → 自然语言查询      │
├─────────────────────────────────────────────────────────────┤
│  关系层（Relationship Layer）                                │
│  ──────────────────────────                                  │
│  • 技术栈交集相似度（Jaccard）                                │
│  • README 描述语义相似度（Embedding 余弦）                    │
│  • 依赖重叠度                                               │
│  • 复合相似度评分                                           │
├─────────────────────────────────────────────────────────────┤
│  画像层（Profile Layer）                                     │
│  ────────────────────                                        │
│  • README 解析：描述、关键词、安装方式                        │
│  • Cargo.toml：workspace 结构、依赖列表、features             │
│  • package.json：scripts、dependencies、keywords              │
│  • go.mod：模块路径、依赖                                   │
│  • pyproject.toml：dependencies、entry points                 │
├─────────────────────────────────────────────────────────────┤
│  存储层（Storage Layer）                                     │
│  ────────────────────                                        │
│  • repo_profiles 表：JSON 画像数据                            │
│  • repo_relationships 表：相似度 + 关系类型                   │
│  • repo_notes 表：用户笔记（Markdown）                        │
│  • repo_embeddings 表：README 描述向量（384-dim）             │
└─────────────────────────────────────────────────────────────┘
```

---

## 2. 数据库 Schema（v16）

### 2.1 `repo_profiles` 表

```sql
CREATE TABLE repo_profiles (
    repo_id         TEXT PRIMARY KEY REFERENCES repos(id) ON DELETE CASCADE,
    description     TEXT,           -- 从 README 提取的描述
    topics          TEXT,           -- JSON ["ai", "agent", "rust"]
    tech_stack      TEXT,           -- JSON {"language":"rust","frameworks":["tokio","axum"],"build_tool":"cargo"}
    dependencies    TEXT,           -- JSON [{"name":"tokio","version":"1.43","category":"async"}]
    workspace_info  TEXT,           -- JSON {"members":3,"total_crates":17,"edition":"2024"}
    readme_length   INTEGER,        -- README 字符数
    has_docker      BOOLEAN,        -- 是否有 Dockerfile
    has_ci          BOOLEAN,        -- 是否有 .github/workflows
    profiled_at     TEXT NOT NULL,  -- ISO 8601
    profile_version INTEGER DEFAULT 1  -- 画像格式版本，未来升级用
);

CREATE INDEX idx_repo_profiles_topics ON repo_profiles(topics);
CREATE INDEX idx_repo_profiles_tech_stack ON repo_profiles(tech_stack);
```

### 2.2 `repo_relationships` 表

```sql
CREATE TABLE repo_relationships (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    source_repo     TEXT NOT NULL REFERENCES repos(id) ON DELETE CASCADE,
    target_repo     TEXT NOT NULL REFERENCES repos(id) ON DELETE CASCADE,
    relation_type   TEXT NOT NULL,  -- "similar" | "depends_on" | "inspired_by" | "fork_of" | "contrasts_with"
    similarity_score REAL,          -- 0.0 ~ 1.0，复合评分
    details         TEXT,           -- JSON {"tech_overlap":0.8,"desc_similarity":0.6,"dep_overlap":0.3}
    computed_at     TEXT NOT NULL,
    UNIQUE(source_repo, target_repo, relation_type)
);

CREATE INDEX idx_repo_relationships_source ON repo_relationships(source_repo);
CREATE INDEX idx_repo_relationships_target ON repo_relationships(target_repo);
CREATE INDEX idx_repo_relationships_score ON repo_relationships(similarity_score DESC);
```

### 2.3 `repo_notes` 表

```sql
CREATE TABLE repo_notes (
    repo_id         TEXT PRIMARY KEY REFERENCES repos(id) ON DELETE CASCADE,
    notes           TEXT NOT NULL DEFAULT '',  -- Markdown
    clone_reason    TEXT,                       -- "为什么 clone"
    key_learnings   TEXT,                       -- JSON ["provider 抽象用 trait","TUI 用 ratatui"]
    todo_items      TEXT,                       -- JSON ["对比 zeroclaw 的 memory 模块","学习它的 plugin 系统"]
    updated_at      TEXT NOT NULL
);
```

### 2.4 `repo_embeddings` 表（复用现有 embedding 基础设施）

```sql
CREATE TABLE repo_embeddings (
    repo_id         TEXT PRIMARY KEY REFERENCES repos(id) ON DELETE CASCADE,
    embedding       BLOB,           -- f32 BLOB，384-dim（复用 skill embedding 模型）
    source_text     TEXT,           -- 用于生成 embedding 的原文（README 描述拼接）
    generated_at    TEXT NOT NULL
);
```

---

## 3. 画像提取器（Profile Extractor）

### 3.1 提取策略

| 文件 | 提取内容 | 优先级 |
|------|---------|--------|
| `README.md` | 描述（前 3 段）、关键词（从 badges/sections 推断）、安装方式 | P0 |
| `Cargo.toml` | workspace members、dependencies、features、edition | P0 |
| `package.json` | dependencies、devDependencies、keywords、scripts | P0 |
| `go.mod` | module path、go version、require | P1 |
| `pyproject.toml` | dependencies、optional-dependencies | P1 |
| `Dockerfile` | 存在性 → `has_docker` | P1 |
| `.github/workflows/*.yml` | 存在性 → `has_ci` | P1 |

### 3.2 README 描述提取算法

```rust
fn extract_description(readme: &str) -> String {
    // 策略：取第一个非空、非 badge、非 heading 的段落
    let lines: Vec<&str> = readme.lines().collect();
    let mut paragraphs = vec![];
    let mut current = String::new();
    
    for line in lines {
        let trimmed = line.trim();
        // 跳过空行、badge 行、纯链接行
        if trimmed.is_empty() 
            || trimmed.starts_with("[![")
            || trimmed.starts_with("<img")
            || trimmed.starts_with("<p")
            || trimmed.starts_with("</p")
            || trimmed.starts_with("---")
            || trimmed.starts_with("# ") {
            if !current.trim().is_empty() {
                paragraphs.push(current.trim().to_string());
                current.clear();
            }
            continue;
        }
        current.push_str(line);
        current.push('\n');
    }
    
    // 取前 3 个有效段落，拼接成描述
    paragraphs.iter().take(3).cloned().collect::<Vec<_>>().join("\n\n")
}
```

### 3.3 技术栈标准化

提取原始依赖后，映射到标准化标签：

```rust
fn normalize_tech_stack(deps: &[String]) -> Vec<String> {
    let mapping = hashmap! {
        "tokio" => "async-runtime",
        "axum" => "web-framework",
        "reqwest" => "http-client",
        "serde" => "serialization",
        "ratatui" => "tui-framework",
        "crossterm" => "terminal",
        "git2" => "git",
        "rusqlite" => "database",
        "tantivy" => "search-engine",
        "tree-sitter" => "parser",
        "blake3" => "crypto",
        "pyo3" => "python-ffi",
        "wasm-bindgen" => "wasm",
    };
    deps.iter()
        .filter_map(|d| mapping.get(d.as_str()).map(|&s| s.to_string()))
        .collect()
}
```

---

## 4. 相似度计算

### 4.1 三维度评分

| 维度 | 算法 | 权重 |
|------|------|------|
| **技术栈交集** | Jaccard(tech_stack_a, tech_stack_b) | 40% |
| **README 语义** | Cosine(embedding_a, embedding_b) | 40% |
| **依赖重叠** | |dep_a ∩ dep_b| / |dep_a ∪ dep_b| | 20% |

### 4.2 复合相似度

```rust
fn compute_similarity(a: &RepoProfile, b: &RepoProfile) -> f32 {
    let tech_jaccard = jaccard(&a.tech_stack, &b.tech_stack);
    let desc_cosine = cosine_similarity(&a.embedding, &b.embedding);
    let dep_overlap = jaccard(
        &a.dependencies.iter().map(|d| d.name.clone()).collect::<HashSet<_>>(),
        &b.dependencies.iter().map(|d| d.name.clone()).collect::<HashSet<_>>(),
    );
    
    tech_jaccard * 0.4 + desc_cosine * 0.4 + dep_overlap * 0.2
}
```

### 4.3 关系类型推断

| 条件 | 关系类型 |
|------|---------|
| similarity_score > 0.7 | `similar` |
| a.dependencies 包含 b.repo_id | `depends_on` |
| a.upstream_url 是 b.upstream_url 的 fork | `fork_of` |
| a 和 b 有相同作者，定位不同 | `contrasts_with` |

---

## 5. CLI 接口

### 5.1 `devbase similar <repo-id>`

```bash
$ devbase similar clarity

与 clarity 最相似的仓库：

1. zeroclaw          相似度 0.87
   原因：同为 Rust AI Agent，共享 tokio/reqwest/serde/ratatui
   差异：zeroclaw 多 14 个 crate，有 hardware/gateway 层

2. claude-code-rust  相似度 0.62
   原因：Rust AI 编程助手，有 LLM 交互层
   差异：无 TUI，聚焦代码编辑而非通用对话

3. openclaw          相似度 0.41
   原因：AI 助手，多渠道网关
   差异：TypeScript/Node 栈，跨平台移动优先
```

### 5.2 `devbase compare <a> <b>`

```bash
$ devbase compare clarity zeroclaw

# 技术栈对比
| 维度          | clarity        | zeroclaw       |
|---------------|----------------|----------------|
| 语言          | Rust           | Rust           |
| Crate 数      | 3              | 17             |
| Edition       | 2021           | 2024           |
| TUI           | ratatui        | ratatui        |
| LLM Providers | 13 (内置)      | zeroclaw-providers crate |
| MCP Client    | ✅             | ✅             |
| Memory 系统   | ❌             | zeroclaw-memory |
| Plugin 系统   | ❌             | zeroclaw-plugins |
| Hardware 抽象 | ❌             | zeroclaw-hardware |

# 依赖差异（ clarity 有而 zeroclaw 无）
- tantivy (全文搜索)
- tree-sitter-* (多语言解析)
- git2 (Git 操作)

# 依赖差异（ zeroclaw 有而 clarity 无）
- wgpu (GPU 渲染)
- tauri (桌面应用框架)
- redis (缓存)
```

### 5.3 `devbase why <repo-id>`

```bash
$ devbase why zeroclaw

【clone 原因】
2026-04-05: 研究 Rust AI Agent 的 crate 拆分策略。
zeroclaw 把 providers/memory/tools/runtime 拆成独立 crate，
想学习这种模块化方式是否适合 clarity。

【关键学习】
- provider 抽象用动态分发而非泛型，减少编译时间
- memory 用 SQLite + 向量索引，和 devbase 的 semantic_index 思路一致
- gateway 用 axum + SSE，可作为 clarity SSE 的参考实现

【待办】
- [ ] 对比 zeroclaw-memory 和 devbase 的 code_embeddings 表设计
- [ ] 评估 zeroclaw-plugins 的 WASM 方案是否适合 clarity
```

### 5.4 `devbase stack <tech>`

```bash
$ devbase stack ratatui

使用 ratatui 的仓库（5）：

1. devbase       — TUI 仪表盘，多仓库管理
2. clarity       — TUI 聊天界面
3. zeroclaw      — TUI 运行时监控
4. gitui         — Git TUI（第三方）
5. openclaw      — 无 TUI（macOS native UI）
```

### 5.5 `devbase query "<natural language>"`

```bash
$ devbase query "rust llm provider with tui"

理解：language=rust, topics 包含 llm/provider, tech_stack 包含 tui-framework

结果（3）：
1. clarity      — 多提供商 LLM 系统，ratatui TUI
2. zeroclaw     — AI Agent，ratatui TUI，13 个 provider
3. devbase      — 非 LLM，但有 TUI（不匹配，展示为"相关"）
```

---

## 6. TUI 集成

在 `RepoList` 视图选中仓库时，右侧面板（detail.rs）增加 **"Knowledge" Tab**：

```
┌─────────────────────────────────────┐
│  Overview │ Health │ Insights │ 📖 Knowledge │
├─────────────────────────────────────┤
│  【技术栈】                          │
│  Language: Rust 2021                 │
│  Crates: 3                           │
│  Frameworks: tokio, reqwest, serde,  │
│              ratatui, git2           │
│                                      │
│  【相似仓库】                        │
│  1. zeroclaw       (87%)  ⭐ 最相似 │
│  2. claude-code-rust (62%)          │
│  3. openclaw       (41%)            │
│                                      │
│  【我的笔记】                        │
│  2026-04-05: clone 来研究 provider   │
│  抽象... [按 Enter 编辑]             │
│                                      │
│  【待办】                            │
│  □ 对比 zeroclaw 的 memory 模块     │
└─────────────────────────────────────┘
```

---

## 7. 分波次实现计划

### Wave 21：Schema + 画像提取器（1 天）

- [ ] Schema v16：`repo_profiles` + `repo_relationships` + `repo_notes`
- [ ] `src/repo_analyzer/` 模块：README/Cargo.toml/package.json 解析器
- [ ] `devbase profile` CLI：手动触发单个仓库画像
- [ ] `scan` 流程自动触发画像生成

**验收**：`devbase profile clarity` 输出 JSON 画像，包含 description/topics/tech_stack/dependencies。

### Wave 22：相似度计算 + `similar` / `stack` CLI（1 天）

- [ ] `repo_embeddings` 表 + `tools/embedding-provider/repos.py` 生成脚本
- [ ] `compute_similarity()` 三维度算法
- [ ] `devbase similar <repo>` CLI
- [ ] `devbase stack <tech>` CLI
- [ ] `scan` 后自动计算全量 repo 关系

**验收**：`devbase similar clarity` 返回排序列表，zeroclaw 排第一。

### Wave 23：`compare` + `why` + 笔记系统（1 天）

- [ ] `repo_notes` CRUD
- [ ] `.devbase/notes.md` 文件同步（双向：文件 ↔ SQLite）
- [ ] `devbase why <repo>` CLI
- [ ] `devbase compare <a> <b>` CLI
- [ ] `devbase note <repo>` 编辑笔记

**验收**：可以给 zeroclaw 写笔记，`devbase why zeroclaw` 显示笔记内容。

### Wave 24：TUI Knowledge Tab + 自然语言查询（1-2 天）

- [ ] DetailTab::Knowledge 渲染
- [ ] TUI 中显示相似仓库、技术栈、笔记
- [ ] `devbase query "rust llm provider"` NL 解析器（规则-based，非 LLM）
- [ ] 集成测试

**验收**：TUI 中选中 clarity → 按 Tab 切到 Knowledge → 看到 zeroclaw 在相似仓库第一位。

---

## 8. 与现有系统的兼容性

| 现有模块 | 关系 |
|---------|------|
| `registry/` | `repo_profiles` 外键关联 `repos.id`， Cascade Delete |
| `semantic_index/` | 复用 embedding 生成脚本，新增 `repos.py` |
| `embedding.rs` | 复用 `cosine_similarity()` 函数 |
| `scan.rs` | `scan` 后自动调用画像生成 + 关系计算 |
| `query.rs` | 扩展 NL 查询支持，底层仍转 SQLite |
| `tui/render/detail.rs` | 新增 DetailTab::Knowledge 分支 |

---

## 9. 风险评估

| 风险 | 缓解 |
|------|------|
| README 解析准确率不足 | 多策略 fallback：取第一个段落 → 取第一个 # 标题下内容 → 取 package.json description |
| 50 个仓库 embedding 生成慢 | 增量更新：只重新生成变更的仓库，复用已有 embedding |
| 关系计算 O(n²) 慢 | 50 个仓库仅需 1225 次对比，毫秒级；未来 >500 时用近似最近邻 |
| 自然语言查询过于简单 | Wave 24 先用规则-based，后续可升级为向量语义匹配 |

---

*文档版本：2026-04-25*  
*基于用户 workspace 真实需求制定*  
*下次 review：Wave 21 完成后*
