# devbase 本质定位：AI 基础设施（智能知识库）

> 核心洞察：AI 无法识别 GUI，devbase 不是 Git TUI，而是**AI 可访问的开发者知识库**
> 竞争赛道：AI Infrastructure（智能数据库）≠ Git Client

---

## 一、为什么之前的分析错了

### 旧框架的错误假设

```
错误框架：devbase 是 "多仓库 Git TUI"
          → 竞品是 lazygit/gitui/desktop
          → 竞争维度是"Git 操作体验"

正确框架：devbase 是 "AI 可访问的代码库知识库"
          → 竞品是 5ire/dify/coze/OpenHands
          → 竞争维度是"AI 对代码库的理解深度"
```

### AI 无法识别 GUI 的本质

| 工具类型 | 人类可用 | AI 可用 | 原因 |
|---------|---------|--------|------|
| GUI (desktop, 5ire GUI) | ✅ | ❌ | AI 无法"看"屏幕像素，无法点击按钮 |
| TUI (lazygit, gitui) | ✅ | ⚠️ 困难 | AI 可以读终端输出，但解析文本布局极不可靠 |
| CLI 文本输出 (`git status`) | ✅ | ⚠️ 困难 | 需要 parsing，格式变化就失效 |
| **结构化 API (MCP/JSON/SQL)** | ❌ 人类不直接读 | ✅ **AI 原生** | 机器可读、schema 稳定、语义明确 |

**关键结论**：
- lazygit/gitui/desktop 是**人类工具**，AI 用不了
- devbase 的 MCP Server + SQLite 注册表是**AI 工具**，人类通过 TUI 间接使用
- **devbase 的竞争不在 Git TUI 赛道，在 AI 基础设施赛道**

---

## 二、新定位：devbase 是什么

```
┌─────────────────────────────────────────────────────────────┐
│                     AI Agent (Claude/Codex/5ire)            │
│                          │                                  │
│                          ▼                                  │
│              ┌─────────────────────┐                        │
│              │    MCP Protocol     │                        │
│              │  (tools/resources)  │                        │
│              └─────────────────────┘                        │
│                          │                                  │
│              ┌───────────┴───────────┐                     │
│              ▼                       ▼                     │
│    ┌─────────────────┐    ┌─────────────────┐              │
│    │  devbase Server │    │  其他 MCP Server │              │
│    │                 │    │  (filesystem)   │              │
│    │ • list_repos    │    │                 │              │
│    │ • repo_health   │    │                 │              │
│    │ • safe_sync     │    │                 │              │
│    │ • query_kb      │    │                 │              │
│    └─────────────────┘    └─────────────────┘              │
│              │                                              │
│              ▼                                              │
│    ┌─────────────────────────────────────┐                  │
│    │         devbase Core                  │                  │
│    │  ┌──────────┐  ┌──────────┐          │                  │
│    │  │ SQLite   │  │ Git      │          │                  │
│    │  │ Registry │  │ Scanner  │          │                  │
│    │  │          │  │          │          │                  │
│    │  │ repos    │  │ status   │          │                  │
│    │  │ tags     │  │ sync     │          │                  │
│    │  │ health   │  │ history  │          │                  │
│    │  │ stars    │  │          │          │                  │
│    │  │ summaries│  │          │          │                  │
│    │  └──────────┘  └──────────┘          │                  │
│    └─────────────────────────────────────┘                  │
│              │                                              │
│              ▼                                              │
│    ┌─────────────────────────────────────┐                  │
│    │         Human Interface (TUI)       │                  │
│    │  ratatui dashboard for humans       │                  │
│    └─────────────────────────────────────┘                  │
└─────────────────────────────────────────────────────────────┘
```

**devbase 的三层架构**：
1. **AI 层**：MCP Server — 给 AI 提供结构化工具
2. **数据层**：SQLite 注册表 + Git 扫描器 — 持久化知识
3. **人类层**：TUI — 给人类看的仪表盘

