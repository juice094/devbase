# devbase 推进路线图 2026

> 基于统一分析（双模态：Human TUI + AI MCP）的阶段性执行计划
> 更新日期：2026-04-23
> 当前版本：commit `2fc7872` (v0.2.3)

---

## Phase 1：人类轮强化（Month 1，立即开始）

### 1.1 跨仓库搜索 `devbase grep`

**价值**：lazygit/gitui 完全没有的能力。人类用户在 TUI 中按 `/` 输入 pattern，搜索所有注册仓库的文件内容。

**实现**：
- TUI 新增 `InputMode::SearchInput`
- 按 `/` 进入搜索模式，输入 pattern
- 用 `ripgrep` 库（`grep` crate）或调用 `rg` 二进制在所有仓库中并行搜索
- 结果列表显示：仓库名、文件路径、匹配行、上下文
- 选中结果按 `Enter` 打开文件（调用 VS Code / vim / 默认编辑器）

**关键设计**：
```rust
// 新增 InputMode
pub enum InputMode {
    Normal,
    TagInput,
    SearchInput, // 新增
}

// App 新增字段
pub(crate) search_results: Vec<SearchResultItem>,
pub(crate) search_pattern: String,

// SearchResultItem
pub struct SearchResultItem {
    pub repo_id: String,
    pub file_path: String,
    pub line_number: usize,
    pub line_content: String,
    pub context_before: Vec<String>,
    pub context_after: Vec<String>,
}
```

**预期成果**：TUI 中按 `/` → 输入 `fetch_github` → 看到所有仓库中匹配的结果 → 按 Enter 打开。

---

### 1.2 Stars 趋势可视化

**价值**：GitHub Desktop 有 stars 显示，但没有趋势。devbase 可以显示 stars 增长曲线（从 cache 历史计算）。

**实现**：
- `repo_stars_cache` 表已有 `fetched_at` 字段，可以计算时间序列
- TUI 详情面板增加 small sparkline（用 ratatui 的 `Sparkline` widget）
- 显示最近 7 次 fetch 的 stars 变化

**预期成果**：选中仓库 → 详情面板底部显示 `★42 → ★45 → ★48 → ★52` 的趋势线。

---

### 1.3 TUI 性能优化

**价值**：50+ 仓库时 TUI 启动和刷新不能卡顿。

**实现**：
- `load_repos()` 中 `assess_safety` 是同步阻塞的，对 50 个仓库可能慢
- 将 `assess_safety` 也移到 `AsyncSingleJob` 框架中异步执行
- 或者至少用 `tokio::task::spawn_blocking` 包裹

**预期成果**：50 仓库 TUI 冷启动 < 1 秒。

---

## Phase 2：AI 轮基础（Month 1-2）

### 2.1 注册表 Schema 扩展：`code_metrics` ✅

**价值**：AI 需要知道"哪个项目最大""哪个项目测试最多"。

**实现**：
```sql
CREATE TABLE repo_code_metrics (
    repo_id TEXT PRIMARY KEY,
    total_lines INTEGER,
    source_lines INTEGER,
    test_lines INTEGER,
    comment_lines INTEGER,
    file_count INTEGER,
    language_breakdown TEXT, -- JSON: {"rust": 80%, "python": 20%}
    updated_at TEXT
);
```

- `scan` 流程中增加代码统计（用 `tokei` crate 或 `scc`）
- MCP 新增 `devkit_code_metrics` tool
- TUI 详情面板显示代码统计

**预期成果**：AI 问 "我最大的 Rust 项目是什么？" → devbase 返回准确的代码行数排名。

---

### 2.2 注册表 Schema 扩展：`module_graph` ✅

**价值**：AI 需要理解仓库内部的模块依赖关系。

**实现**：
```sql
CREATE TABLE repo_modules (
    repo_id TEXT,
    module_path TEXT,
    module_type TEXT, -- "lib", "bin", "mod", "test"
    dependencies TEXT, -- JSON array of module paths
    exported_symbols TEXT, -- JSON array
    PRIMARY KEY (repo_id, module_path)
);
```

- 用 `tree-sitter` 解析代码结构
- 第一阶段只做 Rust（`cargo metadata` 可以获取模块图）
- MCP 新增 `devkit_module_graph` tool

**预期成果**：AI 问 "devbase 项目中 scan 模块依赖哪些模块？" → 返回模块依赖图。

---

### 2.3 MCP Tool 扩展：查询类

**价值**：让 AI 能更灵活地查询注册表。

**新增 tool**：
- `devkit_query_repos` ✅ 已完成
- `devkit_query_metrics` — 查询 code_metrics
- `devkit_query_history` — 查询某个仓库的 sync/health 历史
- `devkit_compare_repos` — 对比两个仓库的健康度、stars、代码量

---

## Phase 3：双轮融合（Month 2-3）

