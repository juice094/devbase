# CLI 参考

> 完整命令列表。所有命令均支持 `--help` 查看详细参数。

---

## 仓库管理

### `devbase scan <path> [--register]`

扫描目录下的 Git 仓库。

```bash
devbase scan . --register        # 扫描并注册
devbase scan ~/projects          # 仅扫描，不注册
```

### `devbase health [--detail] [--limit <n>] [--page <n>]`

检查已注册仓库的健康状态（dirty/behind/ahead/diverged）。

```bash
devbase health --detail           # 显示每个仓库的详细状态
devbase health --limit 10         # 分页显示
```

### `devbase sync [--dry-run] [--filter-tags <tags>] [--exclude <ids>] [--json]`

同步仓库与上游远程分支。

```bash
devbase sync --dry-run            # 预览同步内容
devbase sync --filter-tags rust   # 仅同步 tag 含 rust 的仓库
```

### `devbase index [path]`

索引仓库摘要、模块结构、代码符号。

```bash
devbase index                     # 索引所有已注册仓库
devbase index ./my-project        # 索引特定路径
```

### `devbase query <expression> [--limit <n>] [--page <n>]`

查询知识库。支持 `lang:`、`stale:` 等过滤语法。

```bash
devbase query "lang:rust stale:>30"
devbase query "tag:ai"
```

### `devbase tag <repo_id> <tags>`

为仓库打标签（逗号分隔）。

```bash
devbase tag devbase "rust,cli,ai"
```

### `devbase meta <repo_id> [--tier <tier>] [--workspace-type <type>]`

更新仓库元数据。

```bash
devbase meta devbase --tier private --workspace-type git
```

### `devbase clean`

清理注册表中的归档/备份条目。

---

## Vault 笔记

### `devbase vault scan [path]`

扫描 Markdown 笔记并同步到 Vault。

```bash
devbase vault scan                # 扫描默认 vault 目录
devbase vault scan ./notes        # 扫描指定目录
```

### `devbase vault list [--tag <tag>]`

列出所有 Vault 笔记。

```bash
devbase vault list                # 列出全部
devbase vault list --tag meta     # 按标签过滤
```

### `devbase vault read <path>`

读取指定笔记的完整内容。

```bash
devbase vault read "99-Meta/todo.md"
```

### `devbase vault write <path> [--content <text>] [--title <title>]`

写入或覆盖笔记。`--content -` 从 stdin 读取。

```bash
devbase vault write "01-Projects/idea.md" --content "我的新想法" --title "Idea"
echo "笔记内容" | devbase vault write "00-Inbox/dump.md" --content -
```

### `devbase vault reindex`

重建 Vault 的 Tantivy 搜索索引。

---

## Skill 运行时

### `devbase skill list [--skill-type <type>] [--category <cat>] [--json]`

列出已安装的 Skill。

### `devbase skill install <source> [--git]`

从本地路径或 Git URL 安装 Skill。

```bash
devbase skill install ./my-skill
devbase skill install https://github.com/user/skill --git
```

### `devbase skill run <skill_id> [--arg key=value]... [--timeout <s>] [--json]`

执行指定 Skill。

```bash
devbase skill run hello-world --arg name=Alice --timeout 60
```

### `devbase skill discover <path> [--skill-id <id>] [--dry-run] [--json]`

将项目自动封装为 Skill。

```bash
devbase skill discover . --dry-run    # 预览生成的 Skill 文件
```

### `devbase skill search <query> [--semantic] [--limit <n>]`

搜索 Skill。

### `devbase skill top [--limit <n>]`

显示评分最高的 Skill。

---

## 工作流

### `devbase workflow list`

列出已注册的工作流。

### `devbase workflow show <workflow_id>`

显示工作流定义。

### `devbase workflow register <path>`

从 YAML 文件注册工作流。

### `devbase workflow run <workflow_id> [--input key=value]...`

执行工作流。

### `devbase workflow delete <workflow_id>`

删除工作流。

---

## 运维与诊断

### `devbase oplog [--limit <n>] [--repo <id>]`

查看操作日志。

```bash
devbase oplog --limit 5
devbase oplog --repo devbase
```

### `devbase digest`

生成每日知识摘要。

### `devbase registry export [--format sqlite|json] [--output <path>]`

导出注册表备份。

### `devbase registry import <path> [--yes]`

从备份导入注册表。

### `devbase registry backups`

列出现有备份。

### `devbase registry clean`

清理旧备份。

---

## MCP 服务器

### `devbase mcp [--tools <tiers>]`

启动 MCP 服务器（stdio 传输）。

```bash
devbase mcp --tools stable,beta     # 仅暴露 Stable 和 Beta 工具
```

---

## 其他

| 命令 | 说明 |
|------|------|
| `devbase tui` | 启动交互式终端仪表盘（需 `--features tui`） |
| `devbase daemon [--interval <s>]` | 启动后台维护守护进程 |
| `devbase watch [path] [--duration <s>]` | 监视目录变化 |
| `devbase discover` | 自动发现仓库间关系 |
| `devbase skill-sync [--output <dir>] [--filter-tags <tags>] [--dry-run]` | 同步 Vault 笔记到 Clarity SKILL.md |
| `devbase syncthing-push [--api-url <url>] [--api-key <key>] [--filter-tags <tags>]` | 推送仓库到 Syncthing |
| `devbase limit add <id> [--category <cat>] [--description <text>]` | 添加已知限制 |
| `devbase limit list [--category <cat>] [--mitigated]` | 列出已知限制 |
| `devbase limit resolve <id> [--reason <text>]` | 解决已知限制 |
| `devbase limit delete <id>` | 删除已知限制 |
| `devbase limit seed` | 从 AGENTS.md 导入 Hard Veto |
