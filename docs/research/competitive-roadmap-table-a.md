# 表A 竞品蚕食路线图：由弱到强

> 友军：`syncthing-rust`（已排除）  
> 目标：按**由易到难**顺序，逐个击破/收编表A竞品用户  
> 策略：**先吃边缘用户 → 再切细分场景 → 最后建立生态位**

---

## 总体战力评估

```
竞品战力排名（高 → 低）：

lazygit  ████████████████████████████████████████  ★★★★★  最强，生态壁垒高
gitui    ██████████████████████████████            ★★★★   性能+体验优秀
gitoxide ████████████████                          ★★★    基础设施，用户粘性低
desktop  ██████████                                ★★     GUI 重，场景单一
gws      ████                                      ★      最弱，功能极简
         弱 ←——————————————————————————————————→ 强
```

**蚕食顺序：gws → desktop → gitoxide → gitui → lazygit**

---

## 第1战：吃掉 gws（Python Git Workspace）

### 敌方分析

| 维度 | gws | devbase |
|------|-----|---------|
| 技术栈 | Python | Rust |
| 安装方式 | `pip install gws` | `cargo install` / 二进制 |
| 多仓库支持 | ✅ 基础 status/fetch | ✅ 完整状态+同步+标签 |
| TUI | ❌ CLI only | ✅ ratatui |
| 性能 | 慢（Python + 子进程） | 快（Rust + libgit2） |
| 安全策略 | ❌ | ✅ SyncPolicy |
| 注册表 | ❌ | ✅ SQLite |

### gws 的致命弱点
1. **Python 依赖地狱**：需要 Python 环境，Windows 用户苦不堪言
2. **功能极简**：只做了 `status` 和 `fetch`，没有 sync、没有标签、没有健康检查
3. **无持久化**：每次重新扫描，没有注册表概念
4. **无可视化**：纯 CLI 输出，多仓库状态一屏装不下

### 吞并战术

**战术1A：功能全覆盖（已完成 ✅）**
- devbase 已经实现了 gws 的全部功能（多仓库 status/fetch）
- 额外提供：safe sync、tags、health check、stars、TUI

**战术1B：迁移引导（待做）**
- 增加 `devbase import --gws <path>` 命令，读取 `.gws` 配置文件，一键迁移
- gws 的配置格式是 `repo_name = git_url`，devbase 可以无缝解析

**战术1C：性能碾压（天然优势）**
- gws 扫描 50 个仓库需要 ~30 秒（Python subprocess）
- devbase 扫描 50 个仓库需要 ~3 秒（Rust + 并发）
- 在 README 中放 benchmark 对比

**预期成果**：gws 用户群极小，但**吃掉的不是人数，是概念**——证明"Python 版的 workspace 管理已死，Rust 版是正统"。

---

## 第2战：蚕食 desktop（GitHub Desktop）

### 敌方分析

| 维度 | desktop | devbase |
|------|---------|---------|
| 技术栈 | TypeScript/Electron | Rust |
| 体积 | ~150MB | ~5MB |
| 启动速度 | 慢（Electron 冷启动） | 瞬时（TUI） |
| 多仓库 | ❌ 单窗口单仓库 | ✅ 批量仪表盘 |
| GitHub 集成 | ✅ PR、Actions、Issues | ⚠️ 仅 Stars |
| 可视化 diff | ✅ 行级 diff | ❌ 无 diff 视图 |
| 新手友好度 | ✅ 极高 | ⚠️ 需要 CLI 经验 |
| 无头环境 | ❌ 必须有 GUI | ✅ SSH/终端可用 |

### desktop 的致命弱点
1. **Electron 原罪**：150MB+ 安装包，内存占用高，Linux 支持差
2. **无批量管理**：每个仓库开一个窗口，10 个仓库就疯了
3. **无头环境残疾**：SSH 到服务器上完全无法使用
4. **慢**：冷启动 3-5 秒，devbase TUI 瞬间打开

### 蚕食战术

**战术2A：抢占"轻量化 Git GUI"心智**
- 在文档中定位 devbase 为"TUI 版 GitHub Desktop"——同样的新手友好（安全同步策略兜底），但轻 30 倍
- 强调："不需要鼠标，不需要 GUI 环境，SSH 到服务器也能用"

**战术2B：吃掉 desktop 的"多仓库用户"**
- desktop 用户中有一部分是管理多个小型仓库的（个人项目、微服务）
- devbase 的批量视图是这部分用户的刚需，desktop 完全无法覆盖
- 在 release note 中强调 "manage 50 repos in one TUI"

**战术2C：GitHub 集成补足（中期）**
- desktop 的 GitHub PR/Issue 集成是其核心护城河
- devbase 当前只有 Stars，需要扩展：
  - `devbase pr list` — 列出所有仓库的 open PR
  - `devbase pr create` — 批量创建 PR（用 GitHub CLI 的 gh 命令）
  - TUI 中增加 PR 数量徽章

