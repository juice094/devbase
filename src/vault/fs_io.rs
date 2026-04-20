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