**TUI 不是核心，是副产品。** 真正的核心是让 AI 能：
- "查询我本地有哪些 Rust 项目超过 30 天没同步"
- "扫描当前目录下的所有仓库，生成健康报告"
- "找出所有带 'production' 标签且 status dirty 的仓库"
- "对比两个仓库的 stars 趋势"

---

## 三、真正的竞品：AI 基础设施层

### 竞品全景重分类

```
AI 基础设施赛道（devbase 的真正竞品）：
│
├── 🤖 AI Agent + 知识库
│   ├── 5ire              — 本地知识库(bge-m3) + MCP Client + Electron GUI
│   ├── claude-code-rust  — AI 编码助手 + Skill 系统 + TUI/GUI
│   ├── codex             — OpenAI Codex CLI
│   ├── OpenHands         — AI 软件开发智能体
│   ├── openclaw          — 个人 AI 助手 + 工具调用
│   └── zeroclaw          — 个人 AI 助手
│
├── 🏗️ LLM 应用平台
│   ├── dify              — LLM 应用开发平台
│   └── coze-studio       — AI Bot 开发平台
│
└── 🔌 MCP 生态
    └── rust-sdk (rmcp)   — MCP Rust SDK（基础设施，非竞品）

非竞品（人类工具，AI 用不了）：
├── lazygit, gitui, desktop, gws    — Git 客户端
├── syncthing, syncthing-rust       — 文件同步（友军）
├── gitoxide                        — Git 实现库
├── ollama, candle, burn, vllm      — LLM/ML 运行时
└── ratatui                         — TUI 框架
```

---

### 3.1 核心竞品对比：AI 知识库维度

| 维度 | **devbase** | **5ire** | **claude-code-rust** | **codex** | **OpenHands** | **dify** |
|:-----|:-----------:|:--------:|:--------------------:|:---------:|:-------------:|:--------:|
| **定位** | 代码库知识库 | AI 助手+知识库 | AI 编码助手 | AI CLI 编码 | AI 软件工程师 | LLM 应用平台 |
| **知识类型** | **代码库原生**（Git状态、模块结构、语言、同步历史） | 通用文档（上传文件、网页、笔记） | Skill 系统（预定义模板） | 无 | 任务轨迹 | Prompt+工作流 |
| **数据结构** | **SQLite 结构化** | 向量数据库 | 文件系统 | 无 | 文件系统 | 数据库 |
| **MCP 角色** | ✅ **Server**（提供工具） | ✅ Client | ❌ | ❌ | ❌ | ❌ |
| **AI 可查询** | ✅ SQL + MCP tools | ⚠️ RAG 检索 | ⚠️ Skill 匹配 | ❌ | ⚠️ 任务历史 | ⚠️ API 调用 |
| **代码库理解** | ✅ 深度（Git 图结构、 ahead/behind、dirty 语义） | ❌ 浅层（文本相似度） | ⚠️ 中等（文件内容） | ⚠️ 中等 | ⚠️ 中等 | ❌ |
| **人类界面** | TUI | Electron GUI | TUI+GUI+CLI | CLI | Web | Web |
| **本地优先** | ✅ 完全本地 | ✅ 可选本地模型 | ⚠️ 需 API Key | ⚠️ OpenAI API | ⚠️ 云端运行 | ❌ 云端 |

---

### 3.2 关键差异化：devbase 的不可替代性

#### 5ire 的弱点（最危险竞品）

```
5ire 的知识库：
  用户上传 README.md → 文本切片 → bge-m3 嵌入 → 向量检索
  
  AI 问："我本地有哪些项目有未推送提交？"
  5ire 的回答：❌ "根据您上传的文档，我找不到相关信息"
  
5ire 的问题：
  1. 知识是"静态文档"，不是"动态状态"
  2. 不知道 Git 图结构（ahead/behind/diverged）
  3. 不知道仓库之间的拓扑关系
  4. 没有同步策略概念
```

