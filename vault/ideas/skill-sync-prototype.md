---
id: skill-sync-prototype
tags: [clarity, skill, vault, automation]
ai_context: true
created: 2026-04-23
---

# Vault -> Skill 同步原型设计

## 背景

Clarity 的 Skill 系统是 Markdown + YAML frontmatter，devbase 的 Vault 笔记也是 Markdown + YAML frontmatter。两者格式天然接近，可以设计自动同步机制。

## 方案 A：devbase 导出 -> Clarity 导入

```bash
devbase skill sync --target clarity --dir ./vault/references/
```

- devbase 扫描 Vault 中 `ai_context: true` 的笔记
- 转换为 Clarity SKILL.md 格式
- 写入 `C:\Users\22414\Desktop\clarity\skills\`

## 方案 B：Clarity 直接读取 Vault

- Clarity Agent 启动时通过 `devkit_vault_search` 查询相关笔记
- 动态注入到 system prompt，不落地为 SKILL.md 文件
- 优势：实时同步，无需导出步骤

## 待决策

1. Vault frontmatter 中哪些字段对 Clarity 有意义？
   - `repo:` -> Skill 的关联项目上下文
   - `tags:` -> Skill 的分类标签
   - `ai_context:` -> 是否纳入 AI 上下文
2. 笔记 body 中是否支持 Clarity 的指令语法（如 `@tool_name`）？
