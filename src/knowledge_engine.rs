use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use crate::registry::{RepoEntry, WorkspaceRegistry};

#[derive(Debug, Clone)]
pub struct ModuleInfo {
    pub name: String,
    pub kind: String, // "lib", "bin", "test", "example", "unknown"
}

/// Run an async future from a synchronous context safely.
///
/// If already inside a tokio runtime (e.g. `spawn_blocking`), spawns the
/// future onto that runtime and blocks the current thread on a std channel.
/// If outside any runtime, creates a temporary runtime.
pub(crate) fn block_on_async<T>(
    future: impl std::future::Future<Output = T> + Send + 'static,
) -> Option<T>
where
    T: Send + 'static,
{
    match tokio::runtime::Handle::try_current() {
        Ok(handle) => {
            let (tx, rx) = std::sync::mpsc::channel();
            handle.spawn(async move {
                let _ = tx.send(future.await);
            });
            rx.recv().ok()
        }
        Err(_) => {
            let rt = tokio::runtime::Runtime::new().ok()?;
            Some(rt.block_on(future))
        }
    }
}

/// 从 README 中提取摘要和关键词
/// - 查找 README.md / README.rst / README（不区分大小写）
/// - 读取文件内容，提取第一个非空段落（跳过标题行 # ...）
/// - summary = 该段落的前 200 字符（截断到完整句子）
/// - keywords = 基于词频的 top-5 关键词（见下方说明）
pub fn extract_readme_summary(path: &Path) -> Option<(String, Vec<String>)> {
    let readme = find_readme(path)?;
    let content = std::fs::read_to_string(&readme).ok()?;

    let summary = extract_summary(&content)?;
    let keywords = extract_keywords(&content);

    Some((summary, keywords))
}

fn find_readme(path: &Path) -> Option<std::path::PathBuf> {
    let entries = std::fs::read_dir(path).ok()?;
    for entry in entries {
        let entry = entry.ok()?;
        let name = entry.file_name().to_string_lossy().to_lowercase();
        if name == "readme.md" || name == "readme.rst" || name == "readme" {
            return Some(entry.path());
        }
    }
    None
}

fn extract_summary(content: &str) -> Option<String> {
    let normalized = content.replace("\r\n", "\n");
    let paragraphs: Vec<&str> =
        normalized.split("\n\n").map(|p| p.trim()).filter(|p| !p.is_empty()).collect();

    for para in paragraphs {
        let trimmed = para.trim();
        if is_invalid_paragraph(trimmed) {
            continue;
        }
        // 跳过只包含标题标记、分隔线、数字的段落
        let first_line = trimmed.lines().next().unwrap_or("").trim();
        if first_line.starts_with('#')
            || first_line.starts_with("==")
            || first_line.starts_with("--")
            || first_line
                .chars()
                .all(|c| c.is_ascii_digit() || c.is_whitespace() || c == '.' || c == ')')
        {
            // 但如果段落本身有不止一行且后续行是有效文本，则仍然可用
            let lines: Vec<&str> = trimmed
                .lines()
                .skip(1)
                .map(|l| l.trim())
                .filter(|l| !l.is_empty() && !is_invalid_paragraph(l))
                .collect();
            if !lines.is_empty() {
                let body = lines.join(" ");
                return Some(truncate_to_sentence(&body, 200));
            }
            continue;
        }

        let cleaned = strip_html_tags(trimmed);
        return Some(truncate_to_sentence(&cleaned, 200));
    }
    None
}

fn is_badge_line(line: &str) -> bool {
    // Badge 通常形如 [![build](url)](url) 或连续多个 badge
    line.starts_with("[![") || line.starts_with("[! [")
}

fn is_reference_definition(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.starts_with('[') && trimmed.contains("]:") && trimmed.contains("://")
}

fn is_markup_only_line(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.starts_with("<img ")
        || trimmed.starts_with("<div")
        || trimmed.starts_with("<span")
        || trimmed.starts_with("<p")
        || trimmed.starts_with("<a ")
        || trimmed.starts_with("![")
        || trimmed.starts_with("[![")
        || (trimmed.starts_with('<') && trimmed.ends_with('>') && trimmed.len() < 80)
}