```
devbase 的知识库：
  Git 扫描 → SQLite 注册表 → MCP Server
  
  AI 问："我本地有哪些项目有未推送提交？"
  devbase 的回答：✅ 
    {
      "repos": [
        {"name": "devbase", "ahead": 3, "behind": 0, "branch": "main"},
        {"name": "syncthing-rust", "ahead": 1, "behind": 2, "branch": "develop"}
      ],
      "sync_policy": "Conservative",
      "recommendation": "devbase 可安全推送，syncthing-rust 需先 fetch"
    }
```

#### claude-code-rust 的弱点

```
claude-code-rust 的 Skill 系统：
  - 预定义的代码模板和操作序列
  - 比如 "创建 React 组件" = 生成文件 + 写代码
  
  问题：
  1. Skill 是"怎么做"，不是"现状是什么"
  2. 没有持久化的仓库元数据注册表
  3. 每次启动都要重新扫描/理解项目结构
  
devbase 的优势：
  - 仓库状态是持久化的、结构化的、可查询的
  - AI 不需要重新"理解"项目，直接查注册表
```

#### codex/OpenHands 的弱点

```
codex：纯粹的编码助手，没有本地知识库概念
OpenHands：云端运行，无法访问本地文件系统（除非挂载）

devbase 的优势：
  - 完全本地，零网络依赖
  - 本地 SQLite 注册表 = AI 的"本地记忆"
```

---

## 四、新竞争路线：AI 基础设施赛道

### 4.1 赛道重新定义

```
旧赛道：Git TUI（lazygit 是 king，不可战胜）
新赛道：AI 可访问的代码库知识库（devbase 是唯一玩家）

旧路线：和 lazygit 抢人类用户
新路线：让所有 AI 助手都通过 devbase 理解本地代码库
```

### 4.2 竞品的 AI 可用性评估

| 工具 | 人类可用 | AI 可用性评分 | 原因 |
|------|---------|-------------|------|
| **devbase** | ✅ TUI | ⭐⭐⭐⭐⭐ | MCP Server + SQLite，AI 原生 |
| **5ire** | ✅ GUI | ⭐⭐⭐ | MCP Client，但知识库是文档型而非代码库型 |
| **claude-code-rust** | ✅ TUI/GUI | ⭐⭐⭐ | Skill 系统，但无持久化注册表 |
| **codex** | ✅ CLI | ⭐⭐ | 无知识库，每次从头理解项目 |
| **OpenHands** | ✅ Web | ⭐⭐ | 云端运行，本地代码库访问受限 |
| **lazygit** | ✅ TUI | ⭐ | 文本输出，AI 难以可靠解析 |
| **gitui** | ✅ TUI | ⭐ | 同上 |
| **desktop** | ✅ GUI | ❌ | AI 完全不可访问 |

**devbase 在"AI 可用性"维度是独一档。**

---

## 五、新战略：成为 AI 的"代码库操作系统"

### 5.1 愿景

> "Every AI assistant that touches code should go through devbase first."
>
> 每个接触代码的 AI 助手，都应该先通过 devbase 了解代码库。

### 5.2 新蚕食路线（AI 基础设施视角）

```
Round 1: 定义标准（现在）
  └─ devbase 成为第一个"代码库 MCP Server"标准
  
Round 2: 接入生态（3-6个月）
  ├─ Claude Code 通过 MCP 调用 devbase
  ├─ 5ire 通过 MCP 调用 devbase
  ├─ Codex CLI 通过 MCP 调用 devbase
  └─ Kimi CLI 通过 MCP 调用 devbase
  
Round 3: 知识深度（6-12个月）
  ├─ 代码语义分析（AST 解析、依赖图）
  ├─ 变更影响分析（改了这个文件会影响哪些仓库？）
  └─ 智能同步建议（AI 判断何时 sync、用哪个 policy）
  
Round 4: 生态锁定（1-2年）
  └─ AI 助手默认集成 devbase，成为"代码库理解"的事实标准
```

### 5.3 与旧竞品的正确关系

