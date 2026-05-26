// SPDX-License-Identifier: MIT
// Copyright (c) 2026 juice094

use crate::Frontmatter;

pub fn parse_yaml_frontmatter(raw: &str) -> Frontmatter {
    let mut fm = Frontmatter {
        raw: raw.to_string(),
        ..Default::default()
    };

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

pub fn parse_bool(s: &str) -> bool {
    matches!(s.trim().to_lowercase().as_str(), "true" | "yes" | "1" | "on")
}

pub fn unquote(s: &str) -> &str {
    s.strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .or_else(|| s.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')))
        .unwrap_or(s)
}

pub fn parse_yaml_list<'a>(rest: &'a str, raw: &'a str, line: &'a str) -> Vec<String> {
    if rest.starts_with('[') && rest.ends_with(']') {
        rest[1..rest.len() - 1]
            .split(',')
            .map(|s| unquote(s.trim()).to_string())
            .filter(|s| !s.is_empty())
            .collect()
    } else if rest.is_empty() {
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
