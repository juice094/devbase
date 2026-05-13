# Embedding Provider 配置指南

> devbase v0.17.0+ 不再内置 LLM 推理能力。Embedding 生成由外部 Provider 负责，devbase 仅存储和检索向量。

## 背景

从 v0.17.0 开始，`embedding` feature 从默认构建中移除。这意味着：

- **默认构建**（`cargo build --release`）不包含 Candle/Ollama 依赖，体积更小、编译更快
- **Embedding 生成**必须由外部服务完成，devbase 通过 HTTP API 与之交互
- **向量存储和搜索**仍由 devbase 原生支持（SQLite + `cosine_similarity` UDF）

## 方案一：Ollama（推荐，本地免费）

### 1. 安装 Ollama

Windows (PowerShell):
```powershell
winget install Ollama.Ollama
```

macOS:
```bash
brew install ollama
```

Linux:
```bash
curl -fsSL https://ollama.com/install.sh | sh
```

### 2. 拉取 Embedding 模型

```bash
ollama pull nomic-embed-text
```

> `nomic-embed-text` 生成 768-dim 向量，质量高且速度快。你也可以使用 `all-minilm`（384-dim，更快）。

### 3. 配置 devbase

编辑 `%APPDATA%\devbase\config.toml`（Windows）或 `~/.config/devbase/config.toml`（Linux/macOS）：

```toml
[embedding]
enabled = true
provider = "ollama"
model = "nomic-embed-text"
base_url = "http://localhost:11434"
timeout_seconds = 30
```

### 4. 验证服务可用

```bash
ollama list
# 应看到 nomic-embed-text

curl http://localhost:11434/api/embeddings -d '{
  "model": "nomic-embed-text",
  "prompt": "test"
}'
# 应返回 {"embedding": [0.1, 0.2, ...]}
```

### 5. 在 devbase 中测试语义召回

```bash
# 1. 启动一个 context 并添加记忆
devbase session save test-ctx "Test Project" "验证 embedding"
devbase session capture test-ctx "note" "这是关于 Rust 编译器的笔记"

# 2. 通过 MCP 或 CLI 为记忆注入向量（外部生成后存储）
# 对于 Skill Runtime 自动召回，devbase 会在执行 skill 时自动调用 Ollama

# 3. 运行一个 skill 并观察环境变量
$env:DEVBASE_ACTIVE_CONTEXT = "test-ctx"
devbase skill run some-skill
# 在 skill 进程中，DEVBASE_CONTEXT_MEMORIES 应包含相关记忆
```

## 方案二：OpenAI API（云端）

如果你已有 OpenAI API Key，可直接使用：

```toml
[embedding]
enabled = true
provider = "openai"
model = "text-embedding-3-small"
base_url = "https://api.openai.com/v1"
timeout_seconds = 30
```

**注意**：当前 Skill Runtime 的 `call_external_embedding_endpoint` 默认使用 Ollama `/api/embeddings` 请求格式。OpenAI 的 `/v1/embeddings` 格式不同（需要 `input` 字段而非 `prompt`，响应结构也不同）。

如需支持 OpenAI 格式，请：
1. 提交 Feature Request，或
2. 使用本地代理将 OpenAI 格式桥接为 Ollama 格式

## 方案三：重新启用内置 Embedding（向后兼容）

如果你希望恢复 v0.16.x 的行为（Candle 本地推理，零外部依赖）：

```bash
cargo build --release --features embedding
```

此时 `generate_query_embedding` 将使用 Candle + all-MiniLM 模型本地生成，无需 Ollama 守护进程。

## Troubleshooting

### `embedding provider not enabled in config.toml`

- 检查 `config.toml` 中 `[embedding]` 段的 `enabled = true`
- 确认 config 文件路径正确：`devbase config path`

### `Connection refused` / Ollama 无响应

- 确认 Ollama 服务正在运行：`ollama serve` 或系统托盘图标
- 检查防火墙是否拦截 11434 端口
- 尝试手动 curl 测试连通性

### 召回结果为空或 score 全为 0

- 确认 memories 已创建：`devbase session list`
- 确认 memories 已索引（有 `indexed_at` 字段）：
  ```sql
  SELECT id, content, embedding_model, indexed_at FROM agent_memories;
  ```
- 如果 `embedding` 为 NULL，说明尚未注入向量。Skill Runtime 会在执行时自动尝试生成并注入。

### 模型下载慢

```bash
ollama pull nomic-embed-text
# 或使用镜像加速（中国大陆用户）
# 设置环境变量 OLLAMA_HOST 和代理
```

## 架构图示

```
[User Skill Runtime]
        │
        ▼
build_recall_query(skill_id + args)
        │
        ▼
[Ollama localhost:11434] ──► generate embedding
        │
        ▼
[devbase SQLite] cosine_similarity(embedding, query_embedding)
        │
        ▼
Top-k memories ──► DEVBASE_CONTEXT_MEMORIES env
```

## 参考

- [Ollama 官方文档](https://github.com/ollama/ollama/blob/main/docs/api.md#generate-embeddings)
- [nomic-embed-text 模型](https://ollama.com/library/nomic-embed-text)
- RFC: `docs/RFC/agent-memory-vector-storage.md`
