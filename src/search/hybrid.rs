//! Hybrid search: vector similarity + keyword matching with RRF merge.
//!
//! This module provides the core fusion algorithm for combining dense
//! (embedding-based) and sparse (keyword-based) retrieval results.
//!
//! Design: the keyword path is currently SQLite LIKE matching on symbol
//! name + signature. Future waves may upgrade this to Tantivy BM25 or
//! SQLite FTS5 without changing the RRF merge layer.

use std::collections::HashMap;

use crate::semantic_index::SemanticSearchRow;

/// Keyword search over code symbols using SQLite LIKE on name and signature.
///
/// Score heuristic:
/// - name match = 3.0
/// - signature match = 1.0
///
/// The query string is whitespace-split and each token contributes
/// independently.
pub fn keyword_search_symbols(
    conn: &rusqlite::Connection,
    repo_id: &str,
    query: &str,
    limit: usize,
) -> anyhow::Result<Vec<SemanticSearchRow>> {
    let tokens: Vec<&str> = query.split_whitespace().collect();
    if tokens.is_empty() {
        return Ok(Vec::new());
    }

    // Simple per-token query with aggregated scoring.
    // For each token we query once and accumulate scores in a HashMap.
    // name match = 3.0, signature match = 1.0.
    let mut accum: HashMap<String, (String, String, String, i64, f32)> = HashMap::new();

    for token in &tokens {
        let pat = format!("%{}%", token);
        let mut stmt = conn.prepare(
            "SELECT repo_id, name, file_path, line_start
             FROM code_symbols
             WHERE repo_id = ?1 AND symbol_type = 'function'
               AND (name LIKE ?2 OR signature LIKE ?2)",
        )?;
        let rows = stmt.query_map(rusqlite::params![repo_id, &pat], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
            ))
        })?;

        for row in rows {
            let (repo, name, path, line) = row?;
            let key = format!("{}::{}::{}", repo, name, path);
            let bonus = if name.to_lowercase().contains(&token.to_lowercase()) {
                3.0
            } else {
                1.0
            };
            accum
                .entry(key)
                .and_modify(|(_, _, _, _, score)| *score += bonus)
                .or_insert_with(|| (repo, name, path, line, bonus));
        }
    }

    let mut results: Vec<SemanticSearchRow> = accum.into_values().collect();
    results.sort_by(|a, b| {
        b.4.partial_cmp(&a.4)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.1.cmp(&b.1))
    });
    results.truncate(limit);
    Ok(results)
}

/// Reciprocal Rank Fusion (RRF) merge.
///
/// Takes multiple ranked lists of `SemanticSearchRow` and produces a single
/// fused ranking. The standard k constant is 60.0.
///
/// Items are deduplicated by `(repo_id, name, file_path)`.
pub fn rrf_merge(lists: Vec<Vec<SemanticSearchRow>>, k: f32) -> Vec<SemanticSearchRow> {
    if lists.is_empty() {
        return Vec::new();
    }
    if lists.len() == 1 {
        return lists.into_iter().next().unwrap();
    }

    let mut accum: HashMap<String, (SemanticSearchRow, f32)> = HashMap::new();

    for list in lists {
        for (rank, row) in list.into_iter().enumerate() {
            let key = format!("{}::{}::{}", row.0, row.1, row.2);
            let rrf_score = 1.0 / (k + rank as f32);
            accum
                .entry(key)
                .and_modify(|(_, score)| *score += rrf_score)
                .or_insert_with(|| (row, rrf_score));
        }
    }

    let mut merged: Vec<(SemanticSearchRow, f32)> = accum.into_values().collect();
    merged.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.0.1.cmp(&b.0.1)) // tie-break by name
    });

    merged.into_iter().map(|(row, _)| row).collect()
}

