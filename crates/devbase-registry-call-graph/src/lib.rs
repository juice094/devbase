// SPDX-License-Identifier: MIT
// Copyright (c) 2026 juice094
//! Intra-repository call graph query helpers.

/// A single call edge from the `code_call_graph` table.
#[derive(Debug, Clone)]
pub struct CallEdge {
    pub caller_file: String,
    pub caller_symbol: String,
    pub caller_line: i64,
    pub callee_name: String,
}

/// Query call edges for a specific repository.
///
/// At least one of `callee_name` or `caller_name` should be provided to get
/// meaningful results, but the function does not enforce this — callers may
/// validate as needed.
pub fn query_call_edges(
    conn: &rusqlite::Connection,
    repo_id: &str,
    callee_name: Option<&str>,
    caller_name: Option<&str>,
    file_path: Option<&str>,
    limit: usize,
) -> anyhow::Result<Vec<CallEdge>> {
    let mut sql = String::from(
        "SELECT caller_file, caller_symbol, caller_line, callee_name \
         FROM code_call_graph WHERE repo_id = ?1",
    );
    let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(repo_id.to_string())];

    if let Some(name) = callee_name.filter(|s| !s.is_empty()) {
        sql.push_str(" AND callee_name = ?");
        sql.push_str(&(params.len() + 1).to_string());
        params.push(Box::new(name.to_string()));
    }
    if let Some(name) = caller_name.filter(|s| !s.is_empty()) {
        sql.push_str(" AND caller_symbol = ?");
        sql.push_str(&(params.len() + 1).to_string());
        params.push(Box::new(name.to_string()));
    }
    if let Some(path) = file_path.filter(|s| !s.is_empty()) {
        sql.push_str(" AND caller_file LIKE ?");
        sql.push_str(&(params.len() + 1).to_string());
        params.push(Box::new(format!("%{}%", path)));
    }
    sql.push_str(&format!(" ORDER BY caller_file, caller_line LIMIT {}", limit.min(200)));

    let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(rusqlite::params_from_iter(param_refs), |row| {
        Ok(CallEdge {
            caller_file: row.get::<_, String>(0)?,
            caller_symbol: row.get::<_, String>(1)?,
            caller_line: row.get::<_, i64>(2)?,
            callee_name: row.get::<_, String>(3)?,
        })
    })?;

    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn init_in_memory() -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute(
            "CREATE TABLE code_call_graph (
                id INTEGER PRIMARY KEY,
                repo_id TEXT NOT NULL,
                caller_file TEXT NOT NULL,
                caller_symbol TEXT NOT NULL,
                caller_line INTEGER,
                callee_name TEXT NOT NULL
            )",
            [],
        )
        .unwrap();
        conn
    }

    fn seed_edges(conn: &rusqlite::Connection) {
        let edges = [
            ("repo-a", "src/main.rs", "main", 10, "helper"),
            ("repo-a", "src/main.rs", "main", 15, "process"),
            ("repo-a", "src/lib.rs", "helper", 5, "util"),
            ("repo-a", "src/lib.rs", "process", 20, "util"),
            ("repo-b", "src/main.rs", "entry", 1, "init"),
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
    fn test_query_all_edges() {
        let conn = init_in_memory();
        seed_edges(&conn);

        let results = query_call_edges(&conn, "repo-a", None, None, None, 100).unwrap();
        assert_eq!(results.len(), 4);
    }

    #[test]
    fn test_query_by_callee() {
        let conn = init_in_memory();
        seed_edges(&conn);

        let results = query_call_edges(&conn, "repo-a", Some("util"), None, None, 100).unwrap();
        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|e| e.callee_name == "util"));
    }

    #[test]
    fn test_query_by_caller() {
        let conn = init_in_memory();
        seed_edges(&conn);

        let results = query_call_edges(
            &conn, "repo-a", None, Some("main"), None, 100,
        )
        .unwrap();
        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|e| e.caller_symbol == "main"));
    }

    #[test]
    fn test_query_by_file_path() {
        let conn = init_in_memory();
        seed_edges(&conn);

        let results = query_call_edges(
            &conn, "repo-a", None, None, Some("lib"), 100,
        )
        .unwrap();
        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|e| e.caller_file.contains("lib")));
    }

    #[test]
    fn test_query_combined_filters() {
        let conn = init_in_memory();
        seed_edges(&conn);

        let results = query_call_edges(
            &conn, "repo-a", Some("util"), Some("helper"), None, 100,
        )
        .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].caller_symbol, "helper");
        assert_eq!(results[0].callee_name, "util");
    }

    #[test]
    fn test_query_empty_result() {
        let conn = init_in_memory();
        seed_edges(&conn);

        let results = query_call_edges(
            &conn, "repo-a", Some("nonexistent"), None, None, 100,
        )
        .unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_query_cross_repo_isolation() {
        let conn = init_in_memory();
        seed_edges(&conn);

        let repo_a = query_call_edges(&conn, "repo-a", None, None, None, 100).unwrap();
        let repo_b = query_call_edges(&conn, "repo-b", None, None, None, 100).unwrap();
        assert_eq!(repo_a.len(), 4);
        assert_eq!(repo_b.len(), 1);
    }

    #[test]
    fn test_query_limit() {
        let conn = init_in_memory();
        seed_edges(&conn);

        let results = query_call_edges(&conn, "repo-a", None, None, None, 2).unwrap();
        assert_eq!(results.len(), 2);
    }
}