fn is_invalid_paragraph(para: &str) -> bool {
    let trimmed = para.trim();
    if trimmed.is_empty() {
        return true;
    }
    // 如果段落所有非空行都是 badge、HTML 标签、图片、reference 定义或纯链接，则跳过
    let lines: Vec<&str> = trimmed.lines().map(|l| l.trim()).filter(|l| !l.is_empty()).collect();
    if lines.is_empty() {
        return true;
    }
    lines.iter().all(|line| {
        is_badge_line(line)
            || is_markup_only_line(line)
            || is_reference_definition(line)
            || (line.starts_with('[') && line.contains("](")) // simple markdown link
    })
}

fn strip_html_tags(text: &str) -> String {
    let mut result = text.to_string();
    loop {
        let start = result.find('<');
        let end = start.and_then(|s| result[s..].find('>').map(|e| s + e));
        match (start, end) {
            (Some(s), Some(e)) => {
                result.replace_range(s..=e, " ");
            }
            _ => break,
        }
    }
    result.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn truncate_to_sentence(text: &str, max_chars: usize) -> String {
    let mut chars = text.chars();
    let mut result = String::new();
    let mut count = 0;
    let mut last_dot_pos = None;

    for ch in &mut chars {
        result.push(ch);
        count += 1;
        if ch == '.' {
            last_dot_pos = Some(result.len());
        }
        if count >= max_chars {
            break;
        }
    }

    if let Some(pos) = last_dot_pos {
        result.truncate(pos);
    }

    result.trim().to_string()
}

fn preprocess_for_keywords(content: &str) -> String {
    let mut result = content.to_string();
    // 移除 HTML 标签
    loop {
        let start = result.find('<');
        let end = start.and_then(|s| result[s..].find('>').map(|e| s + e));
        match (start, end) {
            (Some(s), Some(e)) => {
                result.replace_range(s..=e, " ");
            }
            _ => break,
        }
    }
    // 将 markdown 链接 [text](url) 替换为 text
    let mut processed = String::new();
    let mut rest = result.as_str();
    while let Some(start) = rest.find('[') {
        processed.push_str(&rest[..start]);
        rest = &rest[start..];
        if let Some(close) = rest.find("](") {
            let text = &rest[1..close];
            if let Some(url_end) = rest[close..].find(')') {
                processed.push_str(text);
                rest = &rest[close + url_end + 1..];
                continue;
            }
        }
        // 不是标准链接，保留 [
        processed.push('[');
        rest = &rest[1..];
    }
    processed.push_str(rest);
    // 移除 markdown 图片 ![alt](url)
    let mut result2 = String::new();
    let mut rest2 = processed.as_str();
    while let Some(start) = rest2.find("![") {
        result2.push_str(&rest2[..start]);
        rest2 = &rest2[start..];
        if let Some(url_end) = rest2.find(')') {
            rest2 = &rest2[url_end + 1..];
        } else {
            result2.push_str("![");
            break;
        }
    }
    result2.push_str(rest2);
    result2
}

fn extract_keywords(content: &str) -> Vec<String> {
    let content = preprocess_for_keywords(content);
    let stop_words: std::collections::HashSet<&str> = [
        "the",
        "a",
        "is",
        "to",
        "and",
        "of",
        "in",
        "for",
        "with",
        "on",
        "at",
        "by",
        "from",
        "as",
        "it",
        "this",
        "that",
        "be",
        "are",
        "was",
        "were",
        "has",
        "have",
        "had",
        "not",
        "but",
        "or",
        "an",
        "you",
        "we",
        "they",
        "he",
        "she",
        "will",
        "can",
        "may",
        "should",
        "would",
        "could",
        "project",
        "repository",
        "repo",
        "code",
        "software",
        "tool",
        "library",
        "crate",
        "package",
        "http",
        "https",
        "github",
        "gitlab",
        "com",
        "org",
        "net",
        "io",
        "www",
        "there",
        "here",
        "when",
        "where",
        "what",
        "how",
        "why",
        "who",
        "which",
        "their",
        "them",
        "then",
        "than",
        "also",
        "into",
        "out",
        "up",
        "only",
        "just",
        "now",
        "get",
        "use",
        "using",
        "used",
        "make",
        "made",
        "way",
        "new",
        "like",
        "over",
        "your",
        "our",
        "its",
        "see",
        "top",
        "via",
        "every",
        "being",
        "before",
        "after",
        "above",
        "below",
        "blob",
        "tree",
    ]
    .iter()
    .cloned()
    .collect();

    let mut freq: HashMap<String, usize> = HashMap::new();

    for word in content.split(|c: char| !c.is_alphanumeric()) {
        let word = word.to_lowercase();
        if word.len() <= 2 || stop_words.contains(word.as_str()) {
            continue;
        }
        *freq.entry(word).or_insert(0) += 1;
    }

    let mut pairs: Vec<(String, usize)> = freq.into_iter().collect();
    pairs.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));

    pairs.into_iter().take(5).map(|(w, _)| w).collect()
}