| 旧竞品 | 新定位 | 关系 |
|--------|--------|------|
| lazygit | 人类 Git TUI | **互补** — devbase 给 AI 用，lazygit 给人类用 |
| gitui | 人类 Git TUI | **互补** — 同上 |
| desktop | 人类 Git GUI | **无关** — AI 用不了，人类用 desktop 不影响 devbase |
| 5ire | AI 助手平台 | **竞合** — 5ire 是 MCP Client，devbase 是 MCP Server，天然互补 |
| claude-code-rust | AI 编码助手 | **上下游** — Claude Code 调用 devbase 获取代码库上下文 |
| syncthing-rust | 友军 P2P 同步 | **友军** — 代码层同步（devbase）+ 文件层同步（syncthing-rust） |

---

## 六、产品演进建议（AI 基础设施方向）

### 6.1 立即做（AI 原生能力）

1. **强化 MCP Server 的 tool 设计**
   - 当前 tool 偏运维（health、sync），需要增加**知识查询类 tool**：
     - `query_repos_by_language("rust")` — 按语言筛选
     - `query_repos_by_tag("production")` — 按标签筛选
     - `compare_repo_health(repo_a, repo_b)` — 对比健康度
     - `get_repo_module_graph(repo_id)` — 获取模块依赖图

2. **AI 友好的输出格式**
   - MCP tool 的返回应该是**结构化 JSON**，不是人类可读的文本
   - 当前 `devbase repos` 输出人类表格，AI 需要 JSON：
     ```json
     {
       "repos": [
         {
           "id": "devbase",
           "language": "rust",
           "status": {"dirty": false, "ahead": 3, "behind": 0},
           "health": {"last_sync": "2026-04-10", "stale_days": 5},
           "stars": 42
         }
       ]
     }
     ```

3. **注册表 Schema 扩展**
   - 增加 `code_metrics` 表：代码行数、测试覆盖率、依赖数量
   - 增加 `module_graph` 表：模块依赖关系（供 AI 理解架构）
   - 增加 `change_history` 表：每次 sync 的变更摘要

### 6.2 短期做（知识深度）

1. **代码语义索引**
   - 集成 `tree-sitter` 解析代码结构
   - 提取函数/结构体定义，存入 SQLite
   - AI 可以问："哪个仓库里有 `fn fetch_github_stars` 函数？"

2. **跨仓库依赖分析**
   - 解析 `Cargo.toml`、`package.json`、`go.mod`
   - 构建"本地仓库依赖图"
   - AI 可以问："如果改 syncthing-rust 的协议层，哪些友军项目受影响？"

3. **变更影响预测**
   - sync 前分析：这次 pull 会引入哪些文件的变更？
   - AI 可以问："sync devbase 会不会冲突？"

### 6.3 长期做（生态位）

1. **AI 决策代理**
   - devbase 内置轻量决策逻辑：
     - "所有 behind > 0 的 repo 自动 fetch"
     - "dirty repo 不 sync，但通知 AI"
   - AI 可以委托 devbase 执行自动化运维

2. **多 AI 助手协调**
   - devbase 作为中央注册表，多个 AI 助手共享代码库知识
   - Claude Code 和 5ire 看到同一个 devbase 状态

---

## 七、关键认知转变

| 维度 | 旧认知 | 新认知 |
|------|--------|--------|
| **核心用户** | 人类开发者 | **AI Agent + 人类开发者** |
| **核心价值** | TUI 仪表盘 | **结构化代码库知识** |
| **竞争赛道** | Git TUI（lazygit 地盘） | **AI 基础设施（空白市场）** |
| **成功标准** | 人类用户量 | **AI 调用量 + 人类用户量** |
| **产品形态** | TUI 工具 | **MCP Server（TUI 是可选 UI）** |
| **与 lazygit 关系** | 竞争 | **互补**（不同用户：AI vs 人类） |
| **与 5ire 关系** | 竞争 | **互补**（Client vs Server） |

---

*文档结束*
