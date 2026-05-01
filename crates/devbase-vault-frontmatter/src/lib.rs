//! devbase-vault-frontmatter — Markdown frontmatter parser for vault notes.
//!
//! **提取日期**: 2026-05-01 (Workspace split)
//! **零内部耦合**: 此 crate 不依赖 devbase 任何内部模块，仅使用 std。
//! **职责**: 解析 Markdown 文件的 YAML frontmatter 头部，返回键值对。
//! **边界**: 输入 `&str`（文件内容），输出 `HashMap<String, String>`。不触及文件系统。
//!
//! 与 devbase 的关系: 被 devbase `vault/indexer` 调用，解析笔记元数据。
//!
//! Design decisions:
//! - 使用 `---` 分隔符: 兼容 Obsidian / Jekyll / Hugo 格式。
//! - 空值保留为空字符串: 避免 `None` 与 `""` 的歧义。
//! - 只解析第一个 frontmatter 块: 后续 `---` 视为正文。

use std::collections::HashMap;

/// Parsed frontmatter from a Markdown vault note.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Frontmatter {
    pub id: Option<String>,
    pub title: Option<String>,
    pub repo: Option<String>,
    pub tags: Vec<String>,
    pub aliases: Vec<String>,
    pub ai_context: Option<bool>,
    pub created: Option<String>,
    pub updated: Option<String>,
    pub date: Option<String>,
    pub raw: String,
    pub extra: HashMap<String, String>,
}

/// Extract YAML frontmatter from the top of a Markdown document.
///
/// Returns `(frontmatter, body_offset)` where `body_offset` is the byte index
/// at which the Markdown body begins (after the closing `---`).
pub fn extract_frontmatter(content: &str) -> Option<(Frontmatter, usize)> {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        return None;
    }

    let after_open = &trimmed[3..];
    let close_pos = after_open.find("\n---")?;
    let raw = after_open[..close_pos].trim();
    let body_offset = trimmed.as_ptr() as usize - content.as_ptr() as usize + 3 + close_pos + 4;

    let fm = parse_yaml_frontmatter(raw);
    Some((fm, body_offset))
}

fn parse_yaml_frontmatter(raw: &str) -> Frontmatter {
    let mut fm = Frontmatter {
        raw: raw.to_string(),
        ..Default::default()
    };

    // Lightweight YAML parsing: only handle key: value and key: [list] patterns.
    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if let Some((key, rest)) = line.split_once(':') {
            let key = key.trim();
            let rest = rest.trim();

            match key {
                "id" => {
                    fm.id = Some(unquote(rest).to_string());
                }
                "title" => {
                    fm.title = Some(unquote(rest).to_string());
                }
                "repo" => {
                    fm.repo = Some(unquote(rest).to_string());
                }
                "date" => {
                    fm.date = Some(unquote(rest).to_string());
                }
                "created" => {
                    fm.created = Some(unquote(rest).to_string());
                }
                "updated" => {
                    fm.updated = Some(unquote(rest).to_string());
                }
                "ai_context" => {
                    fm.ai_context = Some(parse_bool(rest));
                }
                "tags" => {
                    fm.tags = parse_yaml_list(rest, raw, line);
                }
                "aliases" => {
                    fm.aliases = parse_yaml_list(rest, raw, line);
                }
                _ => {
                    fm.extra.insert(key.to_string(), unquote(rest).to_string());
                }
            }
        }
    }

    fm
}

fn parse_bool(s: &str) -> bool {
    matches!(s.trim().to_lowercase().as_str(), "true" | "yes" | "1" | "on")
}

fn unquote(s: &str) -> &str {
    s.strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .or_else(|| s.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')))
        .unwrap_or(s)
}

fn parse_yaml_list<'a>(rest: &'a str, raw: &'a str, line: &'a str) -> Vec<String> {
    if rest.starts_with('[') && rest.ends_with(']') {
        rest[1..rest.len() - 1]
            .split(',')
            .map(|s| unquote(s.trim()).to_string())
            .filter(|s| !s.is_empty())
            .collect()
    } else if rest.is_empty() {
        // Multi-line list starting on next lines: "- item"
        let mut items = Vec::new();
        let mut in_list = false;
        for l in raw.lines() {
            if l.trim() == line.trim() {
                in_list = true;
                continue;
            }
            if in_list {
                let tl = l.trim_start();
                if let Some(stripped) = tl.strip_prefix("- ") {
                    items.push(unquote(stripped).to_string());
                } else if !tl.is_empty() && !tl.starts_with('#') {
                    break;
                }
            }
        }
        items
    } else {
        vec![unquote(rest).to_string()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_frontmatter() {
        assert!(extract_frontmatter("# Hello\nworld").is_none());
    }

    #[test]
    fn test_basic_yaml_frontmatter() {
        let md = "---\ntitle: Hello World\ntags: [rust, cli]\ndate: 2024-01-01\n---\n# Body\n";
        let (fm, offset) = extract_frontmatter(md).unwrap();
        assert_eq!(fm.title, Some("Hello World".to_string()));
        assert_eq!(fm.tags, vec!["rust", "cli"]);
        assert_eq!(fm.date, Some("2024-01-01".to_string()));
        assert!(md[offset..].trim_start().starts_with("# Body"));
    }

    #[test]
    fn test_multiline_list() {
        let md = "---\ntags:\n  - rust\n  - cli\n---\nbody\n";
        let (fm, _) = extract_frontmatter(md).unwrap();
        assert_eq!(fm.tags, vec!["rust", "cli"]);
    }

    #[test]
    fn test_quoted_strings() {
        let md = "---\ntitle: \"My Note\"\ntags: ['a', 'b']\n---\n";
        let (fm, _) = extract_frontmatter(md).unwrap();
        assert_eq!(fm.title, Some("My Note".to_string()));
        assert_eq!(fm.tags, vec!["a", "b"]);
    }

    #[test]
    fn test_extra_fields() {
        let md = "---\ntitle: X\ncategory: dev\n---\n";
        let (fm, _) = extract_frontmatter(md).unwrap();
        assert_eq!(fm.extra.get("category"), Some(&"dev".to_string()));
    }
}
