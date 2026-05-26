// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (c) 2026 juice094

// Re-export the external crate type for backward compatibility (src/repository/symbol.rs).
pub use devbase_registry::dead_code::DeadFunction;

/// A potentially dead function from the code symbol index.
#[derive(Debug, Clone)]
pub struct DeadCodeRow {
    pub file_path: String,
    pub name: String,
    pub line_start: i64,
    pub signature: Option<String>,
}

/// Query potentially dead functions for a specific repository.
pub fn query_dead_code(
    conn: &rusqlite::Connection,
    repo_id: &str,
    include_pub: bool,
    limit: usize,
) -> anyhow::Result<Vec<DeadCodeRow>> {
    let mut sql = String::from(
        "SELECT file_path, name, line_start, signature \
         FROM code_symbols cs \
         WHERE cs.repo_id = ?1 AND cs.symbol_type = 'function' \
         AND NOT EXISTS ( \
             SELECT 1 FROM code_call_graph ccg \
             WHERE ccg.repo_id = cs.repo_id AND ccg.callee_name = cs.name \
         )",
    );
    if !include_pub {
        sql.push_str(" AND (cs.signature IS NULL OR cs.signature NOT LIKE 'pub%fn%')");
    }
    sql.push_str(" AND cs.name != 'main'");
    sql.push_str(" AND cs.name NOT LIKE 'test_%'");
    sql.push_str(" AND cs.file_path NOT LIKE '%/tests.rs' AND cs.file_path NOT LIKE '%\\tests.rs'");
    sql.push_str(" AND (cs.attributes IS NULL OR cs.attributes NOT LIKE '%#[test]%')");
    sql.push_str(&format!(" ORDER BY cs.file_path, cs.line_start LIMIT {}", limit.min(200)));

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([repo_id], |row| {
        Ok(DeadCodeRow {
            file_path: row.get::<_, String>(0)?,
            name: row.get::<_, String>(1)?,
            line_start: row.get::<_, i64>(2)?,
            signature: row.get::<_, Option<String>>(3)?,
        })
    })?;

    let mut dead = Vec::new();
    for row in rows {
        dead.push(row?);
    }
    Ok(dead)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_query_dead_code_basic() {
        let conn = crate::registry::WorkspaceRegistry::init_in_memory().unwrap();
        let repo = "r1";

        conn.execute(
            "INSERT INTO code_symbols (repo_id, file_path, symbol_type, name, line_start, signature)
             VALUES (?1, 'src/lib.rs', 'function', 'unused_fn', 10, 'fn unused_fn() {}')",
            [repo],
        )
        .unwrap();

        let dead = query_dead_code(&conn, repo, false, 10).unwrap();
        assert_eq!(dead.len(), 1);
        assert_eq!(dead[0].name, "unused_fn");
    }

    #[test]
    fn test_query_dead_code_excludes_called() {
        let conn = crate::registry::WorkspaceRegistry::init_in_memory().unwrap();
        let repo = "r1";

        conn.execute(
            "INSERT INTO code_symbols (repo_id, file_path, symbol_type, name, line_start, signature)
             VALUES (?1, 'src/lib.rs', 'function', 'called_fn', 10, 'fn called_fn() {}')",
            [repo],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO code_call_graph (repo_id, caller_file, caller_symbol, caller_line, callee_name)
             VALUES (?1, 'src/lib.rs', 'other', 1, 'called_fn')",
            [repo],
        )
        .unwrap();

        let dead = query_dead_code(&conn, repo, false, 10).unwrap();
        assert!(dead.is_empty());
    }

    #[test]
    fn test_query_dead_code_excludes_pub_when_not_include_pub() {
        let conn = crate::registry::WorkspaceRegistry::init_in_memory().unwrap();
        let repo = "r1";

        conn.execute(
            "INSERT INTO code_symbols (repo_id, file_path, symbol_type, name, line_start, signature)
             VALUES (?1, 'src/lib.rs', 'function', 'pub_fn', 10, 'pub fn pub_fn() {}')",
            [repo],
        )
        .unwrap();

        let dead = query_dead_code(&conn, repo, false, 10).unwrap();
        assert!(dead.is_empty());

        let dead = query_dead_code(&conn, repo, true, 10).unwrap();
        assert_eq!(dead.len(), 1);
    }

    #[test]
    fn test_query_dead_code_excludes_main() {
        let conn = crate::registry::WorkspaceRegistry::init_in_memory().unwrap();
        let repo = "r1";

        conn.execute(
            "INSERT INTO code_symbols (repo_id, file_path, symbol_type, name, line_start, signature)
             VALUES (?1, 'src/lib.rs', 'function', 'main', 10, 'fn main() {}')",
            [repo],
        )
        .unwrap();

        let dead = query_dead_code(&conn, repo, true, 10).unwrap();
        assert!(dead.is_empty());
    }

    #[test]
    fn test_query_dead_code_excludes_test_prefix() {
        let conn = crate::registry::WorkspaceRegistry::init_in_memory().unwrap();
        let repo = "r1";

        conn.execute(
            "INSERT INTO code_symbols (repo_id, file_path, symbol_type, name, line_start, signature)
             VALUES (?1, 'src/lib.rs', 'function', 'test_something', 10, 'fn test_something() {}')",
            [repo],
        )
        .unwrap();

        let dead = query_dead_code(&conn, repo, true, 10).unwrap();
        assert!(dead.is_empty());
    }

    #[test]
    fn test_query_dead_code_excludes_tests_rs() {
        let conn = crate::registry::WorkspaceRegistry::init_in_memory().unwrap();
        let repo = "r1";

        conn.execute(
            "INSERT INTO code_symbols (repo_id, file_path, symbol_type, name, line_start, signature)
             VALUES (?1, 'src/tests.rs', 'function', 'helper', 10, 'fn helper() {}')",
            [repo],
        )
        .unwrap();

        let dead = query_dead_code(&conn, repo, true, 10).unwrap();
        assert!(dead.is_empty());
    }

    #[test]
    fn test_query_dead_code_excludes_test_attribute() {
        let conn = crate::registry::WorkspaceRegistry::init_in_memory().unwrap();
        let repo = "r1";

        conn.execute(
            "INSERT INTO code_symbols (repo_id, file_path, symbol_type, name, line_start, signature, attributes)
             VALUES (?1, 'src/lib.rs', 'function', 'my_test', 10, 'fn my_test() {}', '#[test]')",
            [repo],
        )
        .unwrap();

        let dead = query_dead_code(&conn, repo, true, 10).unwrap();
        assert!(dead.is_empty());
    }
}
