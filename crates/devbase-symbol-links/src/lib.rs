//! Explicit knowledge link generation between code symbols.
//!
//! Unlike the call graph (which captures runtime relationships),
//! symbol links capture *conceptual* relationships:
//! - `similar_signature`: functions with similar parameter/type signatures
//! - `co_located`: functions defined in the same file
//!
//! These links are stored in `code_symbol_links` (Schema v13) and can be
//! traversed by AI agents to discover related concepts beyond direct calls.

use std::collections::{HashMap, HashSet};

/// A generated link between two symbols.
#[derive(Debug, Clone, PartialEq)]
pub struct SymbolLink {
    pub source_repo: String,
    pub source_symbol: String,
    pub target_repo: String,
    pub target_symbol: String,
    pub link_type: String,
    pub strength: f32,
}

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

/// Build all link types for a repo and persist to `code_symbol_links`.
pub fn generate_and_save_links(
    conn: &mut rusqlite::Connection,
    repo_id: &str,
) -> anyhow::Result<usize> {
    let mut all_links = Vec::new();
    all_links.extend(compute_similar_signature_links(conn, repo_id, 0.3)?);
    all_links.extend(compute_co_located_links(conn, repo_id)?);

    if all_links.is_empty() {
        return Ok(0);
    }

    let tx = conn.transaction()?;
    tx.execute("DELETE FROM code_symbol_links WHERE source_repo = ?1", [repo_id])?;

    let now = chrono::Utc::now().to_rfc3339();
    let mut inserted = 0;
    for link in all_links {
        tx.execute(
            "INSERT OR IGNORE INTO code_symbol_links
             (source_repo, source_symbol, target_repo, target_symbol, link_type, strength, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![
                &link.source_repo,
                &link.source_symbol,
                &link.target_repo,
                &link.target_symbol,
                &link.link_type,
                link.strength,
                &now,
            ],
        )?;
        inserted += 1;
    }
    tx.commit()?;
    Ok(inserted)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokenize_signature() {
        let tokens = tokenize_signature("pub fn authenticate(token: &str) -> Result<User>");
        assert!(tokens.contains("authenticate"));
        assert!(tokens.contains("token"));
        assert!(tokens.contains("str"));
        assert!(tokens.contains("result"));
        assert!(tokens.contains("user"));
        assert!(!tokens.contains("fn"));
        assert!(!tokens.contains("pub"));
    }

    #[test]
    fn test_jaccard_similarity() {
        let a: HashSet<String> = ["a".into(), "b".into(), "c".into()].into_iter().collect();
        let b: HashSet<String> = ["b".into(), "c".into(), "d".into()].into_iter().collect();
        // intersection = 2, union = 4
        assert!((jaccard_similarity(&a, &b) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn test_compute_co_located_links() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute(
            "CREATE TABLE code_symbols (
                repo_id TEXT, file_path TEXT, symbol_type TEXT, name TEXT,
                line_start INTEGER, line_end INTEGER, signature TEXT,
                PRIMARY KEY(repo_id, file_path, name)
            )",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO code_symbols VALUES
             ('r1', 'src/lib.rs', 'function', 'foo', 1, 2, 'fn foo()'),
             ('r1', 'src/lib.rs', 'function', 'bar', 3, 4, 'fn bar()'),
             ('r1', 'src/main.rs', 'function', 'main', 1, 2, 'fn main()')",
            [],
        )
        .unwrap();

        let links = compute_co_located_links(&conn, "r1").unwrap();
        // lib.rs has foo+bar => 2 bidirectional links
        assert_eq!(links.len(), 2);
        // main.rs has only main => no links
        assert!(links.iter().all(|l| l.source_symbol != "main"));
    }

    #[test]
    fn test_compute_similar_signature_links() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute(
            "CREATE TABLE code_symbols (
                repo_id TEXT, file_path TEXT, symbol_type TEXT, name TEXT,
                line_start INTEGER, line_end INTEGER, signature TEXT,
                PRIMARY KEY(repo_id, file_path, name)
            )",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO code_symbols VALUES
             ('r1', 'a.rs', 'function', 'auth_token', 1, 2, 'fn auth_token(token: &str, timeout: u64)'),
             ('r1', 'a.rs', 'function', 'validate_token', 3, 4, 'fn validate_token(t: &str, timeout: u64) -> bool'),
             ('r1', 'a.rs', 'function', 'parse_config', 5, 6, 'fn parse_config(s: &str) -> Config')",
            [],
        )
        .unwrap();

        let links = compute_similar_signature_links(&conn, "r1", 0.3).unwrap();
        // auth_token and validate_token share token, str, timeout => should link
        assert!(!links.is_empty());
        let has_auth_validate = links.iter().any(|l| {
            (l.source_symbol == "auth_token" && l.target_symbol == "validate_token")
                || (l.source_symbol == "validate_token" && l.target_symbol == "auth_token")
        });
        assert!(has_auth_validate, "auth_token and validate_token should be linked");
    }
}