### 3.1 TUI "AI 洞察"面板

**价值**：人类在 TUI 中直接看到 AI 生成的分析，不需要切换工具。

**实现**：
- 详情面板增加第四个区域 "AI Insights"
- 显示内容：
  - "该仓库 5 天未同步，建议 fetch"
  - "该仓库 stars 增速超过同类项目 2 倍"
  - "该仓库有 3 个 mirror 策略的依赖项已落后"
- 这些洞察由本地规则引擎生成（不需要 LLM，基于注册表数据计算）

**规则引擎示例**：
```rust
fn generate_insights(repo: &RepoItem) -> Vec<String> {
    let mut insights = vec![];
    if repo.stale_days > 7 {
        insights.push(format!("{} 天未同步，建议 fetch", repo.stale_days));
    }
    if repo.stars.unwrap_or(0) > 100 && repo.status_behind.unwrap_or(0) > 0 {
        insights.push("热门项目有未合并更新".to_string());
    }
    insights
}
```

---

### 3.2 与 Claude Code 建立 MCP 集成案例

**价值**：证明 AI 基础设施定位的可行性，产生首个"AI 调用 devbase"的真实案例。

**实现**：
- 在 `claude-code-rust` 项目（友军/竞品）中配置 devbase MCP Server
- 写一份集成指南：`docs/mcp-integration-guide.md`
- 录制一个演示场景：
  - 用户问 Claude："我本地有哪些项目需要同步？"
  - Claude 调用 `devkit_health` → 获取结果 → 告诉用户

**预期成果**：一个可复制的 MCP 集成案例，用于 README 展示和社交媒体传播。

---

### 3.3 与 5ire 的 MCP 集成探索

**价值**：5ire 是 MCP Client，devbase 是 MCP Server，天然互补。

**实现**：
- 研究 5ire 的 MCP Client 配置方式
- 尝试让 5ire 通过 stdio 调用 devbase MCP
- 验证 `devkit_query_repos` 返回的结构化数据能否被 5ire 正确消费

---

## Phase 4：生态建立（Month 3-6）

### 4.1 自然语言查询（TUI 内）

**价值**：人类用户不需要记 SQL 或命令，用自然语言查询注册表。

**实现**：
- TUI 中新增 `InputMode::NaturalLanguageQuery`
- 按 `?` 或 `:` 进入自然语言模式
- 用户输入："显示所有 dirty 的 Rust 项目"
- 本地规则引擎解析为 SQLite 查询（不需要 LLM）
- 结果显示在列表中

**解析器**：
```rust
fn nl_to_sql(query: &str) -> Option<String> {
    let q = query.to_lowercase();
    if q.contains("dirty") && q.contains("rust") {
        Some("SELECT * FROM repos WHERE language = 'rust' AND ...".to_string())
    }
    // ... 更多规则
}
```

---

### 4.2 智能同步建议

**价值**：AI 判断何时 sync、用哪个 policy，减少人类决策负担。

**实现**：
- 基于规则（不是 LLM）：
  - "behind > 0, dirty = false, policy = Conservative" → 建议 `safe sync`
  - "ahead > 0, no diverge, policy = Rebase" → 建议 `sync + push`
  - "diverged, policy = Mirror" → 建议 `manual review`
- TUI 中详情面板显示 "建议操作" 按钮
- MCP 新增 `devkit_sync_recommendation` tool

---

### 4.3 跨设备注册表同步（via syncthing-rust）

**价值**：多台开发机器共享同一个代码库注册表。

**实现**：
- SQLite 注册表文件通过 syncthing-rust 同步
- 或者通过 devbase 的 `registry export/import` 手动同步
- 长期：直接集成 syncthing-rust 的 BEP 协议

---

## 执行优先级（立即开始）

```
Week 1-2:  Phase 1.1 跨仓库搜索 (TUI grep)
Week 2-3:  Phase 1.2 Stars 趋势 + Phase 2.1 code_metrics
Week 3-4:  Phase 2.2 module_graph (cargo metadata 方式)
Week 4-5:  Phase 3.1 AI 洞察面板
Week 5-6:  Phase 3.2 Claude Code MCP 集成案例
Month 2+: Phase 4.x 长期建设
```

**每完成一个 Phase，commit + push 一次。**

---

## 成功指标

| 指标 | 当前 | Month 1 目标 | Month 3 目标 |
|------|------|-------------|-------------|
| 注册仓库数 | ? | 统计基线 | +50% |
| TUI DAU | ? | 统计基线 | +30% |
| MCP tool 数 | 19 | 22 | 30 |
| AI 集成案例 | 0 | 1 (Claude Code) | 3 (Claude + 5ire + Codex) |
| 代码语义索引覆盖率 | 0% | Rust 仓库 100% | 全语言 80% |

---

*文档结束*
