use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::registry::{RepoEntry, WorkspaceRegistry};

#[derive(Debug, Clone)]
pub struct ModuleInfo {
    pub name: String,
    pub kind: String, // "lib", "bin", "test", "example", "unknown"
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
    let paragraphs: Vec<&str> = normalized
        .split("\n\n")
        .map(|p| p.trim())
        .filter(|p| !p.is_empty())
        .collect();

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
        "the", "a", "is", "to", "and", "of", "in", "for", "with", "on", "at", "by", "from",
        "as", "it", "this", "that", "be", "are", "was", "were", "has", "have", "had", "not",
        "but", "or", "an", "you", "we", "they", "he", "she", "will", "can", "may", "should",
        "would", "could", "project", "repository", "repo", "code", "software", "tool",
        "library", "crate", "package", "http", "https", "github", "gitlab", "com", "org",
        "net", "io", "www", "there", "here", "when", "where", "what", "how", "why", "who",
        "which", "their", "them", "then", "than", "also", "into", "out", "up", "only", "just",
        "now", "get", "use", "using", "used", "make", "made", "way", "new", "like", "over",
        "your", "our", "its", "see", "top", "via", "every", "being", "before", "after",
        "above", "below", "blob", "tree",
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
    let packages = json
        .get("packages")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    for pkg in packages {
        let targets = pkg
            .get("targets")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        for target in targets {
            let name = target
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
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
            let repo = crate::scan::inspect_repo(&p)?;
            WorkspaceRegistry::save_repo(&mut conn, &repo)?;
            vec![repo]
        }
    };

    let mut count = 0;
    for repo in &repos {
        let (summary, keywords) = match extract_readme_summary(&repo.local_path) {
            Some((s, k)) => (s, k.join(", ")),
            None => {
                warn!("Failed to extract summary for {}", repo.id);
                ("Unknown project".to_string(), "unknown".to_string())
            }
        };

        let modules = extract_module_structure(&repo.local_path);

        WorkspaceRegistry::save_summary(&conn, &repo.id, &summary, &keywords)?;

        let modules_tuple: Vec<(String, String)> = modules
            .into_iter()
            .map(|m| (m.name, m.kind))
            .collect();
        WorkspaceRegistry::save_modules(&mut conn, &repo.id, &modules_tuple)?;

        println!(
            "Indexed [{}] -> \"{}\" (keywords: {})",
            repo.id, summary, keywords
        );
        count += 1;
    }

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
        let content_words = [
            "git", "fast", "terminal", "branches", "commits", "features", "custom",
        ];
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
        let path = Path::new(r"C:\Users\22414\dev\third_party\gitui");
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
        let path = Path::new(r"C:\Users\22414\dev\third_party\syncthing");
        if !path.exists() {
            return;
        }
        let (summary, keywords) = extract_readme_summary(path).unwrap();
        println!("syncthing summary: {}", summary);
        println!("syncthing keywords: {:?}", keywords);
        let modules = extract_module_structure(path);
        println!("syncthing modules (first 10): {:?}", modules.iter().take(10).collect::<Vec<_>>());
        assert!(!summary.is_empty());
    }
}