/// 当 README 不存在时，基于项目元数据生成规则摘要
pub fn generate_fallback_summary(path: &Path) -> (String, String) {
    // Try language-specific metadata files
    if let Some((summary, keywords)) = try_cargo_toml(path) {
        return (summary, keywords);
    }
    if let Some((summary, keywords)) = try_package_json(path) {
        return (summary, keywords);
    }
    if let Some((summary, keywords)) = try_go_mod(path) {
        return (summary, keywords);
    }
    if let Some((summary, keywords)) = try_pyproject(path) {
        return (summary, keywords);
    }

    // Last resort: file type distribution
    let mut counts: HashMap<String, usize> = HashMap::new();
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            if let Some(ext) = entry.path().extension().and_then(|e| e.to_str()) {
                *counts.entry(ext.to_lowercase()).or_insert(0) += 1;
            }
        }
    }
    let mut pairs: Vec<(String, usize)> = counts.into_iter().collect();
    pairs.sort_by(|a, b| b.1.cmp(&a.1));
    let top: Vec<String> =
        pairs.into_iter().take(3).map(|(e, c)| format!("{}({})", e, c)).collect();
    if top.is_empty() {
        ("Unclassified project".to_string(), "unknown".to_string())
    } else {
        (format!("Project containing files: {}", top.join(", ")), top.join(", "))
    }
}

fn try_cargo_toml(path: &Path) -> Option<(String, String)> {
    let cargo_path = path.join("Cargo.toml");
    let cargo = std::fs::read_to_string(&cargo_path).ok()?;
    let value: toml::Value = toml::from_str(&cargo).ok()?;
    let package = value.get("package")?;
    let desc = package.get("description")?.as_str()?;
    let name = package.get("name")?.as_str()?;
    let workspace_members: Vec<String> = value
        .get("workspace")
        .and_then(|w| w.get("members"))
        .and_then(|m| m.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
        .unwrap_or_default();
    let summary = if workspace_members.len() > 1 {
        format!(
            "Rust workspace '{}' with {} crates: {}",
            name,
            workspace_members.len(),
            workspace_members.join(", ")
        )
    } else {
        format!("Rust crate '{}' - {}", name, desc)
    };
    let keywords =
        format!("rust, {}, {}", name, workspace_members.first().cloned().unwrap_or_default());
    Some((summary, keywords))
}

fn try_package_json(path: &Path) -> Option<(String, String)> {
    let content = std::fs::read_to_string(path.join("package.json")).ok()?;
    let value: serde_json::Value = serde_json::from_str(&content).ok()?;
    let name = value.get("name")?.as_str()?;
    let desc = value.get("description")?.as_str().unwrap_or("Node.js project");
    let summary = format!("Node project '{}' - {}", name, desc);
    Some((summary, format!("node, {}, javascript", name)))
}

fn try_go_mod(path: &Path) -> Option<(String, String)> {
    let content = std::fs::read_to_string(path.join("go.mod")).ok()?;
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with("module ") {
            let module_name = line.strip_prefix("module ")?.trim();
            let summary = format!("Go module '{}'", module_name);
            return Some((summary, format!("go, {}", module_name)));
        }
    }
    None
}

