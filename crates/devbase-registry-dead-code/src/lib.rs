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

    sql.push_str(&format!(
        " ORDER BY cs.file_path, cs.line_start LIMIT {}",
        limit.min(200)
    ));

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
