// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (c) 2026 juice094

use std::collections::HashSet;

use crate::SymbolLink;

/// Compute `similar_signature` links within a repo.
///
/// Links symbols whose signatures share >= `threshold` Jaccard similarity
/// of token sets. Default threshold: 0.3 (30% token overlap).
pub fn compute_similar_signature_links(
    conn: &rusqlite::Connection,
    repo_id: &str,
    threshold: f32,
) -> anyhow::Result<Vec<SymbolLink>> {
    let mut stmt = conn.prepare(
        "SELECT name, signature FROM code_symbols
         WHERE repo_id = ?1 AND symbol_type = 'function' AND signature IS NOT NULL",
    )?;
    let rows =
        stmt.query_map([repo_id], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))?;

    let mut symbols: Vec<(String, HashSet<String>)> = Vec::new();
    for row in rows {
        let (name, sig) = row?;
        let tokens = tokenize_signature(&sig);
        if !tokens.is_empty() {
            symbols.push((name, tokens));
        }
    }

    let mut links = Vec::new();
    for i in 0..symbols.len() {
        for j in (i + 1)..symbols.len() {
            let jaccard = jaccard_similarity(&symbols[i].1, &symbols[j].1);
            if jaccard >= threshold {
                // Bidirectional link
                links.push(SymbolLink {
                    source_repo: repo_id.to_string(),
                    source_symbol: symbols[i].0.clone(),
                    target_repo: repo_id.to_string(),
                    target_symbol: symbols[j].0.clone(),
                    link_type: "similar_signature".to_string(),
                    strength: jaccard,
                });
                links.push(SymbolLink {
                    source_repo: repo_id.to_string(),
                    source_symbol: symbols[j].0.clone(),
                    target_repo: repo_id.to_string(),
                    target_symbol: symbols[i].0.clone(),
                    link_type: "similar_signature".to_string(),
                    strength: jaccard,
                });
            }
        }
    }
    Ok(links)
}

fn tokenize_signature(sig: &str) -> HashSet<String> {
    sig.split(|c: char| !c.is_alphanumeric() && c != '_')
        .map(|s| s.to_lowercase())
        .filter(|s| s.len() > 1 && !is_common_keyword(s) && !s.chars().all(|c| c.is_numeric()))
        .collect()
}

fn is_common_keyword(s: &str) -> bool {
    const KEYWORDS: &[&str] = &[
        "fn", "pub", "async", "mut", "let", "const", "static", "use", "impl", "where", "return",
        "self", "true", "false", "if", "else", "for", "while", "loop", "match", "in", "ref",
        "move", "type", "crate", "super", "dyn", "trait", "enum", "struct", "mod", "unsafe",
        "extern", "as", "break", "continue", "yield", "await", "box",
    ];
    KEYWORDS.contains(&s)
}

fn jaccard_similarity(a: &HashSet<String>, b: &HashSet<String>) -> f32 {
    if a.is_empty() && b.is_empty() {
        return 1.0;
    }
    let intersection = a.intersection(b).count();
    let union = a.union(b).count();
    if union == 0 {
        return 0.0;
    }
    intersection as f32 / union as f32
}
