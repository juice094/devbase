// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (c) 2026 juice094
//! Dead code query helpers.

/// A potentially dead function from the code symbol index.
#[derive(Debug, Clone)]
pub struct DeadFunction {
    pub file_path: String,
    pub name: String,
    pub line_start: i64,
    pub signature: Option<String>,
}

/// Query potentially dead functions for a specific repository.
///
/// A function is considered "dead" if it has no incoming call edges in the
/// intra-repo call graph. Results may include false positives (public APIs,
/// trait methods, callbacks, dynamically dispatched functions).
pub fn query_dead_code(
    conn: &rusqlite::Connection,
    repo_id: &str,
    include_pub: bool,
    limit: usize,
) -> anyhow::Result<Vec<DeadFunction>> {
    let mut sql = String::from(
        "SELECT cs.file_path, cs.name, cs.line_start, cs.signature \
         FROM code_symbols cs \
         WHERE cs.repo_id = ?1 AND cs.symbol_type = 'function' \
         AND NOT EXISTS ( \
             SELECT 1 FROM code_call_graph ccg \
             WHERE ccg.repo_id = cs.repo_id AND ccg.callee_name = cs.name \
         )",
    );
    let params: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(repo_id.to_string())];

    if !include_pub {
        // Heuristic: exclude signatures that contain "pub" followed by "fn"
        // Covers: pub fn, pub async fn, pub(crate) fn, pub unsafe fn, etc.
        sql.push_str(" AND (cs.signature IS NULL OR cs.signature NOT LIKE 'pub%fn%')");
    }
    // Exclude main() — entry points are never dead code
    sql.push_str(" AND cs.name != 'main'");
    // Exclude test functions — heuristic: name starts with 'test_' (Rust convention)
    sql.push_str(" AND cs.name NOT LIKE 'test_%'");
    // Exclude functions in tests.rs files (Rust unit-test modules)
    sql.push_str(" AND cs.file_path NOT LIKE '%/tests.rs' AND cs.file_path NOT LIKE '%\\tests.rs'");

    sql.push_str(&format!(" ORDER BY cs.file_path, cs.line_start LIMIT {}", limit.min(200)));

    let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(rusqlite::params_from_iter(param_refs), |row| {
        Ok(DeadFunction {
            file_path: row.get::<_, String>(0)?,
            name: row.get::<_, String>(1)?,
            line_start: row.get::<_, i64>(2)?,
            signature: row.get::<_, Option<String>>(3)?,
        })
    })?;

    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn init_in_memory() -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(
            r#"
            CREATE TABLE code_symbols (
                id INTEGER PRIMARY KEY,
                repo_id TEXT NOT NULL,
                file_path TEXT NOT NULL,
                symbol_type TEXT NOT NULL,
                name TEXT NOT NULL,
                line_start INTEGER,
                line_end INTEGER,
                signature TEXT,
                attributes TEXT
            );
            CREATE TABLE code_call_graph (
                id INTEGER PRIMARY KEY,
                repo_id TEXT NOT NULL,
                caller_file TEXT NOT NULL,
                caller_symbol TEXT NOT NULL,
                caller_line INTEGER,
                callee_name TEXT NOT NULL
            );
            "#,
        )
        .unwrap();
        conn
    }

    fn seed_data(conn: &rusqlite::Connection) {
        // Functions: some have callers, some don't
        let symbols = [
            (
                "repo-a",
                "src/main.rs",
                "function",
                "main",
                1,
                5,
                Some("pub fn main()"),
                None::<&str>,
            ),
            (
                "repo-a",
                "src/lib.rs",
                "function",
                "helper",
                10,
                20,
                Some("fn helper()"),
                None::<&str>,
            ),
            (
                "repo-a",
                "src/lib.rs",
                "function",
                "unused_fn",
                30,
                40,
                Some("fn unused_fn()"),
                None::<&str>,
            ),
            (
                "repo-a",
                "src/lib.rs",
                "function",
                "pub_api",
                50,
                60,
                Some("pub fn pub_api()"),
                None::<&str>,
            ),
            (
                "repo-a",
                "src/tests.rs",
                "function",
                "test_something",
                1,
                10,
                Some("fn test_something()"),
                None::<&str>,
            ),
            (
                "repo-a",
                "src/utils.rs",
                "function",
                "test_helper",
                1,
                5,
                Some("fn test_helper()"),
                None::<&str>,
            ),
            ("repo-a", "src/utils.rs", "struct", "Config", 10, 15, None::<&str>, None::<&str>),
        ];
        for (repo, path, ty, name, start, end, sig, attrs) in symbols {
            conn.execute(
                "INSERT INTO code_symbols (repo_id, file_path, symbol_type, name, line_start, line_end, signature, attributes)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                rusqlite::params![repo, path, ty, name, start, end, sig, attrs],
            )
            .unwrap();
        }

        // Call edges: main -> helper, main -> unused_fn (so unused_fn HAS a caller)
        let edges = [
            ("repo-a", "src/main.rs", "main", 3, "helper"),
            ("repo-a", "src/main.rs", "main", 4, "unused_fn"),
        ];
        for (repo, file, caller, line, callee) in edges {
            conn.execute(
                "INSERT INTO code_call_graph (repo_id, caller_file, caller_symbol, caller_line, callee_name)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![repo, file, caller, line, callee],
            )
            .unwrap();
        }
    }

    #[test]
    fn test_query_dead_code_include_pub() {
        let conn = init_in_memory();
        seed_data(&conn);

        // include_pub=true: should find pub_api (no callers) but not main (excluded by name rule)
        let results = query_dead_code(&conn, "repo-a", true, 100).unwrap();
        assert!(results.iter().any(|f| f.name == "pub_api"));
        assert!(!results.iter().any(|f| f.name == "main"));
        assert!(!results.iter().any(|f| f.name == "test_something"));
        assert!(!results.iter().any(|f| f.name == "test_helper"));
        // Structs are not functions, so Config should not appear
        assert!(!results.iter().any(|f| f.name == "Config"));
    }

    #[test]
    fn test_query_dead_code_exclude_pub() {
        let conn = init_in_memory();
        seed_data(&conn);

        // include_pub=false: pub_api excluded by signature heuristic
        let results = query_dead_code(&conn, "repo-a", false, 100).unwrap();
        assert!(!results.iter().any(|f| f.name == "pub_api"));
    }

    #[test]
    fn test_query_dead_code_excludes_called_functions() {
        let conn = init_in_memory();
        seed_data(&conn);

        // helper and unused_fn both have callers, so they should NOT appear
        let results = query_dead_code(&conn, "repo-a", true, 100).unwrap();
        assert!(!results.iter().any(|f| f.name == "helper"));
        assert!(!results.iter().any(|f| f.name == "unused_fn"));
    }

    #[test]
    fn test_query_dead_code_excludes_tests_rs() {
        let conn = init_in_memory();
        seed_data(&conn);

        let results = query_dead_code(&conn, "repo-a", true, 100).unwrap();
        // test_something is in tests.rs, excluded by file path rule
        assert!(!results.iter().any(|f| f.name == "test_something"));
    }

    #[test]
    fn test_query_dead_code_limit() {
        let conn = init_in_memory();
        seed_data(&conn);

        let results = query_dead_code(&conn, "repo-a", true, 1).unwrap();
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_query_dead_code_empty_repo() {
        let conn = init_in_memory();
        // No data seeded

        let results = query_dead_code(&conn, "repo-x", true, 100).unwrap();
        assert!(results.is_empty());
    }
}
