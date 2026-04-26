# devbase v0.10.0 Release Notes

**Release Date**: 2026-04-26  
**Schema Version**: v19  
**Tests**: 288 passed / 0 failed / 3 ignored

---

## Highlights

### L3 Risk Layer MVP — `known_limits`

系统首次具备**自我边界意识**。`known_limits` 表记录 hard vetoes、已知缺陷和外部依赖风险，并在 Skill 执行前自动审计。

```bash
# 查看当前系统约束
devbase limit list

# 解决一个已知限制（自动记录到 L4 元认知层）
devbase limit resolve my-limit --reason "已验证安全"

# 从 AGENTS.md 自动填充 hard vetoes
devbase limit seed
```

### L4 元认知层 MVP — `knowledge_meta`

记录人类对 L1-L3 的纠正和反馈，形成**认知纠错闭环**。

```bash
# resolve 时带 --reason 会自动创建 L4 记录
devbase limit resolve hard-veto-xxx --reason "经人工复核，该限制在容器环境中可豁免"
```

### Hard Veto 运行时守卫

Skill 执行前自动检查未解决的 hard veto。发现时：**不阻止执行**，但在 `stderr` 头部注入 `[HARD-VETO-WARNING]` 警告，同时写入 OpLog 审计。

```
[HARD-VETO-WARNING] Skill 'embed-repo' executed with 5 unresolved hard veto(s):
- [hard-veto-xxx] 禁止闭源 / 云端强制 / 数据外泄
- [hard-veto-yyy] 禁止 Docker / RAG(Qdrant) / GUI(Electron)
...
```

---

## Schema Changes

| 版本 | 表 | 说明 |
|:---|:---|:---|
| v18 | `known_limits` | L3 风险层：id, category, description, source, severity, first_seen_at, last_checked_at, mitigated |
| v19 | `knowledge_meta` | L4 元认知层：id, target_level, target_id, correction_type, correction_json, confidence, created_at |

---

## New MCP Tools

| Tool | Tier | 说明 |
|:---|:---|:---|
| `devkit_known_limit_store` | Beta | 存储/更新 known limit |
| `devkit_known_limit_list` | Beta | 列出 known limits（支持 category/mitigated 过滤） |

MCP tool 总数：37

---

## New CLI Commands

```bash
devbase limit add <id> --category <cat> --description <desc> [--source <src>] [--severity <1-5>]
devbase limit list [--category <cat>] [--mitigated] [--json]
devbase limit resolve <id> [--reason <reason>]
devbase limit delete <id>
devbase limit seed
```

---

## Migration Notes

从 v0.9.0 升级时，registry 会自动备份并迁移到 Schema v19（`knowledge_meta` 表）。无需手动操作。

---

## Known Limitations

- Hard Veto 守卫当前为**警告模式**（不阻止执行），未来可能支持 `--block-on-veto` 配置
- `knowledge_meta` 尚未暴露 MCP tool，仅通过 CLI `limit resolve --reason` 间接写入
- L0-L2 知识层已存在对应表（entities/skills/vault_notes），但尚未显式标记 `knowledge_level`
