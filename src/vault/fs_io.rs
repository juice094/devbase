use std::path::Path;

/// Read the full Markdown content from the filesystem.
/// Returns `None` if the file does not exist or cannot be read.
pub fn read_note_content(path: &str) -> Option<String> {
    std::fs::read_to_string(Path::new(path)).ok()
}

/// Read note content, split into (body, frontmatter_raw).
/// Returns `None` if the file cannot be read.
pub fn read_note_body(path: &str) -> Option<(String, Option<String>)> {
    let full = read_note_content(path)?;
    let (fm_raw, body_offset) = crate::vault::frontmatter::extract_frontmatter(&full)
        .map(|(fm, off)| (Some(fm.raw), off))
        .unwrap_or((None, 0));
    let body = full[body_offset..].trim().to_string();
    Some((body, fm_raw))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_read_note_content() {
        let mut file = NamedTempFile::new().unwrap();
        write!(file, "# Hello\n\nWorld").unwrap();
        let content = read_note_content(file.path().to_str().unwrap()).unwrap();
        assert_eq!(content, "# Hello\n\nWorld");
    }

    #[test]
    fn test_read_note_content_missing() {
        assert!(read_note_content("/nonexistent/path/note.md").is_none());
    }

    #[test]
    fn test_read_note_body_with_frontmatter() {
        let mut file = NamedTempFile::new().unwrap();
        write!(file, "---\ntitle: Test\n---\n\n# Body\n").unwrap();
        let (body, fm) = read_note_body(file.path().to_str().unwrap()).unwrap();
        assert_eq!(body, "# Body");
        assert!(fm.is_some());
        assert!(fm.unwrap().contains("title: Test"));
    }

    #[test]
    fn test_read_note_body_without_frontmatter() {
        let mut file = NamedTempFile::new().unwrap();
        write!(file, "# Body\n\nText").unwrap();
        let (body, fm) = read_note_body(file.path().to_str().unwrap()).unwrap();
        assert_eq!(body, "# Body\n\nText");
        assert!(fm.is_none());
    }
}
