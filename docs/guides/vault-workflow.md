# Vault 工作流：PARA 实践指南

> PARA = Projects + Areas + Resources + Archives  
> devbase Vault 默认采用 PARA 目录结构管理 Markdown 笔记。

---

## 目录结构

Vault 根目录位于 `%LOCALAPPDATA%\devbase\workspace\vault\`（Windows）或 `~/.local/share/devbase/workspace/vault/`（Linux/macOS）。

```
vault/
├── 00-Inbox/          ← 临时入口，每日清空
├── 01-Projects/       ← 有明确截止目标的独立项目
├── 02-Areas/          ← 持续维护的责任领域（无明确截止）
├── 03-Resources/      ← 参考材料、知识库
├── 04-Archives/       ← 已完成或暂停的项目/领域
└── 99-Meta/           ← 关于 Vault 本身的元笔记
```

`devbase vault scan` 会自动识别这 6 个目录下的所有 `.md` 文件。

---

## 笔记格式

每篇笔记是 Markdown 文件，顶部可选 YAML frontmatter：

```markdown
---
id: idea-2026-04-30
title: 新的索引策略
repo: devbase
tags: [architecture, indexing]
ai_context: true
created: 2026-04-30
updated: 2026-04-30
---

# 新的索引策略

## 背景

当前 `save_modules` 写入 `repo_modules_legacy`...

## 决策

改为写入 `repo_modules`（新 schema），v23 删除 legacy 表。
```

### Frontmatter 字段说明

| 字段 | 必填 | 说明 |
|------|------|------|
| `id` | 否 | 唯一标识，默认使用文件相对路径 |
| `title` | 否 | 标题，默认使用第一个 H1 |
| `repo` | 否 | 关联的仓库 ID，用于 `project_context` 聚合 |
| `tags` | 否 | 标签数组，支持 `devbase vault list --tag` 过滤 |
| `ai_context` | 否 | `true` 时笔记内容会被纳入 `project_context` 返回 |
| `created` | 否 | 创建日期 |
| `updated` | 否 | 更新日期 |

---

## 工作流示例

### 每日工作流

```bash
# 1. 清空 Inbox（将临时笔记分类到 Projects/Areas/Resources）
devbase vault list --tag inbox

# 2. 读取今日待办
devbase vault read "01-Projects/devbase/todo.md"

# 3. 记录新发现
devbase vault write "00-Inbox/fk-error-fix.md" \
  --content "index FK 修复方案..." \
  --title "Index FK Fix"

# 4. 重建搜索索引（如有大量变更）
devbase vault reindex
```

### 项目归档

项目完成后，将整个项目目录从 `01-Projects/` 移动到 `04-Archives/`：

```bash
# PowerShell
Move-Item "$env:LOCALAPPDATA\devbase\workspace\vault\01-Projects\old-project" `
  "$env:LOCALAPPDATA\devbase\workspace\vault\04-Archives\"

# 重新扫描
devbase vault scan
```

---

## 与 AI 的协同

Vault 笔记是跨会话记忆的核心载体：

1. **关键决策写入 Vault**：重要技术决策、架构选择、风险分析
2. **AI 通过 `project_context` 召回**：`project_context` 会自动聚合与项目关联的 Vault 笔记
3. **通过 `vault_search` 主动查询**：AI 可以关键词搜索相关笔记

最佳实践：在笔记中使用 `[[WikiLink]]` 格式建立笔记间链接，devbase 会自动提取 outgoing links 存入 `entities.metadata`。
