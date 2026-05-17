// SPDX-License-Identifier: MIT
// Copyright (c) 2026 juice094
//! Code symbol query helpers.

/// A single code symbol from the `code_symbols` table.
#[derive(Debug, Clone)]
pub struct CodeSymbol {
    pub file_path: String,
    pub symbol_type: String,
    pub name: String,
    pub line_start: i64,
    pub line_end: i64,
    pub signature: Option<String>,
    pub attributes: Option<String>,
}

/// Query code symbols for a specific repository.
pub fn query_code_symbols(
    conn: &rusqlite::Connection,
    repo_id: &str,
    name_filter: Option<&str>,
    symbol_type: Option<&str>,
    file_path: Option<&str>,
    limit: usize,
) -> anyhow::Result<Vec<CodeSymbol>> {
    let mut sql = String::from(
        "SELECT file_path, symbol_type, name, line_start, line_end, signature, attributes \
         FROM code_symbols WHERE repo_id = ?1",
    );
    let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(repo_id.to_string())];

    if let Some(st) = symbol_type.filter(|s| !s.is_empty()) {
        sql.push_str(" AND symbol_type = ?");
        sql.push_str(&(params.len() + 1).to_string());
        params.push(Box::new(st.to_string()));
    }
    if let Some(name) = name_filter.filter(|s| !s.is_empty()) {
        sql.push_str(" AND name LIKE ?");
        sql.push_str(&(params.len() + 1).to_string());
        params.push(Box::new(format!("%{}%", name)));
    }
    if let Some(path) = file_path.filter(|s| !s.is_empty()) {
        sql.push_str(" AND file_path LIKE ?");
        sql.push_str(&(params.len() + 1).to_string());
        params.push(Box::new(format!("%{}%", path)));
    }
    sql.push_str(&format!(" ORDER BY file_path, line_start LIMIT {}", limit.min(200)));

    let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(rusqlite::params_from_iter(param_refs), |row| {
        Ok(CodeSymbol {
            file_path: row.get::<_, String>(0)?,
            symbol_type: row.get::<_, String>(1)?,
            name: row.get::<_, String>(2)?,
            line_start: row.get::<_, i64>(3)?,
            line_end: row.get::<_, i64>(4)?,
            signature: row.get::<_, Option<String>>(5)?,
            attributes: row.get::<_, Option<String>>(6)?,
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
            "CREATE TABLE code_symbols (
                id INTEGER PRIMARY KEY,
                repo_id TEXT NOT NULL,
                file_path TEXT NOT NULL,
                symbol_type TEXT NOT NULL,
                name TEXT NOT NULL,
                line_start INTEGER,
                line_end INTEGER,
                signature TEXT,
                attributes TEXT
            )",
            [],
        )
        .unwrap();
        conn
    }

    fn seed_symbols(conn: &rusqlite::Connection) {
        let symbols = [
            ("repo-a", "src/main.rs", "function", "main", 1, 10, None, None),
            ("repo-a", "src/lib.rs", "function", "helper", 5, 15, Some("fn helper()"), None),
            ("repo-a", "src/lib.rs", "struct", "Config", 20, 30, None, Some("derive(Debug)")),
            ("repo-a", "src/models.rs", "struct", "User", 1, 20, None, None),
            (
                "repo-a",
                "src/models.rs",
                "function",
                "new_user",
                25,
                35,
                Some("fn new_user() -> User"),
                None,
            ),
            ("repo-b", "src/main.rs", "function", "entry", 1, 5, None, None),
        ];
        for (repo, path, ty, name, start, end, sig, attrs) in symbols {
            conn.execute(
                "INSERT INTO code_symbols (repo_id, file_path, symbol_type, name, line_start, line_end, signature, attributes)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                rusqlite::params![repo, path, ty, name, start, end, sig, attrs],
            )
            .unwrap();
        }
    }

    #[test]
    fn test_query_all_symbols() {
        let conn = init_in_memory();
        seed_symbols(&conn);

        let results = query_code_symbols(&conn, "repo-a", None, None, None, 100).unwrap();
        assert_eq!(results.len(), 5);
    }

    #[test]
    fn test_query_by_symbol_type() {
        let conn = init_in_memory();
        seed_symbols(&conn);

        let structs = query_code_symbols(&conn, "repo-a", None, Some("struct"), None, 100).unwrap();
        assert_eq!(structs.len(), 2);
        assert!(structs.iter().all(|s| s.symbol_type == "struct"));
    }

    #[test]
    fn test_query_by_name_filter() {
        let conn = init_in_memory();
        seed_symbols(&conn);

        let results = query_code_symbols(&conn, "repo-a", Some("user"), None, None, 100).unwrap();
        assert_eq!(results.len(), 2); // User and new_user
        assert!(results.iter().any(|s| s.name == "User"));
        assert!(results.iter().any(|s| s.name == "new_user"));
    }

    #[test]
    fn test_query_by_file_path() {
        let conn = init_in_memory();
        seed_symbols(&conn);

        let results = query_code_symbols(&conn, "repo-a", None, None, Some("models"), 100).unwrap();
        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|s| s.file_path.contains("models")));
    }

    #[test]
    fn test_query_combined_filters() {
        let conn = init_in_memory();
        seed_symbols(&conn);

        let results =
            query_code_symbols(&conn, "repo-a", None, Some("struct"), Some("lib"), 100).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "Config");
    }

    #[test]
    fn test_query_empty_result() {
        let conn = init_in_memory();
        seed_symbols(&conn);

        let results =
            query_code_symbols(&conn, "repo-a", Some("nonexistent"), None, None, 100).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_query_limit() {
        let conn = init_in_memory();
        seed_symbols(&conn);

        let results = query_code_symbols(&conn, "repo-a", None, None, None, 3).unwrap();
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn test_query_cross_repo_isolation() {
        let conn = init_in_memory();
        seed_symbols(&conn);

        let repo_a = query_code_symbols(&conn, "repo-a", None, None, None, 100).unwrap();
        let repo_b = query_code_symbols(&conn, "repo-b", None, None, None, 100).unwrap();
        assert_eq!(repo_a.len(), 5);
        assert_eq!(repo_b.len(), 1);
    }

    #[test]
    fn test_query_preserves_optional_fields() {
        let conn = init_in_memory();
        seed_symbols(&conn);

        let results = query_code_symbols(&conn, "repo-a", Some("helper"), None, None, 100).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].signature, Some("fn helper()".to_string()));
        assert_eq!(results[0].attributes, None);

        let results = query_code_symbols(&conn, "repo-a", Some("Config"), None, None, 100).unwrap();
        assert_eq!(results[0].attributes, Some("derive(Debug)".to_string()));
    }
}
