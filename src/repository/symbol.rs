//! Repository for code symbols and call graphs.

use crate::registry::call_graph::CallEdge;
use crate::registry::code_symbols::CodeSymbol;
use crate::registry::dead_code::DeadFunction;

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
        // TODO: migrate from registry::code_symbols::query_code_symbols
        todo!()
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
        // TODO: migrate from registry::call_graph::query_call_edges
        todo!()
    }

    /// Find potentially dead (unused) functions.
    pub fn query_dead_code(
        &self,
        repo_id: &str,
        include_pub: bool,
        limit: usize,
    ) -> anyhow::Result<Vec<DeadFunction>> {
        // TODO: migrate from registry::dead_code::query_dead_code
        todo!()
    }
}

impl<'a> super::Repository for SymbolRepository<'a> {
    fn conn(&self) -> &rusqlite::Connection {
        self.0
    }
}
