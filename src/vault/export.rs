// SPDX-License-Identifier: MIT
// Copyright (c) 2026 juice094
//! Vault export — data freedom and vendor lock-in elimination.

use std::collections::HashSet;
use std::path::Path;

/// Export vault notes to an output directory with integrity validation.
///
/// - Copies all `.md` files preserving relative directory structure
/// - Validates wikilink targets exist (reports broken links)
/// - Validates frontmatter YAML is parseable
/// - Returns statistics and any integrity issues found
pub fn export_vault(vault_dir: &Path, output_dir: &Path) -> anyhow::Result<serde_json::Value> {
    std::fs::create_dir_all(output_dir)?;

    let mut exported = 0usize;
    let mut bytes = 0usize;
    let mut broken_links: Vec<serde_json::Value> = Vec::new();
    let mut frontmatter_errors: Vec<serde_json::Value> = Vec::new();

    // First pass: collect all note IDs for broken link detection
    let mut all_note_ids = HashSet::new();
    for entry in walkdir::WalkDir::new(vault_dir)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().extension().map(|ext| ext == "md").unwrap_or(false))
    {
        let rel = entry.path().strip_prefix(vault_dir).unwrap_or(entry.path());
        let id = rel.to_string_lossy().replace('\\', "/");
        all_note_ids.insert(id.clone());
        // Also index by stem (without .md) for wikilink resolution
        if let Some(stem) = id.strip_suffix(".md") {
            all_note_ids.insert(stem.to_string());
        }
    }

    // Second pass: copy and validate
    for entry in walkdir::WalkDir::new(vault_dir)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let src = entry.path();
        let rel = src.strip_prefix(vault_dir).unwrap_or(src);
        let dst = output_dir.join(rel);

        if let Some(parent) = dst.parent() {
            std::fs::create_dir_all(parent)?;
        }

        if src.extension().map(|e| e == "md").unwrap_or(false) {
            let content = std::fs::read_to_string(src)?;
            bytes += content.len();

            // Validate frontmatter
            if crate::vault::frontmatter::extract_frontmatter(&content).is_none() {
                frontmatter_errors.push(serde_json::json!({
                    "path": rel.to_string_lossy().replace('\\', "/"),
                    "error": "Failed to parse frontmatter",
                }));
            }

            // Validate wikilinks
            for link in crate::vault::wikilink::extract_wikilinks(&content) {
                let target_normalized = link.target.replace('\\', "/");
                if !all_note_ids.contains(&target_normalized) {
                    broken_links.push(serde_json::json!({
                        "source": rel.to_string_lossy().replace('\\', "/"),
                        "target": link.target,
                    }));
                }
            }

            std::fs::write(&dst, content)?;
        } else {
            // Copy non-markdown assets as-is
            std::fs::copy(src, dst)?;
        }
        exported += 1;
    }

    Ok(serde_json::json!({
        "success": true,
        "vault_dir": vault_dir.to_string_lossy(),
        "output_dir": output_dir.to_string_lossy(),
        "exported_files": exported,
        "total_bytes": bytes,
        "broken_links": {
            "count": broken_links.len(),
            "issues": broken_links,
        },
        "frontmatter_errors": {
            "count": frontmatter_errors.len(),
            "issues": frontmatter_errors,
        },
    }))
}
