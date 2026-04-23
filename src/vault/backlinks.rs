use std::collections::HashMap;
use std::path::Path;

/// Scan the vault directory and build a backlink index.
///
/// Returns `HashMap<target_note_id, Vec<source_note_id>>` where each source
/// is a note that contains a wikilink pointing to the target.
pub fn build_backlink_index(vault_dir: &Path) -> anyhow::Result<HashMap<String, Vec<String>>> {
    let mut index: HashMap<String, Vec<String>> = HashMap::new();

    for entry in walkdir::WalkDir::new(vault_dir)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().extension().map(|ext| ext == "md").unwrap_or(false))
    {
        let path = entry.path();
        let rel_path = path.strip_prefix(vault_dir).unwrap_or(path);
        let source_id = rel_path.to_string_lossy().replace('\\', "/");

        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let (_, body_offset) = crate::vault::frontmatter::extract_frontmatter(&content)
            .map(|(_, off)| ((), off))
            .unwrap_or(((), 0));
        let body = &content[body_offset..];
        let links = crate::vault::wikilink::extract_wikilinks(body);

        for link in links {
            index.entry(link.target.clone()).or_default().push(source_id.clone());
        }
    }

    Ok(index)
}

/// Get all source notes that link to the given target note.
pub fn get_backlinks(index: &HashMap<String, Vec<String>>, target: &str) -> Vec<String> {
    index.get(target).cloned().unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backlink_index_basic() {
        let tmp = std::env::temp_dir().join(format!("devbase_bl_{}", std::process::id()));
        std::fs::create_dir_all(&tmp).unwrap();

        std::fs::write(tmp.join("a.md"), "# A\n\nThis links to [[B]] and [[C]].\n").unwrap();
        std::fs::write(tmp.join("b.md"), "# B\n\nThis links to [[C]].\n").unwrap();
        std::fs::write(tmp.join("c.md"), "# C\n\nNo links.\n").unwrap();

        let index = build_backlink_index(&tmp).unwrap();
        assert_eq!(index.get("B").unwrap().len(), 1);
        assert!(index.get("B").unwrap().contains(&"a.md".to_string()));
        assert_eq!(index.get("C").unwrap().len(), 2);

        std::fs::remove_dir_all(&tmp).unwrap();
    }
}
