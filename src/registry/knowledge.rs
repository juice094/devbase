use super::*;
use chrono::{DateTime, Utc};

pub fn save_summary(
    conn: &rusqlite::Connection,
    repo_id: &str,
    summary: &str,
    keywords: &str,
) -> anyhow::Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO repo_summaries (repo_id, summary, keywords, generated_at) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![repo_id, summary, keywords, Utc::now().to_rfc3339()],
    )?;
    Ok(())
}

pub fn save_modules(
    conn: &mut rusqlite::Connection,
    repo_id: &str,
    modules: &[(String, String)],
) -> anyhow::Result<()> {
    let tx = conn.transaction()?;
    tx.execute("DELETE FROM repo_modules WHERE repo_id = ?1", [repo_id])?;
    for (module_name, module_type) in modules {
        tx.execute(
            "INSERT OR REPLACE INTO repo_modules (repo_id, module_name, module_type, module_path) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![repo_id, module_name, module_type, module_name],
        )?;
    }
    tx.commit()?;
    Ok(())
}

pub fn save_module(
    conn: &rusqlite::Connection,
    repo_id: &str,
    module_name: &str,
    module_type: &str,
    module_path: &str,
) -> anyhow::Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO repo_modules (repo_id, module_name, module_type, module_path)
         VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![repo_id, module_name, module_type, module_path],
    )?;
    Ok(())
}

pub fn list_modules(
    conn: &rusqlite::Connection,
    repo_id: &str,
) -> anyhow::Result<Vec<(String, String, String)>> {
    let mut stmt = conn.prepare(
        "SELECT module_name, module_type, module_path FROM repo_modules WHERE repo_id = ?1",
    )?;
    let rows = stmt.query_map([repo_id], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?))
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

pub fn clear_modules(conn: &rusqlite::Connection, repo_id: &str) -> anyhow::Result<()> {
    conn.execute("DELETE FROM repo_modules WHERE repo_id = ?1", [repo_id])?;
    Ok(())
}

pub fn save_discovery(
    conn: &rusqlite::Connection,
    repo_id: Option<&str>,
    dtype: &str,
    desc: &str,
    confidence: f64,
) -> anyhow::Result<()> {
    conn.execute(
        "INSERT INTO ai_discoveries (repo_id, discovery_type, description, confidence, timestamp) VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![repo_id, dtype, desc, confidence, Utc::now().to_rfc3339()],
    )?;
    Ok(())
}

pub fn save_note(
    conn: &rusqlite::Connection,
    repo_id: &str,
    text: &str,
    author: &str,
) -> anyhow::Result<()> {
    conn.execute(
        "INSERT INTO repo_notes (repo_id, note_text, author, timestamp) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![repo_id, text, author, Utc::now().to_rfc3339()],
    )?;
    Ok(())
}

// ------------------------------------------------------------------
// Papers
// ------------------------------------------------------------------
pub fn save_paper(conn: &rusqlite::Connection, paper: &PaperEntry) -> anyhow::Result<()> {
    // Phase 2 Stage E: entities is the sole source of truth for papers
    let metadata = serde_json::json!({
        "authors": paper.authors,
        "venue": paper.venue,
        "year": paper.year,
        "bibtex": paper.bibtex,
        "tags": paper.tags,
        "added_at": paper.added_at.to_rfc3339(),
    });
    crate::registry::upsert_entity(
        conn,
        &paper.id,
        crate::registry::ENTITY_TYPE_PAPER,
        &paper.title,
        paper.pdf_path.as_deref(),
        &metadata,
    )?;
    Ok(())
}

