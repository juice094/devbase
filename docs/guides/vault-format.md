# devbase Vault 格式规范 v0.1

> **目标**：为 clarity 等 MCP Client 提供标准化的 Vault 笔记消费接口。
> **状态**：草案，基于现有 `vault/examples/` 实践提炼。
> **日期**：2026-04-23

---

## 一、文件格式

Vault 笔记是 **Markdown 文件**，文件名以 `.md` 结尾，存放在 `vault/` 目录及其子目录中。

每个文件分为两部分：
1. **YAML frontmatter**（可选，但推荐）
2. **Markdown body**

```markdown
---
id: mcp-integration-guide
repo: devbase
tags: [mcp, architecture, protocol]
ai_context: true
created: 2026-04-20
updated: 2026-04-23
---

# MCP 集成架构笔记

正文内容...
```

---

## 二、Frontmatter 字段规范

### 2.1 字段总览

| 字段 | 类型 | 必填 | 说明 | AI 消费优先级 |
|------|------|------|------|--------------|
| `id` | string | 推荐 | 笔记唯一标识（kebab-case） | 🔴 高 — Skill 唯一标识 |
| `repo` | string | 可选 | 关联的仓库 ID | 🟡 中 — 项目上下文关联 |
| `tags` | string[] | 可选 | 分类标签 | 🟡 中 — Skill 分类 |
| `ai_context` | boolean | 可选 | 是否纳入 AI 上下文 | 🔴 高 — 过滤条件 |
| `created` | date | 可选 | 创建日期 | 🟢 低 — 时间线索 |
| `updated` | date | 可选 | 更新日期 | 🟢 低 — 新鲜度判断 |

### 2.2 字段详解

#### `id`

- **格式**：kebab-case，如 `mcp-integration-guide`
- **唯一性**：在同一个 devbase workspace 内唯一
- **用途**：
  - 作为 `devkit_vault_read` 的查询键
  - 作为 clarity Skill 的唯一标识
  - 作为 wikilink 的目标（`[[id]]` 或 `[[id|显示文本]]`）
- **生成建议**：基于文件路径或标题自动推导
  - 文件 `references/mcp-integration.md` → `mcp-integration-guide`
  - 标题 "MCP 集成架构笔记" → `mcp-integration-architecture`

#### `repo`

- **格式**：devbase 注册表中的仓库 ID
- **用途**：
  - 将该笔记与特定项目关联
  - `devkit_project_context` 会返回该笔记作为 "link" 来源
  - clarity 侧可将该笔记作为项目的 Skill 上下文
- **示例**：`repo: devbase` 表示该笔记是 devbase 项目的参考文档

#### `tags`

- **格式**：YAML 字符串数组
- **分层命名空间**：支持 `domain:subcategory:item` 格式
  - `agri:crop:rice` — 农业领域 > 作物 > 水稻
  - `mcp:protocol` — MCP 领域 > 协议层
- **用途**：
  - `devkit_vault_search` 可按标签过滤
  - clarity 侧可按标签组织 Skill 分类

#### `ai_context`

- **格式**：boolean，默认 `false`
- **用途**：
  - `true` → 该笔记可被 AI 读取并纳入上下文
  - `false` → 该笔记仅作为人类参考，AI 不应主动读取
- **建议**：
  - 技术文档、架构决策、项目笔记 → `true`
  - 个人日记、草稿、临时记录 → `false`

#### `created` / `updated`

- **格式**：ISO 8601 日期（`YYYY-MM-DD`）
- **用途**：时间线索、新鲜度判断

---

## 三、Body 格式

### 3.1 Markdown 标准

- 遵循 CommonMark 规范
- 支持 wikilink：`[[note-id]]` 或 `[[note-id|显示文本]]`
- 支持代码块、表格、列表等标准 Markdown 元素

### 3.2 Wikilink 规范

Wikilink 是 Vault 笔记之间的关联机制。

```markdown
- 参考 [[mcp-integration-guide]] 了解协议细节
- 查看 [[skill-sync-prototype|Skill 同步原型]] 获取设计思路
```

