// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (c) 2026 juice094

use crate::EmbeddingProvider;

pub struct CandleProvider;

impl EmbeddingProvider for CandleProvider {
    fn encode(&self, text: &str) -> anyhow::Result<Vec<f32>> {
        let (model, tokenizer) = get_candle_resources()?;
        encode_with_candle(model, tokenizer, text)
    }
    fn encode_batch(&self, texts: &[&str]) -> anyhow::Result<Vec<Vec<f32>>> {
        let (model, tokenizer) = get_candle_resources()?;
        encode_batch_with_candle(model, tokenizer, texts)
    }
    fn name(&self) -> &'static str {
        "candle-all-MiniLM-L6-v2"
    }
}

fn get_candle_resources()
-> anyhow::Result<&'static (candle_transformers::models::bert::BertModel, tokenizers::Tokenizer)> {
    use std::sync::OnceLock;
    static RESOURCES: OnceLock<
        Result<(candle_transformers::models::bert::BertModel, tokenizers::Tokenizer), String>,
    > = OnceLock::new();
    match RESOURCES.get_or_init(|| init_candle_resources().map_err(|e| e.to_string())) {
        Ok(r) => Ok(r),
        Err(e) => Err(anyhow::anyhow!("CandleProvider init failed: {}", e)),
    }
}

fn init_candle_resources()
-> anyhow::Result<(candle_transformers::models::bert::BertModel, tokenizers::Tokenizer)> {
    use candle_core::Device;
    use candle_nn::VarBuilder;
    use candle_transformers::models::bert::{BertModel, Config};
    use hf_hub::api::sync::Api;
    use tokenizers::Tokenizer;

    let api = Api::new()?;
    let repo = api.model("sentence-transformers/all-MiniLM-L6-v2".to_string());

    let config_path = repo.get("config.json")?;
    let tokenizer_path = repo.get("tokenizer.json")?;
    let weights_path = repo.get("model.safetensors")?;

    let config: Config = serde_json::from_reader(std::fs::File::open(config_path)?)?;
    let tokenizer = Tokenizer::from_file(tokenizer_path).map_err(|e| anyhow::anyhow!(e))?;

    let device = Device::Cpu;
    let vb = unsafe {
        // SAFETY: model.safetensors is read-only after hf-hub download;
        // no other process modifies it. This is the standard candle loading pattern.
        VarBuilder::from_mmaped_safetensors(&[weights_path], candle_core::DType::F32, &device)?
    };
    let model = BertModel::load(vb, &config)?;

    Ok((model, tokenizer))
}

fn encode_with_candle(
    model: &candle_transformers::models::bert::BertModel,
    tokenizer: &tokenizers::Tokenizer,
    text: &str,
) -> anyhow::Result<Vec<f32>> {
    encode_batch_with_candle(model, tokenizer, &[text])
        .and_then(|mut v| v.pop().ok_or_else(|| anyhow::anyhow!("empty embedding batch")))
}

fn encode_batch_with_candle(
    model: &candle_transformers::models::bert::BertModel,
    tokenizer: &tokenizers::Tokenizer,
    texts: &[&str],
) -> anyhow::Result<Vec<Vec<f32>>> {
    use candle_core::Tensor;
    if texts.is_empty() {
        return Ok(Vec::new());
    }

    // Batch tokenize
    let encodings = tokenizer.encode_batch(texts.to_vec(), true).map_err(|e| anyhow::anyhow!(e))?;

    // Find max length for padding
    let max_len = encodings.iter().map(|e| e.get_ids().len()).max().unwrap_or(0);

    // Build padded batch tensors
    let mut input_ids_vec = Vec::new();
    let mut attention_mask_vec = Vec::new();
    for encoding in &encodings {
        let ids = encoding.get_ids();
        let mask = encoding.get_attention_mask();
        let mut padded_ids = ids.to_vec();
        let mut padded_mask = mask.to_vec();
        padded_ids.resize(max_len, 0);
        padded_mask.resize(max_len, 0);
        input_ids_vec.extend(padded_ids);
        attention_mask_vec.extend(padded_mask);
    }

    let batch_size = texts.len();
    let input_ids = Tensor::new(input_ids_vec, &model.device)?.reshape((batch_size, max_len))?;
    let token_type_ids = input_ids.zeros_like()?;
    let attention_mask_t =
        Tensor::new(attention_mask_vec, &model.device)?.reshape((batch_size, max_len))?;

    // Single forward pass for the whole batch
    let output = model.forward(&input_ids, &token_type_ids, Some(&attention_mask_t))?;

    // Mean pooling + L2 normalize per sample
    let mask = attention_mask_t.to_dtype(candle_core::DType::F32)?.unsqueeze(2)?;
    let sum = output.broadcast_mul(&mask)?.sum(1)?;
    let count = mask.sum(1)?;
    let mean_pooled = sum.broadcast_div(&count)?;

    let norm = mean_pooled.sqr()?.sum_keepdim(1)?.sqrt()?;
    let normalized = mean_pooled.broadcast_div(&norm)?;

    // Extract per-sample embeddings
    let mut results = Vec::with_capacity(batch_size);
    for i in 0..batch_size {
        let emb = normalized.get(i)?.squeeze(0)?.to_vec1()?;
        results.push(emb);
    }
    Ok(results)
}
