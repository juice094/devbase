# devbase 重新定义：情境编译器

> 版本：v0.13.0 设计草案  
> 日期：2026-04-30  
> 状态：由 Kimi CLI 与 juice094 协作分析后重新定义  
> 前置分析：`vault/99-Meta/devbase-essence-analysis-20260430.md`

---

## 一、旧定义的问题

**旧定义**："本地优先的 AI Skill 编排基础设施"

**问题**：
1. "基础设施"暗示稳定、可依赖——当前 `index` 报错、双轨制未闭环，不可依赖
2. "Skill 编排"是子集而非本质——Skill 只是本地数字资产的一种
3. 定位粒度太粗，导致 37 个工具碎片蔓延，11 个 Experimental 无人维护

---

## 二、新定义

> **devbase 是本地情境编译器（Local Context Compiler）。**
>
> 它将本地数字资产的原始数据编译为 AI 可决策的结构化情境。
> 不是存储器，不是路由器，不是规则引擎，不是元工具——
> 而是将感知层数据编译为认知层结构的翻译系统。

### 三层架构

```
┌─────────────────────────────────────────┐
│  认知层：Kimi CLI / Claude / 其他 AI      │  ← 决策与执行
├─────────────────────────────────────────┤
│  协议层：MCP (JSON-RPC 2.0 / stdio)      │  ← 神经突触
├─────────────────────────────────────────┤
│  编译层：project_context / relations     │  ← 按目标过滤、生成相关结构
│  编码层：entities / entity_types         │  ← 统一模型、类型定义
│  感知层：scan / index / health           │  ← 扫描文件系统、感知存在性
├─────────────────────────────────────────┤
│  持久层：SQLite + Vault + OpLog          │  ← 跨会话记忆
│  资源层：本地文件系统                    │  ← 原始数据
└─────────────────────────────────────────┘
```

### 核心职责（三件事）

1. **发现（Discovery）**：本地有哪些代码库、Skill、Vault 笔记、工作流
2. **编排（Orchestration）**：安全地组合和执行 Skill（沙箱 + 审计）
3. **锚定（Anchoring）**：跨会话持久化关键决策和知识

### 明确不做的三件事

1. **不替代文件读取**：代码内容理解由 Kimi CLI 内置工具完成
2. **不替代版本控制**：Git 操作由 Git 完成
3. **不替代包管理**：依赖安装由 cargo/pip/npm 完成

---

## 三、AI 决策的信息模型

LLM 决策需要六维结构化情境：

| 维度 | 人类等效 | devbase 供给目标 |
|------|---------|-----------------|
| **Situation** | "我房间里有什么书？" | 仓库/笔记/Skill 全景 |
| **State** | "哪些书还没读完？" | Git 状态、健康度、索引状态 |
| **Relations** | "这本书引用了哪本？" | 依赖图、调用图、反链 |
| **Capability** | "我可以用笔划线、用书签标记" | 工具能力目录 + 输入输出契约 |
| **History** | "上次读这本书到哪一章？" | OpLog 可查询、可关联 |
| **Relevance** | "我现在要写论文，哪些书相关？" | 按目标过滤的上下文切片 |

**当前缺口**：Relevance（目标关联过滤）完全缺失，Relations 和 History 接近零。

---

## 四、与 Kimi CLI 的契约

### devbase 的承诺

- **协议稳定**：MCP 消息格式符合规范，无尾随字节，notification 静默处理
- **默认安全**：destructive 工具需显式启用，vault 路径锁定，skill 环境隔离
- **结构可消费**：返回 JSON，含 `success` / `error` 统一字段，schema 自描述

### Kimi CLI 的期望

- **先问 devbase，再读文件**：复杂任务先调用 `project_context` 获取结构，再按需读文件
- **利用 Vault 做跨会话记忆**：关键决策写入 Vault，下次会话通过 `vault_search` 召回
- **通过 OpLog 审计**：重要操作后查询 `oplog` 确认执行记录

---

## 五、v0.13.0 里程碑定义

v0.13.0 的目标是完成"情境编译器"的最小闭环。

### 必须完成（P0）

1. **entities 激活**：`health`、`query`、`sync` 全部走 `entities` 表，废弃 `repos` 表
2. **`index` 修复**：外键约束错误的根因定位和修复
3. **`relations` 填充**：从 `cargo metadata` / `use crate::` / git submodule 提取依赖关系

### 应该完成（P1）

4. **`project_context` 增强**：返回模块树 + 关键文件列表 + 文件间调用关系（不是全文）
5. **OpLog 可查询**：新增 `devkit_oplog_query` 工具，按 repo/skill/event_type 过滤
6. **Vault CLI 补全**：`vault list`、`vault read`、`vault write` 的 CLI 等价物

### 可以完成（P2）

7. **配置即代码**：`mcp.json` 由 devbase 生成/校验
8. **工具能力图谱**：37 个工具的输入输出契约注册到 `entity_types`

---

## 六、成功标准

v0.13.0 发布时，应能通过以下实战测试：

**测试任务**："分析 devbase 的 sync 模块架构"

**方法**：仅通过 devbase MCP 工具（不直接读文件）

**期望输出**：
```json
{
  "project": "devbase",
  "relevant_modules": ["sync"],
  "module_structure": {
    "sync": {
      "files": ["policy.rs", "tasks.rs", "orchestrator.rs", "tests.rs"],
      "entry_point": "orchestrator.rs",
      "key_types": ["SyncPolicy", "RepoSyncTask", "SyncMode"],
      "relationships": [
        "orchestrator.rs → tasks::collect_tasks()",
        "tasks.rs → policy::SyncPolicy::from_tags()"
      ]
    }
  },
  "suggested_files_to_read": ["src/sync/policy.rs", "src/sync/tasks.rs"]
}
```

---

*本文档取代此前所有关于 devbase 定位的描述。*