**解析规则**：
1. `[[id]]` → 链接到 `id` 对应的笔记
2. `[[id|text]]` → 链接到 `id`，显示文本为 `text`
3. 如果 `id` 不存在于注册表，标记为 "broken link"

**反向链接（backlinks）**：
- `devkit_vault_backlinks` 返回所有包含指向某笔记的 wikilink 的笔记列表

---

## 四、Clarity 侧消费指南

### 4.1 消费流程

```
1. devbase 侧：扫描 vault/ 目录，解析 frontmatter，写入 SQLite
2. clarity 侧：通过 devkit_vault_search 查询相关笔记
3. clarity 侧：通过 devkit_vault_read 读取笔记内容
4. clarity 侧：将笔记内容注入 system prompt 作为 Skill 上下文
```

### 4.2 消费字段映射

| devbase Vault 字段 | clarity Skill 字段 | 映射规则 |
|-------------------|-------------------|---------|
| `id` | `meta.id` | 直接映射 |
| `title`（从文件名或第一个 H1 提取） | `meta.name` | 文件名去掉 `.md` 或第一个 `# ` 标题 |
| `tags` | `meta.tags` | 直接映射 |
| `repo` | `meta.tools` 前缀过滤 | 与 `devkit_project_context` 配合 |
| `ai_context` | 是否纳入上下文 | `true` 才纳入，`false` 跳过 |
| `body` | `body` | Markdown 正文作为 Skill 指令内容 |

### 4.3 最小可用子集

如果 clarity 侧不想实现全部字段，最小子集为：

```yaml
---
id: unique-note-id      # 必须 — 用于读取和引用
ai_context: true        # 必须 — 决定是否纳入 AI 上下文
repo: project-id        # 推荐 — 项目关联
tags: [category]        # 可选 — 分类
---
```

---

## 五、验证规则

### 5.1 Frontmatter 语法验证

| 规则 | 错误示例 | 正确示例 |
|------|---------|---------|
| `id` 只能含小写字母、数字、连字符 | `MCP_Guide` | `mcp-guide` |
| `ai_context` 必须是布尔值 | `ai_context: yes` | `ai_context: true` |
| `tags` 必须是数组 | `tags: mcp` | `tags: [mcp]` |
| `created`/`updated` 必须是日期 | `created: today` | `created: 2026-04-23` |

### 5.2 完整性检查

- `id` 缺失 → 警告（使用文件名作为 fallback id）
- `ai_context` 缺失 → 默认 `false`（保守策略）
- `repo` 指向不存在的仓库 → 警告但不阻塞

---

## 六、示例

### 示例 A：项目参考笔记（完整字段）

```markdown
---
id: mcp-integration-guide
repo: devbase
tags: [mcp, architecture, protocol]
ai_context: true
created: 2026-04-20
updated: 2026-04-23
---

# MCP 集成架构笔记

## 当前架构

devbase 作为 MCP Server...
```

### 示例 B：创意/原型笔记（最小字段）

```markdown
---
id: skill-sync-prototype
tags: [clarity, skill, vault, automation]
ai_context: true
created: 2026-04-23
---

# Vault -> Skill 同步原型设计

## 背景
...
```

### 示例 C：个人笔记（不纳入 AI 上下文）

```markdown
---
id: daily-thoughts-0423
tags: [personal]
ai_context: false
created: 2026-04-23
---

# 今日想法

一些临时记录...
```

---

## 七、版本历史

| 版本 | 日期 | 变更 |
|------|------|------|
| v0.1 | 2026-04-23 | 初始草案，基于 vault/examples/ 实践 |

---

## 八、待决策事项

| # | 事项 | 状态 | 备注 |
|---|------|------|------|
| 1 | `title` 是否作为独立 frontmatter 字段？ | 待讨论 | 当前从文件名或 H1 提取 |
| 2 | `version` 字段是否需要（用于 Skill 版本管理）？ | 待讨论 | clarity SkillMeta 有 version |
| 3 | `author` 字段是否需要？ | 待讨论 | 当前笔记无作者概念 |
| 4 | 支持多 `repo` 关联（数组）？ | 待讨论 | 当前仅支持单 repo |
