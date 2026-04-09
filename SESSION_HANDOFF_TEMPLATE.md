# Kimi CLI 会话交接模板

> 当你需要新开一个 Kimi CLI 窗口处理 `syncthing-rust-rearch` 或 `clarity` 等复杂任务时，复制以下对应段落发送给新的 AI 会话。

---

## 模板 A：排查 syncthing-rust-rearch 的 bug（参照官方 syncthing）

```
我正在开发一个 Rust 重构的 Syncthing 实现，项目位置：
C:\Users\22414\Desktop\syncthing-rust-rearch

这是一个自有项目（没有 upstream remote），目前可能存在 bug 需要排查。

我已经用 `devbase` 管理了本地开发环境。请先做以下准备：
1. 扫描并确认环境状态：
   cd C:\Users\22414\Desktop\devbase
   cargo run -- health --detail

2. 官方 Syncthing 的参考源码已经克隆在：
   C:\Users\22414\dev\third_party\syncthing

请帮我排查 `syncthing-rust-rearch` 中的 [描述你遇到的具体问题，例如：
"P2P 握手流程" / "版本向量冲突处理" / "BEP 协议消息编解码异常" / "文件块索引管理"]。

在分析时，如果我的实现与官方 syncthing（Go 版）存在架构差异或疑似遗漏，
请主动对比官方实现并指出差异点。

注意：
- `syncthing-rust-rearch` 是我的重写项目，`syncthing` 是官方参考仓库。
- 两个项目都已被 devbase 注册，你可以通过 `cargo run -- query syncthing` 确认。
```

---

## 模板 B：继续开发 devbase 功能

```
我正在开发一个名为 `devbase` 的开发者知识库管理工具，项目位置：
C:\Users\22414\Desktop\devbase

`devbase` 是 Rust 编写的 CLI + TUI 工具，核心功能：
- 扫描本地 Git 仓库并注册到 SQLite
- 批量同步第三方参考库（fetch-only / auto-pull）
- 健康检查与查询
- ratatui 交互式界面

本地环境已用 devbase 自身管理。请先做：
   cd C:\Users\22414\Desktop\devbase
   cargo run -- health --detail

当前需求：[描述你的具体任务，例如：
"实现 devbase mcp --transport stdio 子命令" / "给 query 加结构化表达式解析" / "优化 TUI 的 sync 快捷键"]。

相关文档：
- 架构记录：C:\Users\22414\Desktop\devbase\ARCHITECTURE.md
- MCP 契约：C:\Users\22414\Desktop\devbase\docs\mcp_contract.md
```

---

## 模板 C：开发 clarity 项目

```
我正在开发一个名为 `clarity` 的 Rust Agent 执行框架，项目位置：
C:\Users\22414\Desktop\clarity

当前阶段：Phase 2 完成 → Phase 3 实测。

请在开始前读取以下文档获取上下文：
- C:\Users\22414\Desktop\clarity\docs\README.md
- C:\Users\22414\Desktop\clarity\PROJECT_REPORT.md

当前需求：[描述具体任务]。

注意：devbase 是我的开发者环境管理工具，未来会通过 MCP 与 clarity 集成，
但目前两者独立开发。不要修改 clarity 的架构去适配还不存在的 devbase 接口。
```

---

## 通用提示

- **devbase 快捷命令**：
  ```powershell
  cd C:\Users\22414\Desktop\devbase
  cargo run -- health --detail    # 查看所有项目状态
  cargo run -- sync --strategy=fetch-only   # 检查第三方库更新
  cargo run -- tui                 # 启动交互式界面
  ```

- **常用项目路径速查**：
  - `clarity` → `C:\Users\22414\Desktop\clarity`
  - `syncthing-rust-rearch` → `C:\Users\22414\Desktop\syncthing-rust-rearch`
  - `devbase` → `C:\Users\22414\Desktop\devbase`
  - 官方参考库 → `C:\Users\22414\dev\third_party\`
