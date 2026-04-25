# TUI Skill Runtime 集成设计文档

> **分支**: `feature/tui-skill-research`
> **版本**: devbase v0.2.4
> **日期**: 2026-04-25
> **状态**: 调研完成，待实现

---

## 1. 架构分析摘要

### 1.1 现有 TUI 架构概览

devbase TUI 采用经典的三层架构：**状态层 → 事件循环 → 渲染层**。

```
┌─────────────────────────────────────────────────────────────┐
│  State Layer (src/tui/state.rs)                             │
│  ├─ App 结构体：持有所有可变状态                               │
│  ├─ 数据加载：load_repos(), load_vaults()                    │
│  ├─ 导航：next(), previous(), jump_to_top/bottom()           │
│  └─ 业务动作：start_safe_sync(), execute_search(), ...       │
├─────────────────────────────────────────────────────────────┤
│  Event Loop (src/tui/event.rs)                              │
│  ├─ 50ms poll + crossterm KeyEvent                          │
│  ├─ 弹窗拦截模式：SyncPopup / HelpPopup / SearchPopup        │
│  ├─ InputMode 状态机：Normal → TagInput / SearchInput        │
│  └─ 按键 → TuiAction (Quit / LaunchExternal)                │
├─────────────────────────────────────────────────────────────┤
│  Render Layer (src/tui/render/*.rs)                         │
│  ├─ mod.rs: 主分发器 + bottom bar                            │
│  ├─ list.rs: 左侧面板 (RepoList / VaultList)                │
│  ├─ detail.rs: 右上面板 (repo detail / vault detail)        │
│  ├─ logs.rs: 右下面板                                       │
│  ├─ popups.rs: 搜索/同步弹窗                                 │
│  └─ help.rs: 快捷键帮助页                                    │
└─────────────────────────────────────────────────────────────┘
```

### 1.2 关键数据流

1. **视图切换**：`MainView` 枚举（`RepoList | VaultList`）控制整个左右布局的语义。
2. **列表渲染**：`list.rs` 的 `render_list()` 按 `app.main_view` dispatch，使用 `ratatui::widgets::List` + `ListState` 管理选中态。
3. **详情渲染**：`detail.rs` 的 `render_detail()` 同理 dispatch，右侧面板始终与左侧选中项联动。
4. **输入模式**：底部栏在 `InputMode::Normal` 显示快捷键提示，在 `TagInput/SearchInput` 显示输入缓冲区。
5. **异步通知**：`crossbeam_channel` 传递 `AsyncNotification`，在事件循环尾部 `try_recv()` 批量处理。

### 1.3 与 Skill Runtime 的现有接口

Skill Runtime 已具备完整的能力层，TUI 只需调用：

| 能力 | 函数位置 | 返回类型 |
|---|---|---|
| 列出 Skill | `skill_runtime::registry::list_skills(conn, type_filter)` | `Vec<SkillRow>` |
| 搜索 Skill | `skill_runtime::registry::search_skills_text(conn, query, limit)` | `Vec<SkillRow>` |
| 执行 Skill | `skill_runtime::executor::run_skill(skill, args, timeout)` | `ExecutionResult` |
| 记录执行 | `skill_runtime::registry::record_execution_start/finish` | `i64` / `()` |

数据结构 `SkillRow`（`src/skill_runtime/registry.rs:384-406`）已包含 TUI 展示所需的全部字段：
- `id`, `name`, `version`, `description`, `author`, `tags`
- `skill_type: SkillType` (Builtin / Custom / System)
- `entry_script`, `local_path`
- `installed_at`, `updated_at`, `last_used_at`

---

## 2. 最小可行实现方案（MVP）

### 2.1 设计原则

- **零破坏**：不删除/重命名任何现有枚举变体或函数签名。
- **最大复用**：复用 `MainView` 切换机制、`List`/`ListState` 列表组件、`detail` 面板布局、`InputMode` 输入框架、`logs` 输出通道。
- **渐进增强**：第一版只读 + 执行，不实现 install/uninstall/edit。

