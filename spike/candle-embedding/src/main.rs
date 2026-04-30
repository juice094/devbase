use candle_core::{Device, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::bert::{BertModel, Config};
use hf_hub::api::sync::Api;
use tokenizers::Tokenizer;

fn main() -> anyhow::Result<()> {
    let api = Api::new()?;
    let repo = api.model("sentence-transformers/all-MiniLM-L6-v2".to_string());

    let config_path = repo.get("config.json")?;
    let tokenizer_path = repo.get("tokenizer.json")?;
    let weights_path = repo.get("model.safetensors")?;

    println!("config:   {:?}", config_path);
    println!("tokenizer:{:?}", tokenizer_path);
    println!("weights:  {:?}", weights_path);

    let config: Config = serde_json::from_reader(std::fs::File::open(config_path)?)?;
    let tokenizer = Tokenizer::from_file(tokenizer_path).map_err(|e| anyhow::anyhow!(e))?;

    let device = Device::Cpu;
    let vb = unsafe {
        VarBuilder::from_mmaped_safetensors(&[weights_path], candle_core::DType::F32, &device)?
    };
    let model = BertModel::load(vb, &config)?;

    let text = "This is a test sentence for embedding generation.";
    let embedding = encode(&model, &tokenizer, text)?;

    println!("embedding dim = {} (expected 384)", embedding.len());
    println!("first 5 values: {:?}", &embedding[..5.min(embedding.len())]);

    // Verify L2 norm ≈ 1.0 (sentence-transformers normalizes)
    let norm: f32 = embedding.iter().map(|v| v * v).sum::<f32>().sqrt();
    println!("L2 norm = {:.6} (expected ≈ 1.0)", norm);

    Ok(())
}

fn encode(model: &BertModel, tokenizer: &Tokenizer, text: &str) -> anyhow::Result<Vec<f32>> {
    let encoding = tokenizer.encode(text, true).map_err(|e| anyhow::anyhow!(e))?;
    let input_ids = encoding.get_ids();
    let attention_mask = encoding.get_attention_mask();

    let input_ids = Tensor::new(input_ids, &model.device)?.unsqueeze(0)?;
    let token_type_ids = input_ids.zeros_like()?;
    let attention_mask_t = Tensor::new(attention_mask, &model.device)?.unsqueeze(0)?;

    let output = model.forward(&input_ids, &token_type_ids, Some(&attention_mask_t))?;
    // output: [1, seq_len, hidden_size]

    // Mean pooling: average over non-padding tokens
    let mask = attention_mask_t
        .to_dtype(candle_core::DType::F32)?
        .unsqueeze(2)?;
    let sum = output.broadcast_mul(&mask)?.sum(1)?;
    let count = mask.sum(1)?;
    let mean_pooled = sum.broadcast_div(&count)?;

    // L2 normalize
    let norm = mean_pooled.sqr()?.sum_keepdim(1)?.sqrt()?;
    let normalized = mean_pooled.broadcast_div(&norm)?;

    Ok(normalized.squeeze(0)?.to_vec1()?)
}
