//! Repository for search operations (BM25, semantic, hybrid).

use crate::repository::Repository;
use std::collections::HashMap;

/// (repo_id, symbol_name, file_path, line_start, score)
pub type SearchResultRow = (String, String, String, i64, f64);

pub struct SearchRepository<'a>(&'a rusqlite::Connection);

impl<'a> SearchRepository<'a> {
    pub fn new(conn: &'a rusqlite::Connection) -> Self {
        Self(conn)
    }

    /// Hybrid search: fuses vector similarity + keyword matching via RRF.
    pub fn hybrid_search_symbols(
        &self,
        repo_id: &str,
        query_text: &str,
        query_embedding: Option<&[f32]>,
        limit: usize,
    ) -> anyhow::Result<Vec<SearchResultRow>> {
        let mut results = crate::search::hybrid::hybrid_search_symbols(
            self.conn(),
            repo_id,
            query_text,
            query_embedding,
            limit,
        )?;

        // Boost by agent read frequency (behavioral signal)
        if results.len() > 1 {
            let names: Vec<String> = results.iter().map(|r| r.1.clone()).collect();
            let counts =
                crate::registry::knowledge::get_symbol_read_counts(self.conn(), repo_id, &names)?;
            for row in &mut results {
                if let Some(cnt) = counts.get(&row.1) {
                    let boost = (*cnt as f32 * 0.05).min(0.5);
                    row.4 += boost;
                }
            }
            results.sort_by(|a, b| {
                b.4.partial_cmp(&a.4)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| a.1.cmp(&b.1))
            });
        }

        Ok(results
            .into_iter()
            .map(|(r, n, p, l, s)| (r, n, p, l, s as f64))
            .collect())
    }

    /// Semantic search using embedding vector.
    pub fn semantic_search_symbols(
        &self,
        repo_id: &str,
        query_embedding: &[f32],
        limit: usize,
    ) -> anyhow::Result<Vec<SearchResultRow>> {
        let mut stmt = self.conn().prepare(
            "SELECT ce.symbol_name, cs.file_path, cs.line_start, ce.embedding
             FROM code_embeddings ce
             JOIN code_symbols cs ON ce.repo_id = cs.repo_id
                 AND ce.symbol_name = cs.name
             WHERE ce.repo_id = ?1 AND cs.symbol_type = 'function'
             ORDER BY ce.symbol_name",
        )?;
        let rows = stmt.query_map([repo_id], |row: &rusqlite::Row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, Vec<u8>>(3)?,
            ))
        })?;

        let mut scored: Vec<(String, String, i64, f32)> = Vec::new();
        for row in rows {
            let (symbol_name, file_path, line_start, blob) = row?;
            let emb = crate::embedding::bytes_to_embedding(&blob);
            let sim = crate::embedding::cosine_similarity(query_embedding, &emb);
            scored.push((symbol_name, file_path, line_start, sim));
        }

        scored.sort_by(|a, b| b.3.partial_cmp(&a.3).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(limit);

        Ok(scored
            .into_iter()
            .map(|(name, path, line, sim)| (repo_id.to_string(), name, path, line, sim as f64))
            .collect())
    }

    /// Find symbols related to a given symbol name.
    pub fn related_symbols(
        &self,
        repo_id: &str,
        symbol_name: &str,
        limit: usize,
    ) -> anyhow::Result<Vec<SearchResultRow>> {
        let mut stmt = self.conn().prepare(
            "SELECT target_repo, target_symbol, strength
             FROM code_symbol_links
             WHERE source_repo = ?1 AND source_symbol = ?2
             ORDER BY strength DESC
             LIMIT ?3",
        )?;
        let rows = stmt.query_map(
            rusqlite::params![repo_id, symbol_name, limit as i64],
            |row: &rusqlite::Row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, f64>(2)?,
                ))
            },
        )?;

        let mut results = Vec::new();
        for row in rows {
            let (target_repo, target_symbol, strength) = row?;
            // Resolve file_path and line_start from code_symbols if available.
            let (file_path, line_start) = self
                .conn()
                .query_row(
                    "SELECT file_path, line_start
                     FROM code_symbols
                     WHERE repo_id = ?1 AND name = ?2
                     LIMIT 1",
                    rusqlite::params![&target_repo, &target_symbol],
                    |r: &rusqlite::Row| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)),
                )
                .unwrap_or_else(|_| (String::new(), 0));
            results.push((target_repo, target_symbol, file_path, line_start, strength));
        }
        Ok(results)
    }

    /// Cross-repo search for symbols matching a query.
    pub fn cross_repo_search(
        &self,
        query_text: &str,
        limit: usize,
    ) -> anyhow::Result<Vec<SearchResultRow>> {
        // 1. Find all repos (empty tags semantics).
        let mut stmt = self.conn().prepare(&format!(
            "SELECT id FROM entities WHERE entity_type = '{}'",
            crate::registry::ENTITY_TYPE_REPO
        ))?;
        let rows = stmt.query_map([], |row: &rusqlite::Row| row.get::<_, String>(0))?;
        let repo_ids: Vec<String> = rows.collect::<Result<Vec<_>, _>>()?;

        if repo_ids.is_empty() {
            return Ok(Vec::new());
        }

        // 2. Search each repo (generous per-repo limit before global dedup)
        let per_repo_limit = limit.max(10) * 2;
        let mut all_results = Vec::new();
        for repo_id in repo_ids {
            match crate::search::hybrid::hybrid_search_symbols(
                self.conn(),
                &repo_id,
                query_text,
                None,
                per_repo_limit,
            ) {
                Ok(mut results) => all_results.append(&mut results),
                Err(e) => {
                    tracing::warn!("Cross-repo search failed for {}: {}", repo_id, e);
                }
            }
        }

        // 3. Deduplicate and sort globally by score
        let mut deduped: HashMap<String, crate::semantic_index::SemanticSearchRow> = HashMap::new();
        for row in all_results {
            let key = format!("{}::{}::{}", row.0, row.1, row.2);
            deduped.entry(key).or_insert(row);
        }

        let mut merged: Vec<crate::semantic_index::SemanticSearchRow> = deduped.into_values().collect();
        merged.sort_by(|a, b| {
            b.4.partial_cmp(&a.4)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.1.cmp(&b.1))
        });
        merged.truncate(limit);

        Ok(merged
            .into_iter()
            .map(|(r, n, p, l, s)| (r, n, p, l, s as f64))
            .collect())
    }
}

impl<'a> super::Repository for SearchRepository<'a> {
    fn conn(&self) -> &rusqlite::Connection {
        self.0
    }
}