### 2.2 变更清单

#### A. 状态层 (`src/tui/mod.rs`)

在 `MainView` 枚举中增加 `SkillList`：

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum MainView {
    RepoList,
    VaultList,
    SkillList,   // ← 新增
}
```

`MainView::toggle()` 改为三态循环（或保留双态，新增 `k` 键直达）。

在 `App` 结构体中增加 Skill 相关状态：

```rust
pub struct App {
    // ... existing fields ...
    pub(crate) skills: Vec<crate::skill_runtime::SkillRow>,
    pub(crate) skill_selected: usize,
    pub(crate) skill_list_state: ListState,
    pub(crate) skill_run_popup_mode: SkillRunPopupMode, // 见下文
}
```

#### B. 事件层 (`src/tui/event.rs`)

1. **新增按键绑定**：`Normal` 模式下 `KeyCode::Char('k')` 切换至 `MainView::SkillList`（或作为三态 toggle 的入口）。
2. **`Enter` 行为扩展**：在 `SkillList` 视图下，按 `Enter` 触发选中 Skill 的执行流程。
3. **新增弹窗拦截**：`SkillRunPopupMode` 处理参数输入与执行确认。

新增枚举（建议放在 `mod.rs`）：

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum SkillRunPopupMode {
    Hidden,
    Input,      // 底部/居中输入参数，类似 SearchInput
    Running,    // 显示执行中状态（可复用 sync progress 风格）
    Result,     // 显示 ExecutionResult（stdout/stderr/exit_code）
}
```

#### C. 渲染层

**`render/list.rs`** — 复用现有列表模式：

```rust
pub(crate) fn render_list(frame: &mut Frame, app: &mut App, area: Rect, styles: &Styles) {
    match app.main_view {
        MainView::RepoList => render_repo_list(frame, app, area, styles),
        MainView::VaultList => render_vault_list(frame, app, area, styles),
        MainView::SkillList => render_skill_list(frame, app, area, styles), // ← 新增
    }
}
```

`render_skill_list()` 实现要点：
- 使用 `ratatui::widgets::List` + `app.skill_list_state`。
- 每行显示：`[type_icon] name  [tag1] v{version}`。
- `type_icon` 映射：`Builtin → ⭐`, `Custom → 🔧`, `System → ⚙`。
- 空列表提示：运行 `devbase skill install <url>` 或 `devbase skill list`。

**`render/detail.rs`** — 复用 detail 面板：

```rust
pub(crate) fn render_detail(frame: &mut Frame, app: &mut App, area: Rect, styles: &Styles) {
    match app.main_view {
        MainView::RepoList => render_repo_detail(frame, app, area, styles),
        MainView::VaultList => render_vault_detail(frame, app, area, styles),
        MainView::SkillList => render_skill_detail(frame, app, area, styles), // ← 新增
    }
}
```

`render_skill_detail()` 展示字段：
- 名称 + 版本（大标题）
- `id`, `skill_type`, `author`
- `description`
- `entry_script` 路径
- `inputs`（需要从 `SkillMeta` 或单独查询；MVP 可直接从 `SkillRow` 提示"有 N 个参数"）
- `tags`
- 最近执行状态（从 `skill_executions` 查最新一条）

**`render/popups.rs`** — 新增 Skill 执行弹窗：

```rust
pub(crate) fn render_popups(frame: &mut Frame, app: &mut App, styles: &Styles) {
    // ... existing search / sync popups ...
    match app.skill_run_popup_mode {
        SkillRunPopupMode::Input => render_skill_input(frame, app, styles),
        SkillRunPopupMode::Running => render_skill_running(frame, app, styles),
        SkillRunPopupMode::Result => render_skill_result(frame, app, styles),
        SkillRunPopupMode::Hidden => {}
    }
}
```

- `render_skill_input`：居中弹窗，显示 Skill 名称 + 参数提示，底部输入框等待 `key=value` 格式（复用 `app.input_buffer`）。
- `render_skill_running`：复用 sync progress 的列表样式，显示 `RUNNING` 状态。
- `render_skill_result`：显示 `stdout`/`stderr`（Truncated to 行数限制）、exit_code、duration_ms。

