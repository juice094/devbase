# 执行计划：并行串行依赖调度

> 基于 `roadmap-2026.md` 的 12 个任务，按文件冲突风险 + 逻辑依赖排序

---

## 依赖关系图

```
Wave 1（立即并行，文件无冲突）
================================
├─ [A] 1.1 TUI grep ──────────────────────┐
│   文件: event.rs, state.rs, render.rs,   │
│        i18n/*, Cargo.toml                │
│   新增: grep crate                       │
│                                          │
├─ [B] 2.1 code_metrics schema ───────────┤
│   文件: registry.rs, scan.rs, mcp.rs,    │
│        Cargo.toml                        │
│   新增: tokei crate                      │
│                                          │
└─ [C] 3.2 Claude Code MCP 案例（文档）────┘
    文件: docs/mcp-integration-guide.md
    无代码冲突

Wave 2（Wave 1 完成后并行）
================================
├─ [D] 1.2 Stars 趋势 + 1.3 TUI 性能 ─────┐
│   文件: render.rs, state.rs, registry.rs,│
│        asyncgit.rs                       │
│   依赖: Wave 1 的 state.rs 变更已落地   │
│                                          │
└─ [E] 3.1 AI 洞察面板 ───────────────────┘
    文件: render.rs, i18n/*, state.rs
    依赖: Wave 1 的 render.rs/state.rs 变更已落地
    可选依赖: 2.1 code_metrics（有更好，无也能先用 health/stars）

Wave 3（Wave 2 完成后并行）
================================
├─ [F] 2.2 module_graph ──────────────────┐
│   文件: registry.rs, scan.rs, mcp.rs     │
│   依赖: 2.1 code_metrics 的 schema 框架  │
│                                          │
├─ [G] 2.3 MCP tool 扩展 ─────────────────┤
│   文件: mcp.rs                           │
│   依赖: 2.1 + 2.2 的数据表               │
│                                          │
├─ [H] 3.3 5ire 集成探索 ─────────────────┤
│   文件: docs/                            │
│   依赖: 3.2 Claude Code 集成经验         │
│                                          │
└─ [I] 4.2 智能同步建议 ──────────────────┘
    文件: render.rs, i18n/*, sync.rs
    依赖: 现有注册表数据（无强依赖）

Wave 4（长期，串行）
================================
└─ [J] 4.1 自然语言查询
    依赖: 2.1 + 2.2 + 2.3 全部完成
    
└─ [K] 4.3 跨设备同步（syncthing-rust）
    依赖: syncthing-rust 友军项目进度
```

---

## 文件冲突矩阵

| 任务 | event.rs | state.rs | render.rs | registry.rs | scan.rs | mcp.rs | asyncgit.rs | i18n | Cargo.toml | docs |
|:---:|:---:|:---:|:---:|:---:|:---:|:---:|:---:|:---:|:---:|:---:|
| **A** 1.1 grep | ✏️ | ✏️ | ✏️ | — | — | — | — | ✏️ | ✏️ | — |
| **B** 2.1 metrics | — | — | — | ✏️ | ✏️ | ✏️ | — | — | ✏️ | — |
| **C** 3.2 文档 | — | — | — | — | — | — | — | — | — | ✏️ |
| **D** 1.2+1.3 | — | ✏️ | ✏️ | ✏️ | — | — | ✏️ | — | — | — |
| **E** 3.1 洞察 | — | ✏️ | ✏️ | — | — | — | — | ✏️ | — | — |
| **F** 2.2 graph | — | — | — | ✏️ | ✏️ | ✏️ | — | — | — | — |
| **G** 2.3 MCP | — | — | — | — | — | ✏️ | — | — | — | — |

**Wave 1 安全**：A/B/C 三组的修改文件完全不重叠 ✅  
**Wave 2 安全**：D 改 render.rs/state.rs/asyncgit.rs，E 改 render.rs/state.rs/i18n。render.rs 和 state.rs 有重叠，需要 D 和 E 串行或细粒度协调。建议 **D 先做，E 后做**。

---

## 调度甘特图

```
Week:  1    2    3    4    5    6    7    8    9    10   11   12
       ├────┼────┼────┼────┼────┼────┼────┼────┼────┼────┼────┤
[A]    ████████████
[B]    ████████████████
[C]    ████
[D]         ████████████
[E]              ████████████
[F]                   ████████████
[G]                        ████████
[H]                        ████
[I]                             ████████
[J]                                  ████████████████
[K]                                       (友军联动，待定)
```

---

## 当前 Wave 1 任务分解

### [A] 1.1 TUI 跨仓库搜索 `devbase grep`

**工作量**：3 天  
**子代理**：coder（文件多、改动大）  
**输入**：
- 在 `InputMode` 中新增 `SearchInput`
- 按 `/` 进入搜索模式（类似 vim）
- 输入 pattern，按 Enter 执行搜索
- 用 `grep` crate 或调用 `rg` 二进制在所有注册仓库中搜索
- 结果列表显示：仓库名、文件路径、行号、匹配内容
- 按 `Esc` 退出搜索，回到 Normal 模式
- 按 `Enter` 在结果上打开文件（调用 `code` / `vim` / 默认编辑器）

**新增依赖**：`grep = "0.3"`（或直接用 `std::process::Command` 调用 `rg`）

### [B] 2.1 注册表 Schema：`code_metrics`

**工作量**：2 天  
**子代理**：coder（独立模块）  
**输入**：
- 新增 `repo_code_metrics` 表（schema 见 roadmap）
- `scan` 流程中集成代码统计
- 新增 MCP tool：`devkit_code_metrics`
- TUI 详情面板显示代码行数

**新增依赖**：`tokei = "13"`（代码统计）

### [C] 3.2 Claude Code MCP 集成案例

**工作量**：1 天  
**执行者**：父代理直接写文档  
**输入**：
- 写 `docs/mcp-integration-guide.md`
- 包含：MCP Server 配置方式、可用 tool 列表、3 个示例对话
- 不需要改代码

---

*计划结束 — 立即启动 Wave 1*
