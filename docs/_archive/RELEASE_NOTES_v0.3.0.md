# devbase v0.3.0 — 首个产品化发布

> **Bimodal Developer Workspace OS**  
> 双模态开发者工作区操作系统

---

## 一句话总结

v0.3.0 是 devbase 从"功能验证"进化为"可依赖的基础设施组件"的第一个产品化里程碑。34 个 MCP tools、完整的 Skill Runtime、人类友好的 TUI 仪表盘——全部经过测试验证，文档闭环，开箱可用。

---

## 核心能力

### 🤖 AI 层 — 34 MCP Tools

通过 [Model Context Protocol](https://modelcontextprotocol.io) 标准化接口，AI Agent 可以：

- **仓库管理**：扫描、健康检查、同步、GitHub 元数据查询
- **代码分析**：符号提取、依赖图、调用图、死代码检测、模块图
- **语义搜索**：向量语义搜索、混合检索（RRF）、跨仓库聚合、相关符号推荐
- **知识库**：Vault 笔记读写、搜索、反向链接
- **Skill 系统**：发现、搜索、执行、安装、发布、同步
- **专项工具**：arXiv 论文索引、实验日志、代码指标、知识覆盖报告

传输方式：**stdio**（本地进程通信）

### 👤 人类层 — TUI 仪表盘

```bash
devbase tui
```

- 多仓库健康总览（Git 状态、stars、同步策略）
- Vault 笔记浏览与编辑
- Skill 面板（`k` 键）：浏览、查看详情、传参执行、结果展示
- 批量同步预览与执行
- 主题系统 + 响应式布局

### 📦 Skill Runtime

完整的 Skill 全生命周期管理：

| 命令 | 功能 |
|------|------|
| `devbase skill list` | 列出内置/自定义 Skill |
| `devbase skill search <query> --semantic` | 文本/语义搜索 |
| `devbase skill run <id> --arg key=value` | 执行 Skill |
| `devbase skill install <git-url>` | 从 Git 安装 |
| `devbase skill publish` | 验证 + git tag + 推送 |
| `devbase skill sync --target clarity` | 同步到 Clarity |

依赖管理：Kahn 拓扑排序、DFS 环检测、自动安装缺失依赖。

---

## 技术规格

| 指标 | 数值 |
|------|------|
| Rust LOC | ~22,750 |
| MCP Tools | 34 |
| Tests | 239 passed / 0 failed / 3 ignored |
| Schema Version | 15 |
| Built-in Skills | 3 |
| Registry Scale（示例） | 45 repos, 153K symbols, 56K embeddings |

---

## 安装

### 一键安装（推荐）

```powershell
# Windows
irm https://raw.githubusercontent.com/juice094/devbase/main/scripts/install.ps1 | iex

# Linux / macOS
curl -fsSL https://raw.githubusercontent.com/juice094/devbase/main/scripts/install.sh | bash
```

### 从源码

```bash
git clone https://github.com/juice094/devbase.git
cd devbase
cargo install --path .
```

### 预编译二进制

见 [GitHub Releases](https://github.com/juice094/devbase/releases) 下载对应平台二进制。

---

## Quick Start

```bash
# 扫描并注册当前目录下的所有仓库
devbase scan . --register

# 查看健康状态
devbase health --detail

# 启动 TUI 仪表盘
devbase tui

# 启动 MCP Server（stdio 模式，供 AI 使用）
devbase mcp
```

---

## 已知限制

- **SSE transport**：暂未实现，仅支持 stdio。SSE 远程模式计划在未来版本提供。
- **跨仓库搜索**：TUI 内 `/` 搜索尚未实现。
- **自然语言查询**：TUI 内 `?` 模式尚未实现。

---

## 路线图

见 [`docs/ROADMAP.md`](docs/ROADMAP.md)。

**阶段一（本产品化发布）已达成。** 阶段二（协议层跃迁）将在 v0.3.0 稳定后启动。

---

## 致谢

devbase 是单人维护项目（Bus Factor = 1）。感谢所有提供反馈、Issue、想法的用户。详细贡献指南见 [`CONTRIBUTING.md`](CONTRIBUTING.md)。

---

*Release Date: 2026-04-25*  
*Commit: `64a7986`*
