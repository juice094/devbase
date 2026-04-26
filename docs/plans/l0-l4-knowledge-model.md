# L0-L4 五层知识模型 — 路线图草案

> **状态**：部分实现（v0.10.0 进行中）
> **最后更新**：2026-04-26
> **生效范围**：devbase Registry Schema v18+

---

## 1. 五层模型总览

| 层级 | 名称 | 内容示例 | 生长信号 | 遗忘机制 |
|:---|:---|:---|:---|:---|
| L0 | 对象 | 代码符号、文档段落、日志块、论文 PDF | 检索频率、引用次数 | 版本冻结（repo tag 锁定） |
| L1 | 方法 | 检索/分块/向量化策略、搜索参数 | 检索成功率、延迟分布 | A/B 测试淘汰 |
| L2 | 哲学 | 设计原则（本地优先、奥卡姆剃刀） | 架构决策事后验证 | 外部论文扰动更新 |
| L3 | 风险 | 系统弱点图谱、已知限制 | 故障事件、异常日志频率 | 红队攻击结果 |
| L4 | 元认知 | 关于 L1-L3 的元知识、人类纠正记录 | 人类纠正次数、跨会话一致性 | 形式化验证通过 |

---

## 2. 存储映射（初步设计）

### 2.1 细粒度知识 → SQLite（快速查询）

```sql
-- L0 对象层
CREATE TABLE knowledge_objects (
    id              TEXT PRIMARY KEY,
    object_type     TEXT NOT NULL,          -- 'code_symbol' | 'doc_chunk' | 'log_entry' | 'paper_pdf'
    source_repo     TEXT,
    source_path     TEXT,
    content_hash    TEXT,                   -- blake3
    content_text    TEXT,
    embedding       BLOB,                   -- 可选，768-dim f32
    retrieval_count INTEGER DEFAULT 0,
    reference_count INTEGER DEFAULT 0,
    frozen_at       TEXT,                   -- RFC3339，NULL = 未冻结
    created_at      TEXT NOT NULL
);

-- L1 方法层
CREATE TABLE knowledge_methods (
    id              TEXT PRIMARY KEY,
    method_name     TEXT NOT NULL,          -- 'semantic_search' | 'chunk_paragraph' | 'bm25_keyword'
    config_json     TEXT,                   -- 方法参数 JSON
    success_rate    REAL DEFAULT 0.0,
    avg_latency_ms  REAL,
    last_evaluated  TEXT,
    ab_test_group   TEXT                    -- 'control' | 'treatment_a' | NULL
);

-- L2 哲学层（与 vault_notes 关联）
CREATE TABLE knowledge_philosophy (
    id              TEXT PRIMARY KEY,
    vault_note_id   TEXT REFERENCES vault_notes(id),
    principle       TEXT NOT NULL,          -- 'local-first' | 'occams-razor' | 'zero-trust'
    validation_count INTEGER DEFAULT 0,     -- 事后验证次数
    invalidated_at  TEXT                    -- 被推翻时标记
);

-- L3 风险层（已实现：v0.10.0 Wave 35，表名为 `known_limits`）
CREATE TABLE known_limits (
    id              TEXT PRIMARY KEY,
    category        TEXT NOT NULL,          -- 'hard-veto' | 'known-bug' | 'external-dep'
    description     TEXT NOT NULL,
    source          TEXT,
    severity        INTEGER,                -- 1-5
    first_seen_at   TEXT NOT NULL,
    last_checked_at TEXT,
    mitigated       INTEGER DEFAULT 0
);

-- L4 元认知层
CREATE TABLE knowledge_meta (
    id              TEXT PRIMARY KEY,
    target_level    INTEGER NOT NULL,       -- 1 | 2 | 3
    target_id       TEXT NOT NULL,          -- 指向 methods/philosophy/risks 的 ID
    correction_type TEXT,                   -- 'human-feedback' | 'cross-session-drift' | 'formal-proof'
    correction_json TEXT,                   -- 纠正内容
    confidence      REAL DEFAULT 0.0,       -- 0.0-1.0
    created_at      TEXT NOT NULL
);
```

### 2.2 粗粒度语义知识 → Vector DB（语义检索）

- L2 哲学原则、L3 风险描述的 **摘要向量**
- 使用现有 `embedding.rs` 协议存储，不绑定具体模型
- 查询时：向量语义搜索 → 关联到 SQLite 细粒度记录

---

## 3. 与现有 Schema 的兼容性

| 现有表 | 映射到 L0-L4 | 说明 |
|:---|:---|:---|
| `entities` / `entity_types` | L0 对象层 | 通用实体存储，可扩展 `knowledge_level` 字段 |
| `relations` | L0-L1 关联 | 实体间关系，可作为方法调用链 |
| `vault_notes` | L2 哲学层载体 | PARA 笔记，新增 `ai_context=true` 的笔记自动入 L2 |
| `oplog` | L3 风险层信号源 | 异常日志频率 → 风险评分 |
| `known_limits` | L3 风险层实现 | Hard Veto / 已知限制 / 外部依赖风险 |
| `knowledge_meta` | L4 元认知层实现 | 人类纠正、跨会话一致性记录 |
| `skills` / `skill_executions` | L1 方法层 | Skill 执行成功率 → 方法有效性评估 |

---

## 4. 开放问题（待决策）

1. **L0 对象是否需要与 `entities` 表合并？**
   - 选项 A：合并 — 在 `entities` 表新增 `knowledge_level` 字段（默认值 0）
   - 选项 B：独立 — `knowledge_objects` 作为专用表，`entities` 保持通用实体语义

2. **遗忘机制是物理删除还是逻辑标记？**
   - 选项 A：逻辑标记（`frozen_at` / `invalidated_at`）— 保留历史，可审计
   - 选项 B：物理删除 + OpLog 记录 — 节省空间，但无法回溯

3. **L4 元认知的跨会话一致性如何校验？**
   - 短期：依赖人类纠正信号的显式标记
   - 长期：可能需要与 clarity 项目的会话状态协议对接

---

## 5. 下一步行动

| 步骤 | 任务 | 优先级 |
|:---|:---|:---:|
| 1 | 冻结上述 Schema 设计（选择开放问题的选项） | P0 |
| 2 | 编写 `registry/migrate.rs` 的 v18 迁移逻辑 | P0 |
| 3 | 实现 `knowledge_objects` 表的 CRUD + MCP tool `devkit_knowledge_store` | P1 |
| 4 | 实现 L1-L4 的查询层 + MCP tool `devkit_knowledge_query` | P1 |
| 5 | 与 clarity 项目对接 L4 元认知的会话状态协议 | P2 |

---

*本文档为路线图草案，非最终规范。Schema v18 的正式 DDL 将在设计冻结后输出。*
