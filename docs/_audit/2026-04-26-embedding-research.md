# 本地 Embedding 方案调研报告
日期: 2026-04-26
目标: 替换 `generate_query_embedding` 的 Python 回退，实现纯本地、零外部依赖的 embedding 生成
模型: sentence-transformers/all-MiniLM-L6-v2 (384-dim)

## 候选方案对比

| 维度 | A: `ort` + ONNX Runtime | B: `candle` + `tokenizers` (Recommended) |
|------|------------------------|------------------------------------------|
| **核心依赖** | `ort` (自动下载 ONNX Runtime 二进制) | `candle-core`, `candle-nn`, `candle-transformers` |
| **Tokenization** | 需额外处理（ONNX 不含 tokenizer） | `tokenizers` crate (内置 BPE/WordPiece) |
| **外部 C/C++ 依赖** | ONNX Runtime DLL (~10-20MB) | `onig` (可选, C regex; 可关闭) |
| **模型文件** | `model.onnx` (~22MB) | `model.safetensors` (~22MB) + `tokenizer.json` + `config.json` |
| **模型获取** | 手动下载/自建分发 | `hf-hub` 自动从 HuggingFace 下载并缓存 |
| **推理性能** | 优（ORT 高度优化） | 良（纯 Rust，单次 <100ms，够用） |
| **部署复杂度** | 中（Windows 需处理 DLL 路径/复制） | **低（全静态链接，单 binary）** |
| **代码量** | ~50 行 | ~80 行（含 mean pooling + L2 norm） |
| **编译时间增量** | 小 | 中（candle 依赖 gemm/rayon/safetensors） |
| **离线可用** | ✅ | ✅ |
| **Hard Veto 契合** | ✅ 开源 | ✅ **纯 Rust 优先** |

## 方案 B 技术细节

### 依赖配置

```toml
[features]
default = ["tui"]
local-embedding = [
    "dep:candle-core",
    "dep:candle-nn",
    "dep:candle-transformers",
    "dep:tokenizers",
    "dep:hf-hub",
]

[dependencies]
candle-core = { version = "0.10", optional = true }
candle-nn = { version = "0.10", optional = true }
candle-transformers = { version = "0.10", optional = true }
tokenizers = { version = "0.22", default-features = false, features = [], optional = true }
hf-hub = { version = "0.5", optional = true }
```

> `tokenizers` 关闭 `default-features` 可避免 `onig` C 库依赖。BERT WordPiece tokenizer 不依赖 regex 引擎。

### 模型加载流程

```rust
use candle_core::{Device, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::bert::{BertModel, Config};
use hf_hub::api::sync::Api;
use tokenizers::Tokenizer;

fn load_model() -> anyhow::Result<(BertModel, Tokenizer)> {
    let api = Api::new()?;
    let repo = api.model("sentence-transformers/all-MiniLM-L6-v2".to_string());
    
    let config_path = repo.get("config.json")?;
    let tokenizer_path = repo.get("tokenizer.json")?;
    let weights_path = repo.get("model.safetensors")?;
    
    let config: Config = serde_json::from_reader(std::fs::File::open(config_path)?)?;
    let tokenizer = Tokenizer::from_file(tokenizer_path).map_err(|e| anyhow::anyhow!(e))?;
    
    let device = Device::Cpu;
    let vb = unsafe { VarBuilder::from_mmaped_safetensors(&[weights_path], candle_core::DType::F32, &device)? };
    let model = BertModel::load(vb, &config)?;
    
    Ok((model, tokenizer))
}
```

### Embedding 生成（含 mean pooling + L2 norm）

```rust
fn encode(model: &BertModel, tokenizer: &Tokenizer, text: &str) -> anyhow::Result<Vec<f32>> {
    let encoding = tokenizer.encode(text, true).map_err(|e| anyhow::anyhow!(e))?;
    let input_ids = encoding.get_ids();
    let attention_mask = encoding.get_attention_mask();
    
    let input_ids = Tensor::new(input_ids, &model.device)?.unsqueeze(0)?;
    let token_type_ids = input_ids.zeros_like()?;
    let attention_mask = Tensor::new(attention_mask, &model.device)?.unsqueeze(0)?;
    
    let output = model.forward(&input_ids, &token_type_ids, Some(&attention_mask))?;
    // output: [1, seq_len, hidden_size]
    
    // Mean pooling: average over non-padding tokens
    let mask = attention_mask.to_dtype(candle_core::DType::F32)?.unsqueeze(2)?;
    let sum = (output.broadcast_mul(&mask)?)?.sum(1)?;
    let count = mask.sum(1)?;
    let mean_pooled = sum.broadcast_div(&count)?;
    
    // L2 normalize
    let norm = mean_pooled.sqr()?.sum_keepdim(1)?.sqrt()?;
    let normalized = mean_pooled.broadcast_div(&norm)?;
    
    Ok(normalized.squeeze(0)?.to_vec1()?)
}
```

