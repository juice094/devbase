// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (c) 2026 juice094

use std::collections::HashMap;

use crate::SymbolLink;

/// Compute `co_located` links: functions defined in the same source file.
///
/// Strength is fixed at 0.5 — co-location is a moderate signal.
pub fn compute_co_located_links(
    conn: &rusqlite::Connection,
    repo_id: &str,
) -> anyhow::Result<Vec<SymbolLink>> {
    let mut stmt = conn.prepare(
        "SELECT file_path, name FROM code_symbols
         WHERE repo_id = ?1 AND symbol_type = 'function'",
    )?;
    let rows =
        stmt.query_map([repo_id], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))?;

    let mut by_file: HashMap<String, Vec<String>> = HashMap::new();
    for row in rows {
        let (path, name) = row?;
        by_file.entry(path).or_default().push(name);
    }

    let mut links = Vec::new();
    for (_path, names) in by_file {
        if names.len() <= 1 {
            continue;
        }
        for i in 0..names.len() {
            for j in (i + 1)..names.len() {
                links.push(SymbolLink {
                    source_repo: repo_id.to_string(),
                    source_symbol: names[i].clone(),
                    target_repo: repo_id.to_string(),
                    target_symbol: names[j].clone(),
                    link_type: "co_located".to_string(),
                    strength: 0.5,
                });
                links.push(SymbolLink {
                    source_repo: repo_id.to_string(),
                    source_symbol: names[j].clone(),
                    target_repo: repo_id.to_string(),
                    target_symbol: names[i].clone(),
                    link_type: "co_located".to_string(),
                    strength: 0.5,
                });
            }
        }
    }
    Ok(links)
}