fn try_pyproject(path: &Path) -> Option<(String, String)> {
    let content = std::fs::read_to_string(path.join("pyproject.toml")).ok()?;
    let value: toml::Value = toml::from_str(&content).ok()?;
    let project = value.get("project")?;
    let name = project.get("name")?.as_str()?;
    let desc = project.get("description")?.as_str().unwrap_or("Python project");
    let summary = format!("Python project '{}' - {}", name, desc);
    Some((
        summary,
        format!("python, {}, {}", name, desc.split_whitespace().next().unwrap_or("")),
    ))
}

/// 从 Rust 项目中提取模块结构
/// - 调用 `cargo metadata --format-version 1 --manifest-path <path>/Cargo.toml --no-deps`
/// - 解析 JSON，提取每个 package 的 targets[].name 和 targets[].kind[0]
/// - 非 Rust 项目返回空 Vec
pub fn extract_module_structure(path: &Path) -> Vec<ModuleInfo> {
    let manifest = path.join("Cargo.toml");
    if !manifest.exists() {
        return Vec::new();
    }

    let output = Command::new("cargo")
        .args([
            "metadata",
            "--format-version",
            "1",
            "--manifest-path",
            manifest.to_string_lossy().as_ref(),
            "--no-deps",
        ])
        .output();

    let output = match output {
        Ok(o) if o.status.success() => o,
        _ => return Vec::new(),
    };

    let json: serde_json::Value = match serde_json::from_slice(&output.stdout) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };

    let mut modules = Vec::new();
    let packages = json.get("packages").and_then(|v| v.as_array()).cloned().unwrap_or_default();
    for pkg in packages {
        let targets = pkg.get("targets").and_then(|v| v.as_array()).cloned().unwrap_or_default();
        for target in targets {
            let name = target.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let kind = target
                .get("kind")
                .and_then(|v| v.as_array())
                .and_then(|arr| arr.first())
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();
            modules.push(ModuleInfo { name, kind });
        }
    }

    modules
}

fn build_llm_prompt(context: &str) -> String {
    format!(
        r#"Analyze the following project context and produce a JSON object with exactly two string fields: \"summary\" (a one-sentence description of the project) and \"keywords\" (a comma-separated list of relevant tags).

Context:
{}

Respond with only the JSON object, no extra text."#,
        context
    )
}