/// Hybrid search over code symbols.
///
/// - If `query_embedding` is provided: runs vector search + keyword search
///   and fuses results with RRF (k=60).
/// - If only `query_text` is provided: falls back to pure keyword search.
/// - If neither yields results: returns empty Vec.
pub fn hybrid_search_symbols(
    conn: &rusqlite::Connection,
    repo_id: &str,
    query_text: &str,
    query_embedding: Option<&[f32]>,
    limit: usize,
) -> anyhow::Result<Vec<SemanticSearchRow>> {
    let mut lists: Vec<Vec<SemanticSearchRow>> = Vec::new();

    // Vector path
    if let Some(emb) = query_embedding {
        let vec_results = crate::registry::WorkspaceRegistry::semantic_search_symbols(
            conn,
            repo_id,
            emb,
            limit * 2,
        )?;
        if !vec_results.is_empty() {
            lists.push(vec_results);
        }
    }

    // Keyword path
    let kw_results = keyword_search_symbols(conn, repo_id, query_text, limit * 2)?;
    if !kw_results.is_empty() {
        lists.push(kw_results);
    }

    match lists.len() {
        0 => Ok(Vec::new()),
        1 => Ok(lists.into_iter().next().unwrap().into_iter().take(limit).collect()),
        _ => {
            let merged = rrf_merge(lists, 60.0);
            Ok(merged.into_iter().take(limit).collect())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rrf_merge_two_lists() {
        let list_a: Vec<SemanticSearchRow> = vec![
            ("r1".into(), "sym_a".into(), "a.rs".into(), 1, 0.9),
            ("r1".into(), "sym_b".into(), "b.rs".into(), 2, 0.8),
        ];
        let list_b: Vec<SemanticSearchRow> = vec![
            ("r1".into(), "sym_b".into(), "b.rs".into(), 2, 0.85),
            ("r1".into(), "sym_c".into(), "c.rs".into(), 3, 0.7),
        ];

        let merged = rrf_merge(vec![list_a, list_b], 60.0);
        assert_eq!(merged.len(), 3);

        // sym_b appears in both lists -> highest RRF score
        assert_eq!(merged[0].1, "sym_b");
        // sym_a only in list_a at rank 0 -> 1/60
        // sym_c only in list_b at rank 1 -> 1/61
        // So sym_a should outrank sym_c
        assert_eq!(merged[1].1, "sym_a");
        assert_eq!(merged[2].1, "sym_c");
    }

    #[test]
    fn test_rrf_merge_single_list_passthrough() {
        let list: Vec<SemanticSearchRow> = vec![("r1".into(), "x".into(), "x.rs".into(), 1, 0.5)];
        let merged = rrf_merge(vec![list.clone()], 60.0);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].1, "x");
    }

    #[test]
    fn test_rrf_merge_empty_lists() {
        let merged = rrf_merge(vec![], 60.0);
        assert!(merged.is_empty());
    }

    #[test]
    fn test_keyword_search_basic() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute(
            "CREATE TABLE code_symbols (
                repo_id TEXT NOT NULL,
                file_path TEXT NOT NULL,
                symbol_type TEXT NOT NULL,
                name TEXT NOT NULL,
                line_start INTEGER,
                line_end INTEGER,
                signature TEXT,
                PRIMARY KEY (repo_id, file_path, name)
            )",
            [],
        )
        .unwrap();

        conn.execute(
            "INSERT INTO code_symbols (repo_id, file_path, symbol_type, name, line_start, signature)
             VALUES ('repo1', 'src/lib.rs', 'function', 'handle_error', 10, 'pub fn handle_error(e: Error)'),
                    ('repo1', 'src/lib.rs', 'function', 'parse_config', 20, 'fn parse_config() -> Config'),
                    ('repo1', 'src/lib.rs', 'function', 'main', 1, 'fn main()'),
                    ('repo1', 'src/lib.rs', 'struct', 'Config', 5, NULL)",
            [],
        )
        .unwrap();

        let results = keyword_search_symbols(&conn, "repo1", "error", 10).unwrap();
        assert!(!results.is_empty());
        // handle_error should rank highest (name match = 3)
        assert_eq!(results[0].1, "handle_error");
    }

    #[test]
    fn test_hybrid_search_fallback_to_keyword() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute(
            "CREATE TABLE code_symbols (
                repo_id TEXT NOT NULL,
                file_path TEXT NOT NULL,
                symbol_type TEXT NOT NULL,
                name TEXT NOT NULL,
                line_start INTEGER,
                line_end INTEGER,
                signature TEXT,
                PRIMARY KEY (repo_id, file_path, name)
            )",
            [],
        )
        .unwrap();

        conn.execute(
            "INSERT INTO code_symbols (repo_id, file_path, symbol_type, name, line_start, signature)
             VALUES ('repo1', 'src/lib.rs', 'function', 'authenticate', 10, 'pub fn authenticate(token: &str)'),
                    ('repo1', 'src/lib.rs', 'function', 'validate_token', 20, 'fn validate_token(t: &str) -> bool')",
            [],
        )
        .unwrap();

        // No embedding provided -> should fall back to keyword search
        let results = hybrid_search_symbols(&conn, "repo1", "token", None, 10).unwrap();
        assert!(!results.is_empty());
        // Both symbols mention "token" in signature (score 1 each)
        let names: Vec<&str> = results.iter().map(|r| r.1.as_str()).collect();
        assert!(names.contains(&"validate_token"));
    }
}
