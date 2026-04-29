use crate::registry::{VaultNote, WorkspaceRegistry};
use crate::vault::frontmatter::extract_frontmatter;
use crate::vault::wikilink::extract_wikilinks;

use chrono::Utc;
use std::path::{Path, PathBuf};
use tracing::{info, warn};

fn default_vault_dir() -> anyhow::Result<PathBuf> {
    let ws = crate::registry::WorkspaceRegistry::workspace_dir()?;
    let vault = ws.join("vault");
    // P1-2: PARA directory structure
    for sub in &["00-Inbox", "01-Projects", "02-Areas", "03-Resources", "04-Archives", "99-Meta"] {
        std::fs::create_dir_all(vault.join(sub))?;
    }
    Ok(vault)
}

/// Scan a vault directory for Markdown notes and sync them into the registry.
///
/// * `vault_dir` — root of the vault. If `None`, uses the default location.
/// * Returns the number of notes synced.
pub fn scan_vault(
    conn: &mut rusqlite::Connection,
    vault_dir: Option<&Path>,
) -> anyhow::Result<usize> {
    let root = match vault_dir {
        Some(p) => p.to_path_buf(),
        None => default_vault_dir()?,
    };

    if !root.exists() {
        info!("Vault directory does not exist yet: {:?}", root);
        return Ok(0);
    }

    let mut synced = 0;

    for entry in walkdir::WalkDir::new(&root)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().extension().map(|ext| ext == "md").unwrap_or(false))
    {
        let path = entry.path();
        let rel_path = path.strip_prefix(&root).unwrap_or(path);
        let id = rel_path.to_string_lossy().replace('\\', "/");

        match std::fs::read_to_string(path) {
            Ok(content) => {
                let (frontmatter, body_offset) = extract_frontmatter(&content)
                    .map(|(fm, off)| (Some(fm), off))
                    .unwrap_or((None, 0));

                let body = &content[body_offset..];
                let wikilinks = extract_wikilinks(body);
                let outgoing: Vec<String> = wikilinks.into_iter().map(|l| l.target).collect();

                let title = frontmatter.as_ref().and_then(|fm| fm.title.clone()).or_else(|| {
                    // Fallback: first H1 heading
                    body.lines()
                        .find_map(|l| l.trim().strip_prefix("# ").map(|s| s.trim().to_string()))
                });

                let tags = frontmatter.as_ref().map(|fm| fm.tags.clone()).unwrap_or_default();
                let linked_repo = frontmatter.as_ref().and_then(|fm| fm.extra.get("repo").cloned());
                let fm_raw = frontmatter.map(|fm| fm.raw);

                let note = VaultNote {
                    id,
                    path: path.to_string_lossy().to_string(),
                    title,
                    content: body.trim().to_string(),
                    frontmatter: fm_raw,
                    tags,
                    outgoing_links: outgoing,
                    linked_repo,
                    created_at: Utc::now(),
                    updated_at: Utc::now(),
                };

                if let Err(e) = WorkspaceRegistry::save_vault_note(conn, &note) {
                    warn!("Failed to save vault note {}: {}", note.id, e);
                } else {
                    synced += 1;
                }
            }
            Err(e) => {
                warn!("Failed to read vault file {:?}: {}", path, e);
            }
        }
    }

    info!("Vault scan complete: {} notes synced", synced);
    Ok(synced)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scan_vault_basic() {
        let tmp = std::env::temp_dir().join(format!("devbase_vault_scan_{}", std::process::id()));
        std::fs::create_dir_all(&tmp).unwrap();
        std::fs::write(
            tmp.join("hello.md"),
            "---\ntitle: Hello World\ntags: [rust, cli]\n---\n# Hello World\n\nThis is a [[test]] note.\n",
        )
        .unwrap();

        let mut conn = WorkspaceRegistry::init_in_memory().unwrap();
        let count = scan_vault(&mut conn, Some(&tmp)).unwrap();
        assert_eq!(count, 1);

        std::fs::remove_dir_all(&tmp).unwrap();
    }

    #[test]
    fn test_scan_vault_empty_dir() {
        let tmp = std::env::temp_dir().join(format!("devbase_vault_empty_{}", std::process::id()));
        std::fs::create_dir_all(&tmp).unwrap();
        let mut conn = WorkspaceRegistry::init_in_memory().unwrap();
        let count = scan_vault(&mut conn, Some(&tmp)).unwrap();
        assert_eq!(count, 0);
        std::fs::remove_dir_all(&tmp).unwrap();
    }

    #[test]
    fn test_scan_vault_missing_dir() {
        let tmp =
            std::env::temp_dir().join(format!("devbase_vault_missing_{}", std::process::id()));
        let mut conn = WorkspaceRegistry::init_in_memory().unwrap();
        let count = scan_vault(&mut conn, Some(&tmp)).unwrap();
        assert_eq!(count, 0);
    }
}