pub fn list_papers(conn: &rusqlite::Connection) -> anyhow::Result<Vec<PaperEntry>> {
    let mut stmt = conn.prepare(
        "SELECT e.id, e.name, json_extract(e.metadata, '$.authors'),
                json_extract(e.metadata, '$.venue'), json_extract(e.metadata, '$.year'),
                e.local_path, json_extract(e.metadata, '$.bibtex'),
                json_extract(e.metadata, '$.tags'), json_extract(e.metadata, '$.added_at')
         FROM entities e
         WHERE e.entity_type = ?1
         ORDER BY json_extract(e.metadata, '$.added_at') DESC",
    )?;
    let rows = stmt.query_map([crate::registry::ENTITY_TYPE_PAPER], |row| {
        let tags: Option<String> = row.get(7)?;
        Ok(PaperEntry {
            id: row.get(0)?,
            title: row.get(1)?,
            authors: row.get(2)?,
            venue: row.get(3)?,
            year: row.get(4)?,
            pdf_path: row.get(5)?,
            bibtex: row.get(6)?,
            tags: tags
                .map(|s| {
                    s.split(',').map(|t| t.trim().to_string()).filter(|t| !t.is_empty()).collect()
                })
                .unwrap_or_default(),
            added_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(8)?)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

pub fn find_papers_by_venue(
    conn: &rusqlite::Connection,
    venue: &str,
) -> anyhow::Result<Vec<PaperEntry>> {
    let mut stmt = conn.prepare(
        "SELECT e.id, e.name, json_extract(e.metadata, '$.authors'),
                json_extract(e.metadata, '$.venue'), json_extract(e.metadata, '$.year'),
                e.local_path, json_extract(e.metadata, '$.bibtex'),
                json_extract(e.metadata, '$.tags'), json_extract(e.metadata, '$.added_at')
         FROM entities e
         WHERE e.entity_type = ?1 AND json_extract(e.metadata, '$.venue') = ?2
         ORDER BY json_extract(e.metadata, '$.year') DESC",
    )?;
    let rows = stmt.query_map([crate::registry::ENTITY_TYPE_PAPER, venue], |row| {
        let tags: Option<String> = row.get(7)?;
        Ok(PaperEntry {
            id: row.get(0)?,
            title: row.get(1)?,
            authors: row.get(2)?,
            venue: row.get(3)?,
            year: row.get(4)?,
            pdf_path: row.get(5)?,
            bibtex: row.get(6)?,
            tags: tags
                .map(|s| {
                    s.split(',').map(|t| t.trim().to_string()).filter(|t| !t.is_empty()).collect()
                })
                .unwrap_or_default(),
            added_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(8)?)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

// ------------------------------------------------------------------
// Experiments
// ------------------------------------------------------------------
pub fn save_experiment(conn: &rusqlite::Connection, exp: &ExperimentEntry) -> anyhow::Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO experiments (id, repo_id, paper_id, config_json, result_path, git_commit, syncthing_folder_id, status, timestamp) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        rusqlite::params![
            &exp.id,
            exp.repo_id.as_ref(),
            exp.paper_id.as_ref(),
            exp.config_json.as_ref(),
            exp.result_path.as_ref(),
            exp.git_commit.as_ref(),
            exp.syncthing_folder_id.as_ref(),
            &exp.status,
            exp.timestamp.to_rfc3339()
        ],
    )?;
    Ok(())
}

pub fn list_experiments(conn: &rusqlite::Connection) -> anyhow::Result<Vec<ExperimentEntry>> {
    let mut stmt = conn.prepare(
        "SELECT id, repo_id, paper_id, config_json, result_path, git_commit, syncthing_folder_id, status, timestamp FROM experiments ORDER BY timestamp DESC"
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(ExperimentEntry {
            id: row.get(0)?,
            repo_id: row.get(1)?,
            paper_id: row.get(2)?,
            config_json: row.get(3)?,
            result_path: row.get(4)?,
            git_commit: row.get(5)?,
            syncthing_folder_id: row.get(6)?,
            status: row.get(7)?,
            timestamp: DateTime::parse_from_rfc3339(&row.get::<_, String>(8)?)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

pub fn find_experiments_by_repo(
    conn: &rusqlite::Connection,
    repo_id: &str,
) -> anyhow::Result<Vec<ExperimentEntry>> {
    let mut stmt = conn.prepare(
        "SELECT id, repo_id, paper_id, config_json, result_path, git_commit, syncthing_folder_id, status, timestamp FROM experiments WHERE repo_id = ?1 ORDER BY timestamp DESC"
    )?;
    let rows = stmt.query_map([repo_id], |row| {
        Ok(ExperimentEntry {
            id: row.get(0)?,
            repo_id: row.get(1)?,
            paper_id: row.get(2)?,
            config_json: row.get(3)?,
            result_path: row.get(4)?,
            git_commit: row.get(5)?,
            syncthing_folder_id: row.get(6)?,
            status: row.get(7)?,
            timestamp: DateTime::parse_from_rfc3339(&row.get::<_, String>(8)?)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

// ------------------------------------------------------------------
// Code Embeddings (semantic vector search)
// ------------------------------------------------------------------

pub fn save_embeddings(
    conn: &mut rusqlite::Connection,
    repo_id: &str,
    embeddings: &[(String, Vec<f32>)],
) -> anyhow::Result<usize> {
    let tx = conn.transaction()?;
    tx.execute("DELETE FROM code_embeddings WHERE repo_id = ?1", [repo_id])?;
    let now = Utc::now().to_rfc3339();
    let mut inserted = 0;
    for (symbol_name, vec) in embeddings {
        let blob = crate::embedding::embedding_to_bytes(vec);
        tx.execute(
            "INSERT INTO code_embeddings (repo_id, file_path, symbol_name, embedding, generated_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(repo_id, file_path, symbol_name) DO UPDATE SET
             embedding = excluded.embedding,
             generated_at = excluded.generated_at",
            rusqlite::params![repo_id, "", symbol_name, blob, &now],
        )?;
        inserted += 1;
    }
    tx.commit()?;
    Ok(inserted)
}

/// Cross-repo symbol search filtered by tags.
///
/// Searches across all repos that match *all* specified tags.
/// If `tags` is empty, searches across all repos.
/// Results are deduplicated by (repo_id, symbol_name, file_path) and
/// sorted by score descending.
pub fn cross_repo_search_symbols(
    conn: &rusqlite::Connection,
    tags: &[String],
    query_text: &str,
    query_embedding: Option<&[f32]>,
    limit: usize,
) -> anyhow::Result<Vec<crate::semantic_index::SemanticSearchRow>> {
    use std::collections::HashMap;

    // 1. Find repos matching all tags (INTERSECT for AND semantics).
    // Tags are matched against both repo_tags.tag AND repos.language.
    let repo_ids: Vec<String> = if tags.is_empty() {
        let mut stmt = conn.prepare(&format!(
            "SELECT id FROM entities WHERE entity_type = '{}'",
            crate::registry::ENTITY_TYPE_REPO
        ))?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        rows.collect::<Result<Vec<_>, _>>()?
    } else {
        let mut sql = String::new();
        for (i, _) in tags.iter().enumerate() {
            if i > 0 {
                sql.push_str(" INTERSECT ");
            }
            // Match against repo_tags or repos.language
            sql.push_str(&format!(
                "SELECT repo_id FROM repo_tags WHERE LOWER(tag) = LOWER(?) \
                 UNION \
                 SELECT id AS repo_id FROM entities WHERE entity_type = '{}' AND LOWER(language) = LOWER(?)",
                crate::registry::ENTITY_TYPE_REPO
            ));
        }
        let mut stmt = conn.prepare(&sql)?;
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
        for tag in tags {
            params.push(Box::new(tag.clone()));
            params.push(Box::new(tag.clone()));
        }
        let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        let rows = stmt.query_map(param_refs.as_slice(), |row| row.get::<_, String>(0))?;
        rows.collect::<Result<Vec<_>, _>>()?
    };

    if repo_ids.is_empty() {
        return Ok(Vec::new());
    }

    // 2. Search each repo (generous per-repo limit before global dedup)
    let per_repo_limit = limit.max(10) * 2;
    let mut all_results = Vec::new();
    for repo_id in repo_ids {
        match crate::search::hybrid::hybrid_search_symbols(
            conn,
            &repo_id,
            query_text,
            query_embedding,
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
    Ok(merged)
}

/// Hybrid search: vector similarity + keyword matching with RRF merge.
/// Falls back to pure keyword search when no embeddings are available.
/// Results are boosted by agent read frequency (behavioral signal).
pub fn hybrid_search_symbols(
    conn: &rusqlite::Connection,
    repo_id: &str,
    query_text: &str,
    query_embedding: Option<&[f32]>,
    limit: usize,
) -> anyhow::Result<Vec<crate::semantic_index::SemanticSearchRow>> {
    let mut results = crate::search::hybrid::hybrid_search_symbols(
        conn,
        repo_id,
        query_text,
        query_embedding,
        limit,
    )?;

    // Boost by agent read frequency (behavioral signal)
    if results.len() > 1 {
        let names: Vec<String> = results.iter().map(|r| r.1.clone()).collect();
        let counts = get_symbol_read_counts(conn, repo_id, &names)?;
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

    Ok(results)
}

/// Record that an agent read a symbol (for behavioral relevance tracking).
pub fn record_symbol_read(
    conn: &rusqlite::Connection,
    repo_id: &str,
    symbol_name: &str,
    context: Option<&str>,
) -> anyhow::Result<()> {
    conn.execute(
        "INSERT INTO agent_symbol_reads (repo_id, symbol_name, read_at, context)
         VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![repo_id, symbol_name, Utc::now().to_rfc3339(), context],
    )?;
    Ok(())
}

/// Get read counts for a set of symbols in a repo.
pub fn get_symbol_read_counts(
    conn: &rusqlite::Connection,
    repo_id: &str,
    symbol_names: &[String],
) -> anyhow::Result<std::collections::HashMap<String, i64>> {
    if symbol_names.is_empty() {
        return Ok(std::collections::HashMap::new());
    }
    let placeholders = symbol_names.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let sql = format!(
        "SELECT symbol_name, COUNT(*) as cnt
         FROM agent_symbol_reads
         WHERE repo_id = ?1 AND symbol_name IN ({})
         GROUP BY symbol_name",
        placeholders
    );
    let mut stmt = conn.prepare(&sql)?;
    let mut params: Vec<&dyn rusqlite::ToSql> = vec![&repo_id];
    for name in symbol_names {
        params.push(name);
    }
    let rows = stmt.query_map(params.as_slice(), |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
    })?;
    let mut counts = std::collections::HashMap::new();
    for row in rows {
        let (name, cnt) = row?;
        counts.insert(name, cnt);
    }
    Ok(counts)
}

/// Find symbols explicitly linked to the given symbol.
/// Returns Vec<(source_repo, source_symbol, target_repo, target_symbol, link_type, strength)>.
#[allow(clippy::type_complexity)]
pub fn find_related_symbols(
    conn: &rusqlite::Connection,
    repo_id: &str,
    symbol_name: &str,
    limit: usize,
) -> anyhow::Result<Vec<(String, String, String, String, String, f32)>> {
    let mut stmt = conn.prepare(
        "SELECT target_repo, target_symbol, link_type, strength
         FROM code_symbol_links
         WHERE source_repo = ?1 AND source_symbol = ?2
         ORDER BY strength DESC
         LIMIT ?3",
    )?;
    let rows = stmt.query_map(rusqlite::params![repo_id, symbol_name, limit as i64], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, f64>(3)? as f32,
        ))
    })?;

    let mut results = Vec::new();
    for row in rows {
        let (target_repo, target_symbol, link_type, strength) = row?;
        results.push((
            repo_id.to_string(),
            symbol_name.to_string(),
            target_repo,
            target_symbol,
            link_type,
            strength,
        ));
    }
    Ok(results)
}

/// Search for symbols semantically similar to the query embedding.
/// Returns Vec<(repo_id, symbol_name, file_path, line_start, similarity_score)>.
pub fn semantic_search_symbols(
    conn: &rusqlite::Connection,
    repo_id: &str,
    query_embedding: &[f32],
    limit: usize,
) -> anyhow::Result<Vec<crate::semantic_index::SemanticSearchRow>> {
    let mut stmt = conn.prepare(
        "SELECT ce.symbol_name, ce.file_path, cs.line_start, ce.embedding
         FROM code_embeddings ce
         JOIN code_symbols cs ON ce.repo_id = cs.repo_id
             AND ce.file_path = cs.file_path
             AND ce.symbol_name = cs.name
         WHERE ce.repo_id = ?1 AND cs.symbol_type = 'function'
         ORDER BY ce.symbol_name",
    )?;
    let rows = stmt.query_map([repo_id], |row| {
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
        .map(|(name, path, line, sim)| (repo_id.to_string(), name, path, line, sim))
        .collect())
}

impl WorkspaceRegistry {
    pub fn save_summary(
        conn: &rusqlite::Connection,
        repo_id: &str,
        summary: &str,
        keywords: &str,
    ) -> anyhow::Result<()> {
        save_summary(conn, repo_id, summary, keywords)
    }
    pub fn save_modules(
        conn: &mut rusqlite::Connection,
        repo_id: &str,
        modules: &[(String, String)],
    ) -> anyhow::Result<()> {
        save_modules(conn, repo_id, modules)
    }
    pub fn save_module(
        conn: &rusqlite::Connection,
        repo_id: &str,
        module_name: &str,
        module_type: &str,
        module_path: &str,
    ) -> anyhow::Result<()> {
        save_module(conn, repo_id, module_name, module_type, module_path)
    }
    pub fn list_modules(
        conn: &rusqlite::Connection,
        repo_id: &str,
    ) -> anyhow::Result<Vec<(String, String, String)>> {
        list_modules(conn, repo_id)
    }
    pub fn clear_modules(conn: &rusqlite::Connection, repo_id: &str) -> anyhow::Result<()> {
        clear_modules(conn, repo_id)
    }
    pub fn save_discovery(
        conn: &rusqlite::Connection,
        repo_id: Option<&str>,
        dtype: &str,
        desc: &str,
        confidence: f64,
    ) -> anyhow::Result<()> {
        save_discovery(conn, repo_id, dtype, desc, confidence)
    }
    pub fn save_note(
        conn: &rusqlite::Connection,
        repo_id: &str,
        text: &str,
        author: &str,
    ) -> anyhow::Result<()> {
        save_note(conn, repo_id, text, author)
    }
    pub fn save_paper(conn: &rusqlite::Connection, paper: &PaperEntry) -> anyhow::Result<()> {
        save_paper(conn, paper)
    }
    pub fn list_papers(conn: &rusqlite::Connection) -> anyhow::Result<Vec<PaperEntry>> {
        list_papers(conn)
    }
    pub fn find_papers_by_venue(
        conn: &rusqlite::Connection,
        venue: &str,
    ) -> anyhow::Result<Vec<PaperEntry>> {
        find_papers_by_venue(conn, venue)
    }
    pub fn save_experiment(
        conn: &rusqlite::Connection,
        exp: &ExperimentEntry,
    ) -> anyhow::Result<()> {
        save_experiment(conn, exp)
    }
    pub fn list_experiments(conn: &rusqlite::Connection) -> anyhow::Result<Vec<ExperimentEntry>> {
        list_experiments(conn)
    }
    pub fn find_experiments_by_repo(
        conn: &rusqlite::Connection,
        repo_id: &str,
    ) -> anyhow::Result<Vec<ExperimentEntry>> {
        find_experiments_by_repo(conn, repo_id)
    }
    pub fn save_embeddings(
        conn: &mut rusqlite::Connection,
        repo_id: &str,
        embeddings: &[(String, Vec<f32>)],
    ) -> anyhow::Result<usize> {
        save_embeddings(conn, repo_id, embeddings)
    }
    pub fn cross_repo_search_symbols(
        conn: &rusqlite::Connection,
        tags: &[String],
        query_text: &str,
        query_embedding: Option<&[f32]>,
        limit: usize,
    ) -> anyhow::Result<Vec<crate::semantic_index::SemanticSearchRow>> {
        cross_repo_search_symbols(conn, tags, query_text, query_embedding, limit)
    }
    pub fn hybrid_search_symbols(
        conn: &rusqlite::Connection,
        repo_id: &str,
        query_text: &str,
        query_embedding: Option<&[f32]>,
        limit: usize,
    ) -> anyhow::Result<Vec<crate::semantic_index::SemanticSearchRow>> {
        hybrid_search_symbols(conn, repo_id, query_text, query_embedding, limit)
    }
    pub fn record_symbol_read(
        conn: &rusqlite::Connection,
        repo_id: &str,
        symbol_name: &str,
        context: Option<&str>,
    ) -> anyhow::Result<()> {
        record_symbol_read(conn, repo_id, symbol_name, context)
    }
    pub fn get_symbol_read_counts(
        conn: &rusqlite::Connection,
        repo_id: &str,
        symbol_names: &[String],
    ) -> anyhow::Result<std::collections::HashMap<String, i64>> {
        get_symbol_read_counts(conn, repo_id, symbol_names)
    }
    pub fn find_related_symbols(
        conn: &rusqlite::Connection,
        repo_id: &str,
        symbol_name: &str,
        limit: usize,
    ) -> anyhow::Result<Vec<(String, String, String, String, String, f32)>> {
        find_related_symbols(conn, repo_id, symbol_name, limit)
    }
    pub fn semantic_search_symbols(
        conn: &rusqlite::Connection,
        repo_id: &str,
        query_embedding: &[f32],
        limit: usize,
    ) -> anyhow::Result<Vec<crate::semantic_index::SemanticSearchRow>> {
        semantic_search_symbols(conn, repo_id, query_embedding, limit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::WorkspaceRegistry;

    #[test]
    fn test_save_summary_smoke() {
        let mut conn = WorkspaceRegistry::init_in_memory().unwrap();
        WorkspaceRegistry::seed_test_repo(&mut conn, "repo-a").unwrap();
        save_summary(&conn, "repo-a", "A Rust CLI tool", "rust,cli").unwrap();
    }

    #[test]
    fn test_module_crud() {
        let conn = WorkspaceRegistry::init_in_memory().unwrap();
        save_module(&conn, "repo-a", "main", "binary", "src/main.rs").unwrap();
        save_module(&conn, "repo-a", "lib", "library", "src/lib.rs").unwrap();

        let modules = list_modules(&conn, "repo-a").unwrap();
        assert_eq!(modules.len(), 2);

        clear_modules(&conn, "repo-a").unwrap();
        let empty = list_modules(&conn, "repo-a").unwrap();
        assert!(empty.is_empty());
    }

    #[test]
    fn test_paper_roundtrip() {
        let conn = WorkspaceRegistry::init_in_memory().unwrap();
        let paper = crate::registry::PaperEntry {
            id: "paper-1".to_string(),
            title: "Test Paper".to_string(),
            authors: Some("A, B".to_string()),
            venue: None,
            year: None,
            pdf_path: None,
            bibtex: None,
            tags: vec!["ai".to_string()],
            added_at: chrono::Utc::now(),
        };
        save_paper(&conn, &paper).unwrap();
        let papers = list_papers(&conn).unwrap();
        assert_eq!(papers.len(), 1);
        assert_eq!(papers[0].id, "paper-1");
    }

    #[test]
    fn test_find_papers_by_venue() {
        let conn = WorkspaceRegistry::init_in_memory().unwrap();
        let paper1 = crate::registry::PaperEntry {
            id: "p1".to_string(),
            title: "Paper One".to_string(),
            authors: None,
            venue: Some("NeurIPS".to_string()),
            year: Some(2024),
            pdf_path: None,
            bibtex: None,
            tags: vec![],
            added_at: chrono::Utc::now(),
        };
        let paper2 = crate::registry::PaperEntry {
            id: "p2".to_string(),
            title: "Paper Two".to_string(),
            authors: None,
            venue: Some("ICML".to_string()),
            year: Some(2023),
            pdf_path: None,
            bibtex: None,
            tags: vec![],
            added_at: chrono::Utc::now(),
        };
        save_paper(&conn, &paper1).unwrap();
        save_paper(&conn, &paper2).unwrap();

        let neurips = find_papers_by_venue(&conn, "NeurIPS").unwrap();
        assert_eq!(neurips.len(), 1);
        assert_eq!(neurips[0].id, "p1");

        let icml = find_papers_by_venue(&conn, "ICML").unwrap();
        assert_eq!(icml.len(), 1);
        assert_eq!(icml[0].id, "p2");

        let empty = find_papers_by_venue(&conn, "Unknown").unwrap();
        assert!(empty.is_empty());
    }

    #[test]
    fn test_save_embeddings() {
        let mut conn = WorkspaceRegistry::init_in_memory().unwrap();
        WorkspaceRegistry::seed_test_repo(&mut conn, "repo-a").unwrap();
        let embeddings = vec![
            ("func_a".to_string(), vec![1.0_f32, 0.0, 0.0]),
            ("func_b".to_string(), vec![0.0_f32, 1.0, 0.0]),
        ];
        let count = save_embeddings(&mut conn, "repo-a", &embeddings).unwrap();
        assert_eq!(count, 2);

        let mut stmt = conn
            .prepare("SELECT symbol_name, embedding FROM code_embeddings WHERE repo_id = ?1 ORDER BY symbol_name")
            .unwrap();
        let rows = stmt
            .query_map(["repo-a"], |row| Ok((row.get::<_, String>(0)?, row.get::<_, Vec<u8>>(1)?)))
            .unwrap();
        let results: Vec<_> = rows.collect::<Result<Vec<_>, _>>().unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].0, "func_a");
        let emb = crate::embedding::bytes_to_embedding(&results[0].1);
        assert_eq!(emb, vec![1.0, 0.0, 0.0]);
    }

    #[test]
    fn test_find_related_symbols() {
        let conn = WorkspaceRegistry::init_in_memory().unwrap();
        conn.execute(
            "INSERT INTO code_symbol_links (source_repo, source_symbol, target_repo, target_symbol, link_type, strength, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params!["repo-a", "main", "repo-b", "helper", "calls", 0.95_f64, chrono::Utc::now().to_rfc3339()],
        )
        .unwrap();

        let related = find_related_symbols(&conn, "repo-a", "main", 10).unwrap();
        assert_eq!(related.len(), 1);
        assert_eq!(related[0].0, "repo-a");
        assert_eq!(related[0].1, "main");
        assert_eq!(related[0].2, "repo-b");
        assert_eq!(related[0].3, "helper");
        assert_eq!(related[0].4, "calls");
        assert!((related[0].5 - 0.95).abs() < f32::EPSILON);
    }

    #[test]
    fn test_semantic_search_symbols() {
        let mut conn = WorkspaceRegistry::init_in_memory().unwrap();
        WorkspaceRegistry::seed_test_repo(&mut conn, "repo-a").unwrap();

        conn.execute(
            "INSERT INTO code_symbols (repo_id, file_path, symbol_type, name, line_start, line_end, signature) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params!["repo-a", "src/lib.rs", "function", "hello", 1_i64, 3_i64, Option::<&str>::None],
        )
        .unwrap();

        conn.execute(
            "INSERT INTO code_symbols (repo_id, file_path, symbol_type, name, line_start, line_end, signature) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params!["repo-a", "", "function", "hello", 1_i64, 3_i64, Option::<&str>::None],
        )
        .unwrap();

        let emb = crate::embedding::embedding_to_bytes(&[1.0_f32, 0.0, 0.0]);
        conn.execute(
            "INSERT INTO code_embeddings (repo_id, file_path, symbol_name, embedding, generated_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params!["repo-a", "", "hello", emb, chrono::Utc::now().to_rfc3339()],
        )
        .unwrap();

        let results = semantic_search_symbols(&conn, "repo-a", &[1.0_f32, 0.0, 0.0], 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].1, "hello");
        assert!((results[0].4 - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_cross_repo_search_symbols() {
        let mut conn = WorkspaceRegistry::init_in_memory().unwrap();
        WorkspaceRegistry::seed_test_repo(&mut conn, "repo-a").unwrap();
        WorkspaceRegistry::seed_test_repo(&mut conn, "repo-b").unwrap();

        // Tag repo-a with "rust"
        conn.execute(
            "INSERT INTO repo_tags (repo_id, tag) VALUES (?1, ?2)",
            rusqlite::params!["repo-a", "rust"],
        )
        .unwrap();

        // Insert symbols and embeddings for repo-a
        conn.execute(
            "INSERT INTO code_symbols (repo_id, file_path, symbol_type, name, line_start, line_end, signature) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params!["repo-a", "src/lib.rs", "function", "hello", 1_i64, 3_i64, Option::<&str>::None],
        )
        .unwrap();

        let emb = crate::embedding::embedding_to_bytes(&[1.0_f32, 0.0, 0.0]);
        conn.execute(
            "INSERT INTO code_embeddings (repo_id, file_path, symbol_name, embedding, generated_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params!["repo-a", "src/lib.rs", "hello", emb, chrono::Utc::now().to_rfc3339()],
        )
        .unwrap();

        // Search with matching tag should find repo-a
        let results = cross_repo_search_symbols(
            &conn,
            &["rust".to_string()],
            "hello",
            Some(&[1.0_f32, 0.0, 0.0]),
            10,
        )
        .unwrap();
        assert!(!results.is_empty());

        // Search with non-matching tag should return empty
        let empty = cross_repo_search_symbols(
            &conn,
            &["python".to_string()],
            "hello",
            Some(&[1.0_f32, 0.0, 0.0]),
            10,
        )
        .unwrap();
        assert!(empty.is_empty());

        // Empty tags should search all repos
        let all =
            cross_repo_search_symbols(&conn, &[], "hello", Some(&[1.0_f32, 0.0, 0.0]), 10).unwrap();
        assert!(!all.is_empty());
    }

    #[test]
    fn test_symbol_read_tracking() {
        let conn = WorkspaceRegistry::init_in_memory().unwrap();

        // Record some reads
        record_symbol_read(&conn, "repo-a", "func_a", Some("test")).unwrap();
        record_symbol_read(&conn, "repo-a", "func_a", Some("test")).unwrap();
        record_symbol_read(&conn, "repo-a", "func_b", Some("test")).unwrap();

        // Query counts
        let counts = get_symbol_read_counts(
            &conn,
            "repo-a",
            &["func_a".to_string(), "func_b".to_string(), "func_c".to_string()],
        )
        .unwrap();

        assert_eq!(counts.get("func_a"), Some(&2_i64));
        assert_eq!(counts.get("func_b"), Some(&1_i64));
        assert_eq!(counts.get("func_c"), None);

        // Empty names -> empty counts
        let empty = get_symbol_read_counts(&conn, "repo-a", &[]).unwrap();
        assert!(empty.is_empty());
    }
}
