# Devbase Project Skill — 项目内工作指南

> 本 skill 仅在 `dev/third_party/devbase` 目录内生效，补充通用 devbase skill 的项目特定细节。

## 1. 项目速览

- **版本**：v0.16.1（单 crate，22.7 KLOC，18 workspace crates 已提取）
- **核心定位**：本地知识库 MCP Server — AI agent 的海马体
- **技术栈**：Rust 2024, SQLite, tokio, tree-sitter, Tantivy
- **构建命令**：`cargo build --release`（release 约 30-40s）
- **测试命令**：`cargo test --workspace`（456 passed / 0 failed / 5 ignored）
- **代码检查**：`cargo clippy --all-targets -D warnings` + `cargo fmt --check`

## 2. 模块结构（Workspace Crates）

```
crates/
├── devbase-core-types/          # 基础类型（RepoEntry, SymbolType 等）
├── devbase-registry-* /         # Registry 子模块（health/metrics/workspace/entity/relation/call-graph/dead-code/code-symbols）
├── devbase-embedding/           # 向量嵌入生成与序列化
├── devbase-skill-runtime-* /    # Skill Runtime（types + parser）
├── devbase-vault-* /            # Vault 解析（frontmatter + wikilink）
├── devbase-workflow-* /         # Workflow 引擎（model + interpolate）
├── devbase-symbol-links/        # 符号相似度链接
├── devbase-sync-protocol/       # Sync 协议类型
└── devbase-syncthing-client/    # Syncthing P2P 客户端
```

## 3. 开发红线（Architecture Guardrails）

修改代码前必读：

| 红线 | 规则 | 检查方式 |
|------|------|----------|
| **RF-1** | 禁止新增全局路径硬编码 | `grep -rn "dirs::data_local_dir" src/ \| grep -v "backup.rs\|migrate.rs\|search.rs"` |
| **RF-2** | 测试必须密封（不修改全局状态） | 用 `tempfile` + 注入路径 |
| **RF-3** | Schema 变更必须同步 `SCHEMA_DDL` 和 `migrate.rs` | `cargo test registry::test_helpers::tests` |
| **RF-4** | `main.rs` ≤ 1000 行 | `wc -l src/main.rs` |
| **RF-5** | 禁止模块间循环依赖 | 手动检查双向 `use crate::` |
| **RF-6** | 生产代码禁止 `unwrap/expect/panic` | `grep -rn "unwrap()\|expect()\|panic!" src/ \| grep -v "#\[cfg(test)\]"` |
| **RF-7** | 新增模块 `crate::` 引用 ≤ 5 才能提取为独立 crate | 统计 `grep -c 'crate::' src/<mod>.rs` |

## 4. Schema 迁移规范

1. **版本号**：`CURRENT_SCHEMA_VERSION` 在 `src/registry/migrate.rs`
2. **新迁移文件**：`src/registry/migrations/v<NN>_<description>.rs`
3. **Idempotent**：使用 `PRAGMA table_info` 检查列是否存在后再 `ALTER TABLE`
4. **同步要求**：
   - `migrate.rs` 的 `init_db_at()` DDL（新 DB）
   - `migrations/mod.rs` 的 runner 块（现有 DB）
   - `registry/test_helpers.rs` 的 `SCHEMA_DDL`（测试 DB）
5. **备份**：迁移前自动调用 `backup::auto_backup_before_migration()`

## 5. MCP Tool 开发规范

新增 MCP tool 的步骤：

1. 在 `src/mcp/tools/` 下创建/修改对应的 tool 文件
2. 在 `src/mcp/mod.rs` 的 `McpToolEnum` 中添加枚举变体
3. 在 `tier()` 方法中指定稳定性（Stable/Beta/Experimental）
4. 在 `build_server_with_tiers()` 中注册
5. **Schema**：`inputSchema` 必须符合 JSON Schema，包含 `description`
6. **幂等性**：状态变更操作必须幂等（`ON CONFLICT ... DO UPDATE`）
7. **审计**：写入操作自动记录 OpLog

## 6. 常见开发任务

### 添加新 CLI 子命令

```rust
// 1. 在 src/commands/<category>.rs 中实现
pub fn cmd_newthing(app: &mut AppContext) -> Result<()> { ... }

// 2. 在 src/main.rs 的 match 分支中注册
"newthing" => commands::newthing::cmd_newthing(&mut app)?,

// 3. 确保不堆积业务逻辑于 main.rs
```

### 测试数据隔离

```rust
// 使用 DEVBASE_DATA_DIR 环境变量覆盖数据目录
std::env::set_var("DEVBASE_DATA_DIR", temp_dir.path());
// 测试结束后清理由测试框架负责
```

### 运行特定测试

```bash
cargo test --lib semantic_index::symbol::tests::test_extract_rust_attributes -- --nocapture
cargo test --workspace  # 全量
cargo test --test cli   # 集成测试
```

## 7. 上下文压缩恢复点

如果 Kimi CLI 会话被压缩，恢复后执行：

```bash
# 1. 确认编译状态
cargo test --workspace 2>&1 | grep "test result"

# 2. 确认 workspace 成员
cargo metadata --format-version 1 | jq '.workspace_members'

# 3. 确认高耦合模块
grep -rc 'crate::' src/*.rs | sort -t: -k2 -n | tail -5
```

## 8. 与 Kimi CLI 的集成

- **MCP Server**：`devbase mcp`（stdio transport）
- **配置位置**：`C:\Users\22414\.kimi\mcp.json`
- **环境变量**：`DEVBASE_MCP_ENABLE_DESTRUCTIVE=1` 启用写操作
- **工具 tier 过滤**：`DEVBASE_MCP_TOOL_TIERS=stable,beta`

在 devbase 目录内启动 Kimi CLI 会话时，MCP server 会自动提供 48 个工具，包括代码分析、知识库管理、工作流执行等能力。