**战术2D：diff 视图（长期）**
- desktop 最强的是行级 diff 可视化
- devbase 可以集成 `delta` 或自研 diff 组件（ratatui 支持）：
  - 选中仓库按 `d` 进入 diff 视图
  - 显示 staged/unstaged 变更的 side-by-side diff

**预期成果**：不吃掉 desktop 的全部用户（新手 GUI 用户永远存在），但**吃掉"用 desktop 管多个仓库的痛点用户"**。

---

## 第3战：收编 gitoxide 的 CLI 用户

### 敌方分析

| 维度 | gitoxide | devbase |
|------|----------|---------|
| 定位 | Git 的纯 Rust 重新实现 | 开发者工作区管理 |
| 形态 | 库 + CLI (`gix`/`ein`) | TUI + CLI |
| TUI | ❌ | ✅ |
| 多仓库 | ❌ | ✅ |
| 底层控制 | ✅ 极致（可编程 Git） | ⚠️ 通过 libgit2 |
| 用户群体 | 开发者/基础设施 | 终端用户 |

### gitoxide 的弱点
1. **没有 TUI**：`ein` CLI 是命令式工具，不是交互式
2. **没有多仓库概念**：每次操作指定单个仓库
3. **学习曲线陡峭**：API 设计偏底层，用户需要理解 Git 内部机制

### 收编战术

**战术3A：成为 gitoxide 的"上层 UI"**
- gitoxide 的定位是"库/基础设施"，不是终端用户工具
- devbase 可以成为 gitoxide 生态的推荐 TUI 前端：
  - 在文档中写 "powered by libgit2, compatible with gitoxide ecosystem"
  - 未来可选后端：libgit2 ↔ gitoxide（gix crate）

**战术3B：吃掉"想要纯 Rust Git 工具"的用户**
- gitoxide 吸引的是"讨厌 git 命令"的 Rust 开发者
- 这些用户同样会被 devbase（纯 Rust 工具链）吸引
- devbase 比 `ein` 更容易用（TUI 点点点 vs CLI 记命令）

**战术3C：功能互补（不正面冲突）**
- gitoxide 做的是"替代 git 命令"
- devbase 做的是"管理多个 git 仓库"
- 两者是垂直关系，不是水平竞争
- 在 devbase 中增加 `devbase git <command>` 透传，底层可选 gitoxide

**预期成果**：不战而胜。gitoxide 的用户会自动流入 devbase，因为两者解决不同层级的问题，且都是 Rust 生态。

---

## 第4战：与 gitui 共存，蚕食盲区用户

### 敌方分析

| 维度 | gitui | devbase |
|------|-------|---------|
| 技术栈 | Rust | Rust |
| TUI 框架 | 自研 | ratatui |
| 单仓库操作 | ✅ 非常流畅 | ⚠️ 仅 fetch/sync |
| 文件级操作 | ✅ stage/unstage/hunk | ❌ |
| 多仓库视图 | ❌ | ✅ 核心能力 |
| 性能 | ✅ 极快 | ✅ 极快 |
| 生态 | ✅ 成熟，社区大 | 🆕 新兴 |

### gitui 的弱点
1. **单仓库锁定**：打开 gitui 就是一个仓库，切换仓库要退出重开
2. **无同步策略**：push/pull 就是裸 git 命令，没有安全兜底
3. **无知识库**：没有仓库摘要、stars、健康度等元信息
4. **无批量操作**：不能一次对 10 个仓库执行 fetch

### 蚕食战术

**战术4A：成为 gitui 的"启动器"（最关键）**
- 在 devbase TUI 中，选中仓库按 `Enter` 启动 gitui（如果已安装）
- 实现方式：检测 `gitui` 二进制，存在则 `std::process::Command::new("gitui").current_dir(repo_path).spawn()`
- 用户在 devbase 看全景，进 gitui 做精细操作，**分工明确**

```rust
// 在 event.rs 中增加
KeyCode::Enter => {
    if let Some(repo) = app.current_repo() {
        if which::which("gitui").is_ok() {
            // 挂起 TUI，启动 gitui，退出后恢复
            ratatui::restore();
            let _ = std::process::Command::new("gitui")
                .current_dir(&repo.path)
                .status();
            terminal = ratatui::init();
            terminal.clear()?;
        }
    }
}
```

**战术4B：吃掉"多仓库场景的 gitui 用户"**
- gitui 用户在管理多个仓库时极其痛苦（不断 cd + gitui）
- devbase 的批量视图正好覆盖这个场景
- 在 README 中写："if you use gitui for 3+ repos, you need devbase"