**`render/help.rs`** — 在 Help 面板增加 Skill 分类：

```rust
let skill_lines = help_section(
    "Skill",
    &[("k", "进入 Skill 列表"), ("Enter", "执行选中 Skill"), ("Esc", "关闭弹窗")],
    styles,
);
```

**`render/mod.rs` (bottom bar)** — 更新视图标签和提示：

```rust
let view_label = match app.main_view {
    crate::tui::MainView::RepoList => "[Repos]",
    crate::tui::MainView::VaultList => "[Vault]",
    crate::tui::MainView::SkillList => "[Skills]", // ← 新增
};
```

增加 `k=skills` 快捷键提示。

#### D. 状态管理 (`src/tui/state.rs`)

1. **`App::new()`** 初始化：`skills: Vec::new()`, `skill_selected: 0`, `skill_list_state: ListState::default()`。
2. **新增 `load_skills()`**：
   ```rust
   pub(crate) fn load_skills(&mut self) -> anyhow::Result<()> {
       let conn = WorkspaceRegistry::init_db()?;
       let rows = crate::skill_runtime::registry::list_skills(&conn, None)?;
       self.skills = rows;
       self.skill_selected = 0;
       self.skill_list_state.select(Some(0));
       self.log_info(format!("已加载 {} 个 Skills", self.skills.len()));
       Ok(())
   }
   ```
3. **导航函数** (`next`, `previous`, `jump_to_top`, `jump_to_bottom`)：增加 `MainView::SkillList` 分支，操作 `skill_selected` 和 `skill_list_state`。
4. **新增 `run_selected_skill()`**：
   - 若 Skill 无输入参数（或 MVP 简化），直接调用 `executor::run_skill`。
   - 若有参数，进入 `SkillRunPopupMode::Input`。
   - 执行使用 `tokio::task::spawn_blocking`（复用 MCP 中的调用模式），通过 `async_tx` 发送自定义 `AsyncNotification::SkillRunFinished` 通知。
5. **异步通知处理**：在 `update_async()` 中增加 `SkillRunFinished` 分支，更新弹窗状态并写入日志。

#### E. i18n (`src/i18n/`)

在 `TuiStrings` 中新增（最小集）：
- `title_skills: &'static str`
- `help_skills: &'static str`
- `skill_no_params: &'static str`

---

## 3. 预估工作量

| 模块 | 变更内容 | 预估行数 | 风险 |
|---|---|---|---|
| `src/tui/mod.rs` | `MainView` + `SkillRunPopupMode` 枚举 | ~20 | 低 |
| `src/tui/event.rs` | 按键绑定 + 弹窗拦截逻辑 | ~40 | 低 |
| `src/tui/state.rs` | `load_skills`, 导航分支, 执行逻辑, 异步通知 | ~80 | 中（异步执行需测试） |
| `src/tui/render/list.rs` | `render_skill_list` | ~50 | 低 |
| `src/tui/render/detail.rs` | `render_skill_detail` | ~60 | 低 |
| `src/tui/render/popups.rs` | Skill 输入/运行/结果弹窗 | ~70 | 低 |
| `src/tui/render/help.rs` | 帮助文本新增 Skill 分类 | ~15 | 低 |
| `src/tui/render/mod.rs` | Bottom bar 视图标签 + 快捷键 | ~10 | 低 |
| `src/i18n/` | 中英文字符串常量 | ~10 | 低 |
| **合计** | | **~355** | |

**时间估算**：
- 编码：2-3 小时
- 本地手动测试（切换视图、执行 skill、弹窗交互）：1-2 小时
- 总计：**0.5 人日**

---

## 4. 关键代码位置引用

### 4.1 TUI 核心

