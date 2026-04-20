use std::collections::HashMap;

/// Parsed frontmatter from a Markdown vault note.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Frontmatter {
    pub title: Option<String>,
    pub tags: Vec<String>,
    pub aliases: Vec<String>,
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
                "title" => {
                    fm.title = Some(unquote(rest).to_string());
                }
                "date" => {
                    fm.date = Some(unquote(rest).to_string());
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
                if tl.starts_with("- ") {
                    items.push(unquote(&tl[2..]).to_string());
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