**战术4C：功能差异化（不重复造轮子）**
- devbase 不碰 gitui 擅长的领域：hunk stage、interactive rebase、blame
- devbase 专注：多仓库状态聚合、安全批量同步、知识库、标签过滤
- **让 gitui 用户觉得"两者都需要"，而不是"二选一"**

**预期成果**：gitui 无法正面击败（单仓库体验太好），但**devbase 可以成为 gitui 用户的"第二屏"**——每天用 devbase 看全局，每周用 gitui 做精细提交。

---

## 第5战：与 lazygit 建立生态位，长期博弈

### 敌方分析

| 维度 | lazygit | devbase |
|------|---------|---------|
| 技术栈 | Go | Rust |
| TUI 框架 | 自研 (gocui) | ratatui |
| 单仓库 Git 操作 | ✅ **行业标杆** | ❌ 差距巨大 |
| 交互设计 | ✅ 极其成熟 | 🆕 待打磨 |
| 生态/社区 | ✅ 巨大 | 🆕 小型 |
| 多仓库视图 | ❌ | ✅ |
| 批量同步策略 | ❌ | ✅ |
| 可定制性 | ⚠️ yaml 配置 | ✅ config.toml + 代码 |

### lazygit 的弱点
1. **Go 生态 vs Rust 生态**：Rust 开发者更倾向于用 Rust 工具（主观偏好）
2. **多仓库管理缺失**：和 gitui 一样，单仓库工具
3. **无注册表/知识库**：没有仓库元信息持久化
4. **配置方式单一**：yaml 配置，无插件/MCP 扩展能力

### 长期博弈战术

**战术5A：Rust 生态忠诚度**
- lazygit 是 Go 写的，在 Rust 社区天然有"外来者"劣势
- devbase 纯 Rust，可以和 cargo、rust-analyzer 等工具链深度集成
- 在 Rust 开发者群体中优先推广

**战术5B：MCP 差异化（降维打击）**
- lazygit 是传统 TUI 工具，无 AI/Agent 概念
- devbase 的 MCP Server 能力是 lazygit 完全无法复制的：
  - AI 助手通过 devbase 查询"我本地有哪些 Rust 项目有未推送提交"
  - lazygit 永远是一个"人操作的工具"，devbase 可以是"Agent 调用的工具"

**战术5C：同样做 lazygit 的启动器**
- 和 gitui 一样，devbase 按 `Enter` 启动 lazygit
- lazygit 的单仓库操作体验确实更好，不硬拼

**战术5D：交互设计追赶（长期）**
- lazygit 的键位设计是行业标杆（`?` 帮助、`space` 选择、`enter` 确认）
- devbase 应该学习其交互模式，降低迁移成本：
  - 统一键位：`?` 帮助、`/` 搜索、`q` 退出
  - 面板布局：左侧列表、右侧详情、底部快捷键提示

**预期成果**：lazygit 无法短期击败，但 devbase 可以**在 Rust 社区 + MCP 生态 + 多仓库场景**中建立独立生态位。lazygit 管"一个仓库怎么提交"，devbase 管"五十个仓库怎么同步"。

---

## 总体路线图时间线

```
Month 1-2          Month 3-6           Month 6-12          Year 2+
   │                  │                   │                  │
   ▼                  ▼                   ▼                  ▼
┌──────┐        ┌──────────┐        ┌──────────┐       ┌──────────┐
│ 吃掉  │        │ 蚕食     │        │ 收编     │       │ 建立     │
│ gws   │   →    │ desktop  │   →    │ gitui    │  →    │ 生态位   │
│       │        │          │        │ gitoxide │       │ vs       │
│       │        │ 集成     │        │          │       │ lazygit  │
│       │        │ delta    │        │ 启动器   │       │          │
│       │        │ diff     │        │ 模式     │       │ MCP      │
│       │        │          │        │          │       │ 差异化   │
└──────┘        └──────────┘        └──────────┘       └──────────┘

胜利标准：
• gws: 用户全部迁移（人数少，但概念胜利）
• desktop: 吃掉"多仓库痛点用户"（~20% 重叠用户）
• gitoxide: 成为推荐 TUI 前端（生态合作）
• gitui: 成为"第二屏"（50% gitui 用户同时用 devbase）
• lazygit: Rust 社区首选 + MCP 场景不可替代
```

---

## 下一步行动（立即执行）

1. **增加 gitui/lazygit 启动器**（1天）：按 `Enter` 检测并启动外部 TUI
2. **增加 `import --gws` 命令**（1天）：读取 `.gws` 配置迁移用户
3. **README 定位升级**（半天）：明确写 "TUI dashboard for managing multiple Git repos, with safe sync policies"
4. **benchmark 脚本**（1天）：对比 gws / devbase 扫描 50 仓库的性能

*文档结束*