fn parse_llm_json(text: &str) -> Option<(String, String)> {
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

async fn call_llm(
    api_key: &str,
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
    let response = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await?;
    let status = response.status();
    let json: serde_json::Value = response.json().await?;
    if !status.is_success() {
        anyhow::bail!("LLM API error: {}", json["error"]["message"].as_str().unwrap_or("unknown"));
    }
    let content = json["choices"][0]["message"]["content"].as_str().unwrap_or("").to_string();
    Ok(content)
}

fn try_llm_summary(path: &Path, config: &crate::config::LlmConfig) -> Option<(String, String)> {
    if !config.enabled {
        return None;
    }

    let mut context = if let Some(readme) = find_readme(path) {
        std::fs::read_to_string(&readme)
            .map(|c| c.chars().take(3000).collect::<String>())
            .unwrap_or_default()
    } else {
        String::new()
    };

    if context.is_empty() {
        context = if let Some((summary, _)) = try_cargo_toml(path) {
            summary
        } else if let Some((summary, _)) = try_package_json(path) {
            summary
        } else if let Some((summary, _)) = try_go_mod(path) {
            summary
        } else if let Some((summary, _)) = try_pyproject(path) {
            summary
        } else {
            return None;
        };
    }

    let api_key = config.api_key.clone()?;
    let (base_url, model) = match config.provider.as_str() {
        "deepseek" => (
            config
                .base_url
                .clone()
                .unwrap_or_else(|| "https://api.deepseek.com/v1".to_string()),
            config.model.clone().unwrap_or_else(|| "deepseek-chat".to_string()),
        ),
        "kimi" => (
            config
                .base_url
                .clone()
                .unwrap_or_else(|| "https://api.moonshot.cn/v1".to_string()),
            config.model.clone().unwrap_or_else(|| "kimi-k2-07132k".to_string()),
        ),
        "openai" => (
            config
                .base_url
                .clone()
                .unwrap_or_else(|| "https://api.openai.com/v1".to_string()),
            config.model.clone().unwrap_or_else(|| "gpt-4o".to_string()),
        ),
        "dashscope" => (
            config
                .base_url
                .clone()
                .unwrap_or_else(|| "https://dashscope.aliyuncs.com/compatible-mode/v1".to_string()),
            config.model.clone().unwrap_or_else(|| "qwen-max".to_string()),
        ),
        _ => return None,
    };

    let prompt = build_llm_prompt(&context);
    let timeout = config.timeout_seconds;
    let max_tokens = config.max_tokens;
    let result = block_on_async(async move {
        tokio::time::timeout(
            Duration::from_secs(timeout),
            call_llm(&api_key, &base_url, &model, &prompt, max_tokens),
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

fn index_repo_in_search(
    repo: &crate::registry::RepoEntry,
    summary: &str,
    keywords: &str,
) -> anyhow::Result<()> {
    let (index, _reader) = crate::search::init_index()?;
    let mut writer = crate::search::get_writer(&index)?;
    let schema = index.schema();
    crate::search::delete_repo_doc(&mut writer, &schema, &repo.id)?;
    crate::search::add_repo_doc(&mut writer, &schema, &repo.id, summary, keywords, &repo.tags)?;
    crate::search::commit_writer(&mut writer)?;
    Ok(())
}

pub fn index_repo(repo: &crate::registry::RepoEntry) -> anyhow::Result<()> {
    use tracing::{info, warn};

    let mut conn = WorkspaceRegistry::init_db()?;

    let config = crate::config::Config::load().ok();
    let (summary, keywords) = config
        .as_ref()
        .and_then(|cfg| try_llm_summary(&repo.local_path, &cfg.llm))
        .or_else(|| extract_readme_summary(&repo.local_path).map(|(s, k)| (s, k.join(", "))))
        .unwrap_or_else(|| {
            warn!("No README found for {}, generating fallback summary", repo.id);
            generate_fallback_summary(&repo.local_path)
        });

    let modules = extract_module_structure(&repo.local_path);

    WorkspaceRegistry::save_summary(&conn, &repo.id, &summary, &keywords)?;

    if let Err(e) = index_repo_in_search(repo, &summary, &keywords) {
        warn!("Failed to index repo in search: {}", e);
    }

    let modules_tuple: Vec<(String, String)> =
        modules.into_iter().map(|m| (m.name, m.kind)).collect();
    WorkspaceRegistry::save_modules(&mut conn, &repo.id, &modules_tuple)?;

    let detected_lang = crate::scan::detect_language(&repo.local_path);
    if let Some(ref lang) = detected_lang {
        WorkspaceRegistry::update_repo_language(&conn, &repo.id, Some(lang))?;
    }

    info!(
        "Indexed [{}] -> \"{}\" (keywords: {}) language={:?}",
        repo.id, summary, keywords, detected_lang
    );
    Ok(())
}

/// 兼容旧调用的包装层：执行索引逻辑
pub fn run_index(path: &str) -> anyhow::Result<usize> {
    use tracing::{info, warn};

    let mut conn = WorkspaceRegistry::init_db()?;

    let repos: Vec<RepoEntry> = if path.is_empty() {
        WorkspaceRegistry::list_repos(&conn)?
    } else {
        let p = PathBuf::from(path);
        if !p.exists() {
            anyhow::bail!("Path does not exist: {}", path);
        }
        let registered = WorkspaceRegistry::list_repos(&conn)?;
        if let Some(repo) = registered.into_iter().find(|r| r.local_path == p) {
            vec![repo]
        } else {
            info!("Registering {} before indexing", path);
            let repo = crate::scan::inspect_repo(&p, None)?;
            WorkspaceRegistry::save_repo(&mut conn, &repo)?;
            vec![repo]
        }
    };

    // Initialize Tantivy search index writer once for the batch
    let (search_index, _reader) = crate::search::init_index()?;
    let mut search_writer = crate::search::get_writer(&search_index)?;
    let search_schema = search_index.schema();

    let mut count = 0;
    for repo in &repos {
        let config = crate::config::Config::load().ok();
        let (summary, keywords) = config
            .as_ref()
            .and_then(|cfg| try_llm_summary(&repo.local_path, &cfg.llm))
            .or_else(|| extract_readme_summary(&repo.local_path).map(|(s, k)| (s, k.join(", "))))
            .unwrap_or_else(|| {
                warn!("No README found for {}, generating fallback summary", repo.id);
                generate_fallback_summary(&repo.local_path)
            });

        let modules = extract_module_structure(&repo.local_path);

        WorkspaceRegistry::save_summary(&conn, &repo.id, &summary, &keywords)?;

        // Add/update repo document in Tantivy index
        crate::search::delete_repo_doc(&mut search_writer, &search_schema, &repo.id)?;
        crate::search::add_repo_doc(
            &mut search_writer,
            &search_schema,
            &repo.id,
            &summary,
            &keywords,
            &repo.tags,
        )?;

        let modules_tuple: Vec<(String, String)> =
            modules.into_iter().map(|m| (m.name, m.kind)).collect();
        WorkspaceRegistry::save_modules(&mut conn, &repo.id, &modules_tuple)?;

        let detected_lang = crate::scan::detect_language(&repo.local_path);
        if let Some(ref lang) = detected_lang {
            WorkspaceRegistry::update_repo_language(&conn, &repo.id, Some(lang))?;
        }

        // Semantic code indexing (tree-sitter AST extraction + call graph)
        let (symbols, calls) = crate::semantic_index::index_repo_full(&repo.local_path);
        if !symbols.is_empty() {
            match crate::semantic_index::save_symbols(&mut conn, &repo.id, &symbols) {
                Ok(n) => info!("Saved {} code symbols for {}", n, repo.id),
                Err(e) => warn!("Failed to save code symbols for {}: {}", repo.id, e),
            }
        }
        if !calls.is_empty() {
            match crate::semantic_index::save_calls(&mut conn, &repo.id, &calls) {
                Ok(n) => info!("Saved {} call edges for {}", n, repo.id),
                Err(e) => warn!("Failed to save call graph for {}: {}", repo.id, e),
            }
        }

        // Semantic embeddings for vector search
        let emb_config = crate::config::Config::load().ok().map(|c| c.embedding);
        if let Some(ref ec) = emb_config
            && ec.enabled
        {
            let func_symbols: Vec<&crate::semantic_index::CodeSymbol> = symbols
                .iter()
                .filter(|s| s.symbol_type == crate::semantic_index::SymbolType::Function)
                .collect();
            if !func_symbols.is_empty() {
                let texts: Vec<String> = func_symbols
                    .iter()
                    .map(|s| {
                        let sig = s.signature.as_deref().unwrap_or(&s.name);
                        format!("{} in {}: {}", s.name, s.file_path.display(), sig)
                    })
                    .collect();
                let ec = ec.clone();
                let embs = block_on_async(async move {
                    crate::embedding::generate_embeddings(&texts, &ec).await
                })
                .unwrap_or_default();
                if !embs.is_empty() {
                    let pairs: Vec<(String, Vec<f32>)> = func_symbols
                        .iter()
                        .zip(embs.into_iter())
                        .map(|(s, e)| (s.name.clone(), e))
                        .collect();
                    match crate::registry::WorkspaceRegistry::save_embeddings(
                        &mut conn, &repo.id, &pairs,
                    ) {
                        Ok(n) => info!("Saved {} embeddings for {}", n, repo.id),
                        Err(e) => warn!("Failed to save embeddings for {}: {}", repo.id, e),
                    }
                }
            }
        }

        // Cross-repo dependency graph
        match crate::dependency_graph::build_dependency_graph(&mut conn, &repo.id, &repo.local_path)
        {
            Ok(n) => {
                if n > 0 {
                    info!("Resolved {} local dependencies for {}", n, repo.id);
                }
            }
            Err(e) => warn!("Failed to build dependency graph for {}: {}", repo.id, e),
        }

        println!(
            "Indexed [{}] -> \"{}\" (keywords: {}) language={:?} symbols={} calls={} embeddings={}",
            repo.id,
            summary,
            keywords,
            detected_lang,
            symbols.len(),
            calls.len(),
            emb_config
                .as_ref()
                .filter(|e| e.enabled)
                .map(|_| {
                    symbols
                        .iter()
                        .filter(|s| s.symbol_type == crate::semantic_index::SymbolType::Function)
                        .count()
                })
                .unwrap_or(0)
        );
        count += 1;
    }

    crate::search::commit_writer(&mut search_writer)?;

    println!("\nIndexed {} repositories.", count);
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_extract_readme_summary_basic() {
        let dir = tempfile::tempdir().unwrap();
        let readme = dir.path().join("README.md");
        let mut file = std::fs::File::create(&readme).unwrap();
        write!(
            file,
            r#"# MyProject

This is a blazing fast terminal user interface for git. It provides an intuitive way to manage branches, commits, and staging areas directly from the command line. Additional features include custom themes and keyboard shortcuts.

## Features

- fast
- safe
"#
        )
        .unwrap();

        let (summary, keywords) = extract_readme_summary(dir.path()).unwrap();
        assert!(summary.contains("This is a blazing fast terminal user interface for git"));
        assert!(summary.len() <= 210); // 允许一些余量
        assert!(!keywords.is_empty() && keywords.len() <= 5);
        // 验证关键词是从有效词汇中提取的（至少包含一些内容词）
        let content_words =
            ["git", "fast", "terminal", "branches", "commits", "features", "custom"];
        assert!(keywords.iter().any(|k| content_words.contains(&k.as_str())));
    }

    #[test]
    fn test_extract_readme_summary_with_badges() {
        let dir = tempfile::tempdir().unwrap();
        let readme = dir.path().join("README.rst");
        let mut file = std::fs::File::create(&readme).unwrap();
        write!(
            file,
            r#"[![build](https://example.com)](https://example.com)
[![coverage](https://example.com)](https://example.com)

Gitui is an awesome tool written in rust. It supports async operations and handles large repositories with ease.

# Details

More info here.
"#
        )
        .unwrap();

        let (summary, keywords) = extract_readme_summary(dir.path()).unwrap();
        assert!(summary.contains("Gitui is an awesome tool written in rust"));
        assert!(!keywords.is_empty() && keywords.len() <= 5);
        let content_words = ["gitui", "rust", "awesome", "async", "operations", "repositories"];
        assert!(keywords.iter().any(|k| content_words.contains(&k.as_str())));
    }

    #[test]
    fn test_extract_readme_summary_truncates_at_sentence() {
        let dir = tempfile::tempdir().unwrap();
        let readme = dir.path().join("README.md");
        let mut file = std::fs::File::create(&readme).unwrap();
        let long_sentence = "A ".repeat(250);
        write!(
            file,
            r#"# Title

First sentence here. {}Second sentence here.
"#,
            long_sentence
        )
        .unwrap();

        let (summary, _) = extract_readme_summary(dir.path()).unwrap();
        assert!(summary.ends_with('.'));
        assert!(summary.contains("First sentence here."));
        assert!(!summary.contains("Second sentence here"));
    }

    #[test]
    fn test_fallback_summary_cargo_toml() {
        let dir = tempfile::tempdir().unwrap();
        let cargo = dir.path().join("Cargo.toml");
        let mut file = std::fs::File::create(&cargo).unwrap();
        write!(
            file,
            r#"[package]
name = "test-fallback-crate"
version = "0.1.0"
edition = "2021"
description = "A test crate for semantic fallback without README."
"#
        )
        .unwrap();

        let (summary, keywords) = generate_fallback_summary(dir.path());
        assert!(summary.contains("test-fallback-crate"), "summary: {}", summary);
        assert!(
            summary.contains("A test crate for semantic fallback without README"),
            "summary: {}",
            summary
        );
        assert!(
            keywords.contains("rust") || keywords.contains("test-fallback-crate"),
            "keywords: {}",
            keywords
        );
    }

    #[test]
    fn test_extract_module_structure_for_devbase() {
        // 使用当前项目（devbase）自身进行集成测试
        let path = Path::new(env!("CARGO_MANIFEST_DIR"));
        let modules = extract_module_structure(path);
        assert!(!modules.is_empty());
        let names: Vec<&str> = modules.iter().map(|m| m.name.as_str()).collect();
        assert!(names.contains(&"devbase"));
    }

    #[test]
    fn test_extract_module_structure_non_rust() {
        let dir = tempfile::tempdir().unwrap();
        let modules = extract_module_structure(dir.path());
        assert!(modules.is_empty());
    }

    #[test]
    fn test_try_llm_summary_disabled_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let config = crate::config::LlmConfig {
            enabled: false,
            provider: "ollama".to_string(),
            api_key: None,
            model: None,
            base_url: None,
            max_tokens: 200,
            timeout_seconds: 30,
        };
        assert!(try_llm_summary(dir.path(), &config).is_none());
    }

    #[test]
    fn test_parse_llm_json_valid() {
        let result = parse_llm_json(r#"{"summary":"A tool","keywords":"rust, cli"}"#).unwrap();
        assert_eq!(result.0, "A tool");
        assert_eq!(result.1, "rust, cli");
    }

    #[test]
    fn test_parse_llm_json_markdown_fenced() {
        let text = "```json\n{\"summary\":\"A tool\",\"keywords\":\"rust, cli\"}\n```";
        let result = parse_llm_json(text).unwrap();
        assert_eq!(result.0, "A tool");
        assert_eq!(result.1, "rust, cli");
    }

    #[test]
    fn test_build_llm_prompt_contains_json_instruction() {
        let prompt = build_llm_prompt("test context");
        assert!(prompt.contains("JSON object"));
        assert!(prompt.contains("summary"));
    }

    #[test]
    fn test_module_info_clone() {
        let m = ModuleInfo {
            name: "foo".to_string(),
            kind: "lib".to_string(),
        };
        let cloned = m.clone();
        assert_eq!(cloned.name, "foo");
        assert_eq!(cloned.kind, "lib");
    }

    #[test]
    #[ignore = "integration test on real gitui repo"]
    fn test_real_gitui_repo() {
        let path = Path::new(r"C:\Users\<user>\dev\third_party\gitui");
        if !path.exists() {
            return;
        }
        let (summary, keywords) = extract_readme_summary(path).unwrap();
        println!("gitui summary: {}", summary);
        println!("gitui keywords: {:?}", keywords);
        let modules = extract_module_structure(path);
        println!("gitui modules (first 10): {:?}", modules.iter().take(10).collect::<Vec<_>>());
        assert!(!summary.is_empty());
        assert!(!modules.is_empty());
    }

    #[test]
    #[ignore = "integration test on real syncthing repo"]
    fn test_real_syncthing_repo() {
        let path = Path::new(r"C:\Users\<user>\dev\third_party\syncthing");
        if !path.exists() {
            return;
        }
        let (summary, keywords) = extract_readme_summary(path).unwrap();
        println!("syncthing summary: {}", summary);
        println!("syncthing keywords: {:?}", keywords);
        let modules = extract_module_structure(path);
        println!(
            "syncthing modules (first 10): {:?}",
            modules.iter().take(10).collect::<Vec<_>>()
        );
        assert!(!summary.is_empty());
    }
}
