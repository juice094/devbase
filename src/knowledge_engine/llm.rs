use std::path::Path;
use std::time::Duration;

pub(crate) fn build_llm_prompt(context: &str) -> String {
    format!(
        r#"Analyze the following project context and produce a JSON object with exactly two string fields: \"summary\" (a one-sentence description of the project) and \"keywords\" (a comma-separated list of relevant tags).

Context:
{}

Respond with only the JSON object, no extra text."#,
        context
    )
}

pub(crate) fn parse_llm_json(text: &str) -> Option<(String, String)> {
    let trimmed = text.trim();
    let json_str = if trimmed.starts_with("```json") {
        trimmed.strip_prefix("```json").and_then(|s| s.strip_suffix("```"))?.trim()
    } else if trimmed.starts_with("```") {
        trimmed.strip_prefix("```").and_then(|s| s.strip_suffix("```"))?.trim()
    } else {
        trimmed
    };
    let value: serde_json::Value = serde_json::from_str(json_str).ok()?;
    let summary = value.get("summary")?.as_str()?.to_string();
    let keywords = value.get("keywords")?.as_str()?.to_string();
    if summary.is_empty() || keywords.is_empty() {
        return None;
    }
    Some((summary, keywords))
}

// TODO(veto-audit-2026-04-26): HV-1 数据外泄风险 — 此函数将用户 README 内容发送到外部 LLM API。
// 缓解: (1) 默认 enabled=false 已在 config 落地; (2) 已增加 ollama 本地分支（2026-04-26 修复）。
// 剩余: 用户仍需显式 opt-in（enabled=true + provider/api_key 配置）后才启用。
async fn call_llm(
    api_key: Option<&str>,
    base_url: &str,
    model: &str,
    prompt: &str,
    max_tokens: u32,
) -> anyhow::Result<String> {
    let client = reqwest::Client::builder().timeout(Duration::from_secs(60)).build()?;
    let url = format!("{}/chat/completions", base_url.trim_end_matches('/'));
    let body = serde_json::json!({
        "model": model,
        "messages": [{"role": "user", "content": prompt}],
        "max_tokens": max_tokens,
    });
    let mut request = client.post(&url).header("Content-Type", "application/json").json(&body);
    if let Some(key) = api_key {
        request = request.header("Authorization", format!("Bearer {}", key));
    }
    let response = request.send().await?;
    let status = response.status();
    let json: serde_json::Value = response.json().await?;
    if !status.is_success() {
        anyhow::bail!("LLM API error: {}", json["error"]["message"].as_str().unwrap_or("unknown"));
    }
    let content = json["choices"][0]["message"]["content"].as_str().unwrap_or("").to_string();
    Ok(content)
}

// TODO(veto-audit-2026-04-26): HV-1 自动数据外泄 — index_repo/run_index 自动调用此函数，
// 将 README (3000 chars) POST 到外部 LLM。虽需配置 api_key，但触发是自动非显式。
// 修复: 默认返回 None（enabled=false），用户 opt-in 后才启用。
pub fn try_llm_summary(path: &Path, config: &crate::config::LlmConfig) -> Option<(String, String)> {
    if !config.enabled {
        return None;
    }

    let mut context = if let Some(readme) = super::readme::find_readme(path) {
        std::fs::read_to_string(&readme)
            .map(|c| c.chars().take(3000).collect::<String>())
            .unwrap_or_default()
    } else {
        String::new()
    };

    if context.is_empty() {
        context = if let Some((summary, _)) = super::fallback::try_cargo_toml(path) {
            summary
        } else if let Some((summary, _)) = super::fallback::try_package_json(path) {
            summary
        } else if let Some((summary, _)) = super::fallback::try_go_mod(path) {
            summary
        } else if let Some((summary, _)) = super::fallback::try_pyproject(path) {
            summary
        } else {
            return None;
        };
    }

    let (base_url, model, api_key_opt) = match config.provider.as_str() {
        "deepseek" => {
            let key = config.api_key.clone()?;
            (
                config
                    .base_url
                    .clone()
                    .unwrap_or_else(|| "https://api.deepseek.com/v1".to_string()),
                config.model.clone().unwrap_or_else(|| "deepseek-chat".to_string()),
                Some(key),
            )
        }
        "kimi" => {
            let key = config.api_key.clone()?;
            (
                config
                    .base_url
                    .clone()
                    .unwrap_or_else(|| "https://api.moonshot.cn/v1".to_string()),
                config.model.clone().unwrap_or_else(|| "kimi-k2-07132k".to_string()),
                Some(key),
            )
        }
        "openai" => {
            let key = config.api_key.clone()?;
            (
                config
                    .base_url
                    .clone()
                    .unwrap_or_else(|| "https://api.openai.com/v1".to_string()),
                config.model.clone().unwrap_or_else(|| "gpt-4o".to_string()),
                Some(key),
            )
        }
        "dashscope" => {
            let key = config.api_key.clone()?;
            (
                config.base_url.clone().unwrap_or_else(|| {
                    "https://dashscope.aliyuncs.com/compatible-mode/v1".to_string()
                }),
                config.model.clone().unwrap_or_else(|| "qwen-max".to_string()),
                Some(key),
            )
        }
        "ollama" => (
            config
                .base_url
                .clone()
                .unwrap_or_else(|| "http://localhost:11434/v1".to_string()),
            config.model.clone().unwrap_or_else(|| "qwen2.5".to_string()),
            config.api_key.clone(), // Ollama does not require an API key
        ),
        _ => return None,
    };

    let prompt = build_llm_prompt(&context);
    let timeout = config.timeout_seconds;
    let max_tokens = config.max_tokens;
    let result = super::block_on_async(async move {
        tokio::time::timeout(
            Duration::from_secs(timeout),
            call_llm(api_key_opt.as_deref(), &base_url, &model, &prompt, max_tokens),
        )
        .await
    })?;

    match result {
        Ok(Ok(content)) => parse_llm_json(&content),
        Ok(Err(e)) => {
            tracing::debug!("LLM completion error: {}", e);
            None
        }
        Err(_) => {
            tracing::debug!("LLM completion timed out");
            None
        }
    }
}

