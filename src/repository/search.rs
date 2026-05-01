//! Repository for search operations (BM25, semantic, hybrid).

pub struct SearchRepository<'a>(&'a rusqlite::Connection);

impl<'a> SearchRepository<'a> {
    pub fn new(conn: &'a rusqlite::Connection) -> Self {
        Self(conn)
    }

    /// Hybrid search: fuses vector similarity + keyword matching via RRF.
    #[allow(unused_variables)]
    pub fn hybrid_search_symbols(
        &self,
        repo_id: &str,
        query_text: &str,
        query_embedding: Option<&[f32]>,
        limit: usize,
    ) -> anyhow::Result<Vec<(String, String, String, i64, f64)>> {
        // TODO: migrate from mcp/tools/repo.rs DevkitHybridSearchTool
        todo!()
    }

    /// Semantic search using embedding vector.
    #[allow(unused_variables)]
    pub fn semantic_search_symbols(
        &self,
        repo_id: &str,
        query_embedding: &[f32],
        limit: usize,
    ) -> anyhow::Result<Vec<(String, String, String, i64, f64)>> {
        // TODO: migrate from mcp/tools/repo.rs DevkitSemanticSearchTool
        todo!()
    }

    /// Find symbols related to a given symbol name.
    #[allow(unused_variables)]
    pub fn related_symbols(
        &self,
        repo_id: &str,
        symbol_name: &str,
        limit: usize,
    ) -> anyhow::Result<Vec<(String, String, String, i64, f64)>> {
        // TODO: migrate from mcp/tools/repo.rs DevkitRelatedSymbolsTool
        todo!()
    }

    /// Cross-repo search for symbols matching a query.
    #[allow(unused_variables)]
    pub fn cross_repo_search(
        &self,
        query_text: &str,
        limit: usize,
    ) -> anyhow::Result<Vec<(String, String, String, i64, f64)>> {
        // TODO: migrate from mcp/tools/repo.rs DevkitCrossRepoSearchTool
        todo!()
    }
}

impl<'a> super::Repository for SearchRepository<'a> {
    fn conn(&self) -> &rusqlite::Connection {
        self.0
    }
}
