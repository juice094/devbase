/// A single WikiLink found in a Markdown document.
#[derive(Debug, Clone, PartialEq)]
pub struct WikiLink {
    pub target: String,
    pub display: Option<String>,
    pub start: usize,
    pub end: usize,
}

/// Extract all `[[...]]` style WikiLinks from Markdown content.
pub fn extract_wikilinks(content: &str) -> Vec<WikiLink> {
    let mut links = Vec::new();
    let chars: Vec<char> = content.chars().collect();
    let mut i = 0;

    while i + 1 < chars.len() {
        if chars[i] == '[' && chars[i + 1] == '[' {
            let start = i;
            i += 2;
            let mut depth = 1;
            let inner_start = i;

            while i < chars.len() && depth > 0 {
                if i + 1 < chars.len() && chars[i] == ']' && chars[i + 1] == ']' {
                    depth -= 1;
                    if depth == 0 {
                        let inner = &chars[inner_start..i];
                        let inner_str: String = inner.iter().collect();
                        let link = parse_link(&inner_str, start, i + 2);
                        links.push(link);
                        i += 2;
                        break;
                    }
                } else if i + 1 < chars.len() && chars[i] == '[' && chars[i + 1] == '[' {
                    depth += 1;
                    i += 2;
                } else {
                    i += 1;
                }
            }
        } else {
            i += 1;
        }
    }

    links
}

fn parse_link(inner: &str, start: usize, end: usize) -> WikiLink {
    if let Some(pipe_pos) = inner.find('|') {
        WikiLink {
            target: inner[..pipe_pos].trim().to_string(),
            display: Some(inner[pipe_pos + 1..].trim().to_string()),
            start,
            end,
        }
    } else {
        let target = inner.trim().to_string();
        WikiLink {
            target: target.clone(),
            display: None,
            start,
            end,
        }
    }
}

/// Build a backlink index: for each target, list the source note IDs that link to it.
pub fn build_backlink_index<'a>(
    notes: impl Iterator<Item = (&'a str, &'a [WikiLink])>,
) -> std::collections::HashMap<String, Vec<String>> {
    let mut index: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();
    for (source_id, links) in notes {
        for link in links {
            index.entry(link.target.clone()).or_default().push(source_id.to_string());
        }
    }
    index
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_wikilink() {
        let text = "See [[Rust]] for details.";
        let links = extract_wikilinks(text);
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].target, "Rust");
        assert_eq!(links[0].display, None);
    }

    #[test]
    fn test_wikilink_with_alias() {
        let text = "See [[Rust|the Rust language]] for details.";
        let links = extract_wikilinks(text);
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].target, "Rust");
        assert_eq!(links[0].display, Some("the Rust language".to_string()));
    }

    #[test]
    fn test_multiple_wikilinks() {
        let text = "[[A]] and [[B|bee]] are friends.";
        let links = extract_wikilinks(text);
        assert_eq!(links.len(), 2);
        assert_eq!(links[0].target, "A");
        assert_eq!(links[1].target, "B");
    }

    #[test]
    fn test_no_wikilinks() {
        let text = "Just plain text [not a link](url).";
        let links = extract_wikilinks(text);
        assert!(links.is_empty());
    }

    #[test]
    fn test_backlink_index() {
        let note_a = extract_wikilinks("[[B]] and [[C]]");
        let note_b = extract_wikilinks("[[A]]");
        let notes = vec![("a", note_a.as_slice()), ("b", note_b.as_slice())];
        let index = build_backlink_index(notes.into_iter());
        assert_eq!(index.get("B"), Some(&vec!["a".to_string()]));
        assert_eq!(index.get("C"), Some(&vec!["a".to_string()]));
        assert_eq!(index.get("A"), Some(&vec!["b".to_string()]));
    }
}
