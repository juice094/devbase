# RFC: Agent Memory Vector Storage — v0.17.0

**Status**: Draft → Accepted  
**Target Version**: v0.17.0  
**Author**: juice094  
**Date**: 2026-05-13

## 1. 问题陈述

devbase v0.16.x 内嵌了 Candle + Ollama 双后端用于生成 embedding。这导致：

- 编译时间膨胀 30~50%（Candle 及其 ML 依赖树）
- release 二进制体积显著增大
- Windows 跨平台兼容性恶化（CUDA/ROCm FFI）
- 运行时依赖 Ollama 守护进程，违背"轻量索引"承诺
- 架构边界模糊：devbase 从 "Context Compiler" 滑向了 "LLM Runtime"

## 2. 核心判断

**devbase 不需要 LLM 推理能力。**

devbase 的本质是 **Local Context Compiler**——将本地数字资产编译为 AI 可决策的结构化情境。Compiler 不生产原材料（embedding），只编译已存在的材料。

正确范式参照 PostgreSQL + pgvector：
- PostgreSQL **不生成** embedding
- PostgreSQL **存储** vector 类型
- PostgreSQL **索引** IVFFlat / HNSW
- PostgreSQL **查询** `<->` / `<=>` / `<#>` 距离算子

devbase 应该复制这个边界：**只做存储、索引和查询执行。**

## 3. 设计目标

| 目标 | 说明 |
|------|------|
| G1 | Embedding 生成完全外置（Ollama CLI / OpenAI API / Python 脚本） |
| G2 | devbase 默认构建零 LLM 依赖（Candle 等降级为 opt-in feature） |
| G3 | Agent Memory 支持向量存储 + 余弦相似度搜索（纯 SQL / UDF） |
| G4 | 现有 `devkit_embedding_store` / `devkit_embedding_search` 接口兼容保留 |
| G5 | 对 <10k memory 量级保持毫秒级查询延迟（暴力扫描足够快） |

## 4. 架构变更

### 4.1 Feature Flag 重构

```toml
# Cargo.toml
[features]
default = ["tui", "mcp", "lang-rust", "lang-python", "lang-js-ts", "lang-go"]
# embedding removed from default in v0.17.0
llm-backend = ["dep:devbase-embedding"]  # opt-in; pulls Candle + Ollama
```

`src/embedding.rs` 已有 `#[cfg(feature = "embedding")]` gate：
- 启用时：`generate_query_embedding` 通过 Candle/Ollama 生成
- 禁用时：`generate_query_embedding` 返回错误，强制调用方提供外部向量

### 4.2 Schema v34

```sql
ALTER TABLE agent_memories ADD COLUMN embedding BLOB;
ALTER TABLE agent_memories ADD COLUMN embedding_model TEXT;
ALTER TABLE agent_memories ADD COLUMN indexed_at DATETIME;

CREATE INDEX idx_agent_memories_embedding
    ON agent_memories(context_id, indexed_at)
    WHERE embedding IS NOT NULL;
```

设计决策：
- `BLOB` 存储 little-endian f32 数组，通用且零序列化开销
- `embedding_model` 记录模型来源（如 `"nomic-embed-text"`），便于后期一致性校验
- `indexed_at` 标记索引时间，支持增量重索引策略
- Partial index 避免为纯文本 memory 浪费索引空间

### 4.3 SQLite UDF: `cosine_similarity`

```rust
conn.create_scalar_function(
    "cosine_similarity",
    2,
    SQLITE_UTF8 | SQLITE_DETERMINISTIC,
    |ctx| {
        let a: Vec<u8> = ctx.get(0)?;  // little-endian f32 blob
        let b: Vec<u8> = ctx.get(1)?;
        // compute dot / (norm_a * norm_b)
        Ok(similarity as f64)
    },
)?;
```

注册时机：`WorkspaceRegistry::init_db_at` 中 `run_all` 之后统一注册。

### 4.4 数据流

```
[外部 Embedding Provider]
        │  POST /embed {input: "query"}
        ▼
   query_embedding [f32; 768]
        │
        ▼
[devkit_session_recall]
        │  SELECT ... ORDER BY cosine_similarity(embedding, ?) DESC
        ▼
   top-k AgentMemories
        │
        ▼
[Skill Runtime] DEVBASE_CONTEXT_MEMORIES 注入
```

## 5. API 变更

### 5.1 保留接口（零行为变更）

- `devkit_embedding_store` — 继续接受外部向量写入 code_symbols 表
- `devkit_embedding_search` — 继续基于外部向量搜索 code_symbols
- `devkit_semantic_search` — 若未提供 `query_embedding` 且 `llm-backend` 未启用，返回明确错误

### 5.2 新增接口

**`devkit_session_recall`**
- 输入：`context_id`, `query_embedding` (required, external), `limit`
- 行为：注册 UDF → 执行 `cosine_similarity` ORDER BY → 返回 memories + score
- 约束：`query_embedding` 必须外部生成；devbase 不生成

**`devkit_session_index`**
- 输入：`memory_id`, `embedding` (external), `embedding_model`
- 行为：UPDATE agent_memories SET embedding=?, embedding_model=?, indexed_at=NOW()
- 用途：外部 indexer pipeline 调用，批量为已有 memories 注入向量

### 5.3 修改接口

**`devkit_session_save`** — 无变更。memories 仍以纯文本形式创建，embedding 通过后续 `devkit_session_index` 注入。

## 6. 性能预期

对于 Agent Memory 典型量级（< 10,000 条 / context）：

| 操作 | 延迟 |
|------|------|
| 暴力余弦扫描 1k memories | ~1-3 ms |
| 暴力余弦扫描 10k memories | ~5-15 ms |
| 插入带 embedding memory | ~1 ms |

结论：无需引入 HNSW / IVF 索引。SQLite partial index + UDF 暴力扫描足够。

若未来量级突破 100k，可评估引入 `sqlite-vec` C 扩展，但绝不引入 PyTorch/Candle。

## 7. 回滚与兼容性

- Schema v34 仅 ADD COLUMN，无破坏性变更
- `llm-backend` feature 默认关闭；现有用户可通过 `--features llm-backend` 恢复旧行为
- `devkit_semantic_search` 的 `query_text` 自动生成功能在 `llm-backend` 启用时继续工作

## 8. 任务清单

- [x] RFC 撰写
- [x] Cargo.toml: `embedding` 从 default 移除
- [x] Schema v34 迁移脚本
- [x] `agent_memories` 结构体扩展 embedding 字段
- [x] `cosine_similarity` UDF 注册
- [x] `search_memories_semantic` 纯 SQL 实现
- [x] MCP Tools: `devkit_session_recall`, `devkit_session_index`
- [x] test_helpers.rs SCHEMA_DDL 同步
- [ ] 外部 provider 配置文档（config.toml 示例）
- [ ] Skill Runtime 自动召回集成（注入 top-k memories）
- [ ] CHANGELOG 更新
- [ ] AGENTS.md 版本号更新 → v0.17.0-dev

## 9. 决策记录

**ADR-017: Embedding Generation is Out of Scope for devbase**

> devbase 作为 Local Context Compiler，其职责边界止于向量存储和相似性查询执行。任何涉及神经网络推理的操作（embedding 生成、LLM 补全、多模态编码）均属于上游 Producer 职责，devbase 仅通过标准接口（BLOB 存储、余弦 UDF、HTTP config）与之集成。
