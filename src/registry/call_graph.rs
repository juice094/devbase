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
    sql.push_str(&format!(
        " ORDER BY caller_file, caller_line LIMIT {}",
        limit.min(200)
    ));

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
