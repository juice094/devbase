//! Repository for code symbols and call graphs.

use crate::registry::call_graph::CallEdge;
use crate::registry::code_symbols::CodeSymbol;
use crate::registry::dead_code::DeadFunction;
use crate::repository::Repository;

pub struct SymbolRepository<'a>(&'a rusqlite::Connection);

impl<'a> SymbolRepository<'a> {
    pub fn new(conn: &'a rusqlite::Connection) -> Self {
        Self(conn)
    }

    /// Query code symbols with optional filters.
    pub fn query_code_symbols(
        &self,
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
        let mut stmt = self.conn().prepare(&sql)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(param_refs), |row: &rusqlite::Row| {
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

    /// Query call edges for a repo.
    pub fn query_call_graph(
        &self,
        repo_id: &str,
        callee: Option<&str>,
        caller: Option<&str>,
        file_path: Option<&str>,
        limit: usize,
    ) -> anyhow::Result<Vec<CallEdge>> {
        let mut sql = String::from(
            "SELECT caller_file, caller_symbol, caller_line, callee_name \
             FROM code_call_graph WHERE repo_id = ?1",
        );
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(repo_id.to_string())];

        if let Some(name) = callee.filter(|s| !s.is_empty()) {
            sql.push_str(" AND callee_name = ?");
            sql.push_str(&(params.len() + 1).to_string());
            params.push(Box::new(name.to_string()));
        }
        if let Some(name) = caller.filter(|s| !s.is_empty()) {
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
        let mut stmt = self.conn().prepare(&sql)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(param_refs), |row: &rusqlite::Row| {
            Ok(CallEdge {
                caller_file: row.get::<_, String>(0)?,
                caller_symbol: row.get::<_, String>(1)?,
                caller_line: row.get::<_, i64>(2)?,
                callee_name: row.get::<_, String>(3)?,
            })
        })?;

        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    /// Find potentially dead (unused) functions.
    pub fn query_dead_code(
        &self,
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
        let mut stmt = self.conn().prepare(&sql)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(param_refs), |row: &rusqlite::Row| {
            Ok(DeadFunction {
                file_path: row.get::<_, String>(0)?,
                name: row.get::<_, String>(1)?,
                line_start: row.get::<_, i64>(2)?,
                signature: row.get::<_, Option<String>>(3)?,
            })
        })?;

        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }
}

impl<'a> super::Repository for SymbolRepository<'a> {
    fn conn(&self) -> &rusqlite::Connection {
        self.0
    }
}
