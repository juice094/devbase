# devbase 阶段性报告

**日期**: 2026-04-10  
**版本**: 0.1.0-beta  
**提交人**: Kimi Code CLI

---

## 一、本次完成的核心功能

### 1. 参考仓库迁移与路径统一
- 将 `Desktop/devbase_refs` 的 9 个仓库迁移至 `dev/third_party`，跳过重复项 `iroh`，删除空目录。
- 更新了 devbase SQLite registry 中的 10 条 `local_path` 记录，全部指向新路径。
- 生成仓库参考报告：`docs/referenced_repos_report.md`。

### 2. 学术资产管理（papers + experiments）
- `registry.rs`: 新增 `papers` 和 `experiments` 表，含外键约束与 CRUD 操作。
- `mcp.rs`: 新增 `devkit_paper_index` 和 `devkit_experiment_log` 两个 MCP 工具（工具总数从 7 增至 9）。
- `query.rs`: 扩展查询语法，支持 `paper:venue:<name>` 和 `experiment:repo:<id>`。

### 3. 语义摘要回退机制
- `knowledge_engine.rs`: 当 README 缺失或 Ollama 不可用时，自动读取 `Cargo.toml` / `package.json` / `pyproject.toml` / `go.mod` 提取描述信息作为 fallback summary。
- 已修复 TOML fixture 中的转义引号问题，测试通过。

### 4. GitHub 集成（本轮最新完成）
- `config.rs`: 新增 `github.token: Option<String>` 配置字段。
- `mcp.rs`: 新增第 10 个 MCP 工具 `devkit_github_info`：
  - 支持从 `origin` remote URL 解析 `owner/repo`
  - 调用 GitHub API 获取 stars / forks / description / language / open_issues / updated_at
  - 若配置了 `github.token`，自动附加 `Authorization: Bearer <token>`
  - 支持 `write_summary=true` 将 description 写入 `repo_summaries` 表
- 已通过端到端测试验证（以 `codex` 仓库为例）。

---

## 二、测试状态

- `cargo test`: **26 passed, 2 ignored**（全绿）
- `devkit_github_info` 端到端: **通过**（stdio MCP 调用成功，数据正确写入 summary）

---

## 三、产生的附加文档

- `docs/referenced_repos_report.md` — 27 个 GitHub 参考仓库的验证报告
- `docs/competitive_analysis_plan.md` — 基于 `third_party` 的竞品分析与业务能力增强规划

---

## 四、待测试 / 待完成事项

### 短期（1–2 天）
1. [ ] `devkit_github_info` 对 SSH 格式 `git@github.com:owner/repo.git` 的解析边界测试
2. [ ] GitHub API 速率限制下的错误处理与降级行为测试
3. [ ] `devkit_experiment_log` 批量写入与 `repo_tags` 并发更新测试
4. [ ] `query` 引擎对 `paper:venue:` 和 `experiment:repo:` 的组合条件查询测试

### 中期（1 周内）
5. [ ] 将 MCP Server 迁移到官方 `rmcp` SDK，获得标准 capability 声明与 HTTP transport
6. [ ] 引入 `sqlite-vec` 做语义检索 PoC
7. [ ] 建立 README / 源码分块 → embedding → 索引的 Pipeline

### 长期（视优先级）
8. [ ] 与 `clarity` 的 MCP Client 做互联互通验证
9. [ ] 基于 `iroh` 的 P2P 发现层技术预研

---

## 五、已知问题

- 存在若干 `dead_code` / `unused` 编译器警告（非阻塞）
- `FolderScheduler::new` 当前未使用，但保留作为后续 watch 功能扩展接口
