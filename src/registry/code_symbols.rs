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
        "SELECT file_path, symbol_type, name, line_start, line_end, signature \
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
    sql.push_str(&format!(
        " ORDER BY file_path, line_start LIMIT {}",
        limit.min(200)
    ));

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
        })
    })?;

    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}
