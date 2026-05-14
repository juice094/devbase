// SPDX-License-Identifier: MIT
// Copyright (c) 2026 juice094
//! Vault export — data freedom and vendor lock-in elimination.

use std::collections::{HashMap, HashSet};
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
    let mut broken_block_refs: Vec<serde_json::Value> = Vec::new();
    let mut frontmatter_errors: Vec<serde_json::Value> = Vec::new();

    // First pass: collect all note IDs and headings for broken link / block-ref detection
    let mut all_note_ids = HashSet::new();
    let mut note_headings: HashMap<String, HashSet<String>> = HashMap::new();
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
        if let Some(stem) = id.strip_suffix(".md") {
            all_note_ids.insert(stem.to_string());
        }

        // Extract headings for block-ref validation
        if let Ok(content) = std::fs::read_to_string(entry.path()) {
            let mut headings = HashSet::new();
            for line in content.lines() {
                let trimmed = line.trim_start();
                if let Some(rest) = trimmed.strip_prefix("# ") {
                    headings.insert(rest.trim().to_string());
                } else if let Some(rest) = trimmed.strip_prefix("## ") {
                    headings.insert(rest.trim().to_string());
                } else if let Some(rest) = trimmed.strip_prefix("### ") {
                    headings.insert(rest.trim().to_string());
                }
            }
            note_headings.insert(id.clone(), headings);
            if let Some(stem) = id.strip_suffix(".md") {
                note_headings.insert(stem.to_string(), note_headings[&id].clone());
            }
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

            // Validate wikilinks and block refs
            for link in crate::vault::wikilink::extract_wikilinks(&content) {
                let target_normalized = link.target.replace('\\', "/");
                if !all_note_ids.contains(&target_normalized) {
                    broken_links.push(serde_json::json!({
                        "source": rel.to_string_lossy().replace('\\', "/"),
                        "target": link.target,
                    }));
                } else if let Some(ref anchor) = link.anchor {
                    // Only validate heading anchors (not ^block-id)
                    if !anchor.starts_with('^') {
                        let headings = note_headings.get(&target_normalized);
                        if headings.map(|h| !h.contains(anchor)).unwrap_or(true) {
                            broken_block_refs.push(serde_json::json!({
                                "source": rel.to_string_lossy().replace('\\', "/"),
                                "target": link.target,
                                "anchor": anchor,
                            }));
                        }
                    }
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
        "broken_block_refs": {
            "count": broken_block_refs.len(),
            "issues": broken_block_refs,
        },
    }))
}
