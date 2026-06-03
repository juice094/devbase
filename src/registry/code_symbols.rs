// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (c) 2026 juice094

// Re-export the external crate type for backward compatibility (src/repository/symbol.rs).
pub use devbase_registry::code_symbols::CodeSymbol;

/// A single code symbol from the `code_symbols` table (RegistryClient variant).
#[derive(Debug, Clone)]
pub struct CodeSymbolRow {
    pub file_path: String,
    pub symbol_type: String,
    pub name: String,
    pub line_start: i64,
    pub line_end: i64,
    pub signature: Option<String>,
}

/// Query code symbols for a specific repository.
pub fn query_code_symbols(
    conn: &rusqlite::Connection,
    repo_id: &str,
    name: Option<&str>,
    symbol_type: Option<&str>,
    file: Option<&str>,
    limit: usize,
) -> anyhow::Result<Vec<CodeSymbolRow>> {
    let mut sql = String::from(
        "SELECT file_path, symbol_type, name, line_start, line_end, signature \
         FROM code_symbols WHERE repo_id = ?1",
    );
    let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(repo_id.to_string())];
    if let Some(ty) = symbol_type.filter(|s| !s.is_empty()) {
        sql.push_str(" AND symbol_type = ?");
        sql.push_str(&(params.len() + 1).to_string());
        params.push(Box::new(ty.to_string()));
    }
    if let Some(n) = name.filter(|s| !s.is_empty()) {
        sql.push_str(" AND name LIKE ?");
        sql.push_str(&(params.len() + 1).to_string());
        params.push(Box::new(format!("%{}%", n)));
    }
    if let Some(f) = file.filter(|s| !s.is_empty()) {
        sql.push_str(" AND file_path LIKE ?");
        sql.push_str(&(params.len() + 1).to_string());
        params.push(Box::new(format!("%{}%", f)));
    }
    sql.push_str(&format!(" ORDER BY file_path, line_start LIMIT {}", limit.min(200)));

    let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(rusqlite::params_from_iter(param_refs), |row| {
        Ok(CodeSymbolRow {
            file_path: row.get::<_, String>(0)?,
            symbol_type: row.get::<_, String>(1)?,
            name: row.get::<_, String>(2)?,
            line_start: row.get::<_, i64>(3)?,
            line_end: row.get::<_, i64>(4)?,
            signature: row.get::<_, Option<String>>(5)?,
        })
    })?;

    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_query_code_symbols_no_filter() {
        let conn = crate::registry::WorkspaceRegistry::init_in_memory().unwrap();
        conn.execute(
            "INSERT INTO code_symbols (repo_id, file_path, symbol_type, name, line_start, line_end, signature)
             VALUES ('r1', 'src/lib.rs', 'function', 'foo', 10, 20, 'fn foo() {}')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO code_symbols (repo_id, file_path, symbol_type, name, line_start, line_end, signature)
             VALUES ('r1', 'src/lib.rs', 'struct', 'Bar', 30, 40, 'struct Bar;')",
            [],
        )
        .unwrap();

        let rows = query_code_symbols(&conn, "r1", None, None, None, 10).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].name, "foo");
        assert_eq!(rows[1].name, "Bar");
    }

    #[test]
    fn test_query_code_symbols_by_symbol_type() {
        let conn = crate::registry::WorkspaceRegistry::init_in_memory().unwrap();
        conn.execute(
            "INSERT INTO code_symbols (repo_id, file_path, symbol_type, name, line_start, line_end, signature)
             VALUES ('r1', 'src/lib.rs', 'function', 'foo', 10, 20, 'fn foo() {}')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO code_symbols (repo_id, file_path, symbol_type, name, line_start, line_end, signature)
             VALUES ('r1', 'src/lib.rs', 'struct', 'Bar', 30, 40, 'struct Bar;')",
            [],
        )
        .unwrap();

        let rows = query_code_symbols(&conn, "r1", None, Some("function"), None, 10).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "foo");
    }

    #[test]
    fn test_query_code_symbols_by_name() {
        let conn = crate::registry::WorkspaceRegistry::init_in_memory().unwrap();
        conn.execute(
            "INSERT INTO code_symbols (repo_id, file_path, symbol_type, name, line_start, line_end, signature)
             VALUES ('r1', 'src/lib.rs', 'function', 'foobar', 10, 20, 'fn foobar() {}')",
            [],
        )
        .unwrap();

        let rows = query_code_symbols(&conn, "r1", Some("oob"), None, None, 10).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "foobar");
    }

    #[test]
    fn test_query_code_symbols_by_file() {
        let conn = crate::registry::WorkspaceRegistry::init_in_memory().unwrap();
        conn.execute(
            "INSERT INTO code_symbols (repo_id, file_path, symbol_type, name, line_start, line_end, signature)
             VALUES ('r1', 'src/main.rs', 'function', 'foo', 10, 20, 'fn foo() {}')",
            [],
        )
        .unwrap();

        let rows = query_code_symbols(&conn, "r1", None, None, Some("main"), 10).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].file_path, "src/main.rs");
    }

    #[test]
    fn test_query_code_symbols_limit() {
        let conn = crate::registry::WorkspaceRegistry::init_in_memory().unwrap();
        for i in 0..5 {
            conn.execute(
                &format!(
                    "INSERT INTO code_symbols (repo_id, file_path, symbol_type, name, line_start, line_end, signature)
                     VALUES ('r1', 'src/lib.rs', 'function', 'f{}', {}, {}, 'fn f{}() {{}}')",
                    i, i, i + 1, i
                ),
                [],
            )
            .unwrap();
        }

        let rows = query_code_symbols(&conn, "r1", None, None, None, 3).unwrap();
        assert_eq!(rows.len(), 3);
    }
}