### 缓存策略

- `hf-hub` 默认缓存路径: `~/.cache/huggingface/hub/`
- 首次下载后离线可用
- 模型文件总计 ~23MB

## Binary 大小估算

| 方案 | 当前 binary | 增量 | 预期总大小 |
|------|------------|------|-----------|
| baseline (release) | 24.3 MB | — | 24.3 MB |
| `ort` + ONNX Runtime | — | +10-20MB (DLL) + 22MB (model) | 34-46MB + 22MB model |
| `candle` (静态链接) | — | +5-8MB | ~30-32MB + 22MB model |

> model 文件不计入 binary，运行时按需加载。

## 风险与缓解

| 风险 | 可能性 | 缓解措施 |
|------|--------|---------|
| `tokenizers` default-features=false 导致 BERT tokenizer 异常 | 中 | 集成测试验证; 异常时开启 `onig` |
| `candle` CPU 推理速度不足 | 低 | MiniLM-L6 单次 <50ms; 可缓存常用 query |
| `hf-hub` 首次下载失败 | 低 | 提供手动下载指令;  graceful 回退到当前 Python 路径 |
| Windows 编译 `candle` 依赖问题 | 低 | 本地已验证 cache 存在; CI 验证 |

## 推荐决策

**采用方案 B (`candle` + `tokenizers`)**

理由:
1. 纯 Rust 栈契合 Hard Veto 和项目"Local Context Compiler"哲学
2. 无 DLL 分发问题，单 binary 部署
3. `hf-hub` 自动缓存模型，首次运行透明下载
4. 推理性能对 embedding 场景完全足够
5. 代码可控（~80 行），不引入黑盒 C++ runtime

## 实施步骤 (v0.14)

1. Cargo.toml 添加 `local-embedding` feature 和依赖
2. 新建 `src/embedding/local.rs` — candle 模型加载 + encode
3. 修改 `src/embedding.rs` — feature-gated 路由 (`local-embedding` 优先，否则 Python 回退)
4. 集成测试: 验证 embedding 维度 = 384，与 Python 输出余弦相似度 > 0.999
5. 更新 `project_context` — `goal` 参数启用时自动生成 query embedding

---

## 2026-05-04 补充：batch encoding 实验与知识蒸馏参考

### batch encoding 实验（Candle CPU BERT）

**假设**：batch_size=32 可提升 BERT 吞吐量，减少 1572 次单条 forward 的 overhead。
**结果**：batch encoding **比单条慢 5.2 倍**（88s vs 16s）。

| 方案 | embed 时间 | total 时间 | symbols |
|------|-----------|-----------|---------|
| 单条并行 (`rayon::par_iter()`) | 15,941ms | 16,322ms | 1573 |
| batch=32 (`encode_batch_with_candle`) | 88,259ms | 88,987ms | 1573 |
| `--skip-embeddings` | 0ms | 245ms | 1573 |

**根因分析**：
- Candle CPU 矩阵乘法对大 padded batch 不友好
- batch 内序列长度差异导致 pad 到 max_len，处理大量无效 token
- 每 batch forward ~1.7s，而单条仅 ~10ms；49 batches × 1.7s >> 1572 × 10ms

**决策**：
- `generate_and_save_embeddings` 回滚到 `rayon::par_iter()` 单条编码
- 保留 `EmbeddingProvider::encode_batch` trait 方法供未来 GPU/ONNX provider
- 新增 `--skip-embeddings` CLI 标志：纯符号/调用图索引跳过 embedding（16s → 0.25s）

### 外部文献参考：知识蒸馏 Pipeline

> 来源：Kimi 会话研究（`kimi.com/share/19e06760-97a2-84ea-8000-00006aaf036d`）

**`distill-knowledge` Skill 设计规格**（六阶段 Pipeline）：
1. **噪声过滤**：去除 UI 元素、元数据噪音、重复路径、广告语句、时间戳
2. **语义块分割**：切分为最小知识单元（项目/概念/论证）
3. **主题聚类**：一级主题 ≤6，二级分类 ≤4（奥卡姆剃刀）
4. **层级展开**：三级知识点表格化（项目|核心特征|可信度）
5. **可信度标注**：🟡 文档验证 / ⚠️ 社区传播 / ❓ 设想级
6. **结构化输出**：Markdown 知识库（目录锚点/统计区块/跨域规律附录）

**与 devbase 的对接点**：
- Vault 笔记系统可作为知识蒸馏的**输出存储**（`devkit_vault_write`）
- `devkit_paper_index`（PDF）可扩展为通用文本知识蒸馏入口
- MCP Server 可暴露 `devkit_knowledge_distill` 工具，供 Clarity 等 Agent 调用做预处理
- 噪声过滤规则库可直接嵌入 vault 索引流程，提升笔记质量

**状态**：设计规格级，待 Clarity 侧落地验证后评估 devbase 集成优先级。