| 文件 | 行号范围 | 说明 |
|---|---|---|
| `src/tui/mod.rs` | 17-29 | `MainView` 枚举定义 |
| `src/tui/mod.rs` | 84-89 | `InputMode` 枚举 |
| `src/tui/mod.rs` | 123-157 | `App` 结构体字段 |
| `src/tui/event.rs` | 13-226 | 事件循环主逻辑 |
| `src/tui/event.rs` | 87-172 | `InputMode::Normal` 按键处理 |
| `src/tui/state.rs` | 12-59 | `App::new()` 初始化 |
| `src/tui/state.rs` | 310-381 | `next` / `previous` / `jump_to_*` |
| `src/tui/state.rs` | 428-494 | `update_async()` 通知分发 |

### 4.2 渲染层

| 文件 | 行号范围 | 说明 |
|---|---|---|
| `src/tui/render/mod.rs` | 17-47 | `ui()` 主渲染分发器 |
| `src/tui/render/mod.rs` | 49-116 | `render_bottom_bar()` 底部栏 |
| `src/tui/render/list.rs` | 11-16 | `render_list()` dispatch |
| `src/tui/render/list.rs` | 18-136 | `render_repo_list()` — 复用模板 |
| `src/tui/render/detail.rs` | 12-17 | `render_detail()` dispatch |
| `src/tui/render/detail.rs` | 19-252 | `render_repo_detail()` — 复用模板 |
| `src/tui/render/popups.rs` | 12-24 | `render_popups()` dispatch |
| `src/tui/render/popups.rs` | 283-349 | `render_sync_progress()` — 复用模板 |
| `src/tui/render/help.rs` | 11-82 | `render_help()` 帮助面板 |

### 4.3 Skill Runtime（只读调用）

| 文件 | 行号范围 | 说明 |
|---|---|---|
| `src/skill_runtime/mod.rs` | 62-81 | `SkillMeta` 结构体 |
| `src/skill_runtime/mod.rs` | 152-167 | `SkillRow` 结构体（TUI 可直接用） |
| `src/skill_runtime/registry.rs` | 132-152 | `list_skills()` |
| `src/skill_runtime/registry.rs` | 154-167 | `search_skills_text()` |
| `src/skill_runtime/registry.rs` | 169-181 | `record_execution_start()` |
| `src/skill_runtime/registry.rs` | 183-211 | `record_execution_finish()` |
| `src/skill_runtime/executor.rs` | 10-140 | `run_skill()` 同步执行入口 |
| `src/mcp/tools/skill.rs` | 177-222 | MCP 调用 `run_skill` 的封装参考 |

---

## 5. 未决问题

1. **参数输入 UX**：MVP 采用 `key=value` 单行输入（复用 `input_buffer`）。若 Skill 有多个必填参数，是否需要分步输入？
   - *建议 MVP 保持单行，用逗号分隔或多次按 Enter 确认。*
2. **执行结果展示**：`stdout` 可能很长，弹窗内需要分页或截断（参考 logs 面板的滚动逻辑）。
3. **Skill 安装入口**：TUI 内是否支持 `k` → `i` 触发 install？MVP 建议只提供 CLI 入口，TUI 内只读+执行。
4. **`MainView::toggle()` 语义**：当前为双态翻转。加入 `SkillList` 后，按 `Tab` 是否三态循环（Repo → Vault → Skill）？
   - *建议 `Tab` 保持 Repo↔Vault，`k` 直达 Skill，`Esc` 或 `k` 返回上一视图，避免破坏用户肌肉记忆。*

---

## 6. 结论

**TUI 集成 Skill Runtime 完全可行，且可以极低成本实现。**

现有架构的 `MainView` 切换机制、`ListState` 列表管理、`detail` 面板、`InputMode` 输入框架、`popups` 弹窗系统、`async_rx` 异步通道均可直接复用。Skill Runtime 的 `SkillRow` + `executor::run_skill` 提供了无需改动的只读调用接口。

按本方案实施，预计新增代码 **~350 行**，可在 **0.5 人日** 内完成 MVP，使 TUI 具备 Skill 浏览、查看详情、传参执行、结果展示的完整闭环。
