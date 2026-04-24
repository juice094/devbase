use super::*;
use chrono::{DateTime, Utc};

impl WorkspaceRegistry {
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
        for (module_path, public_apis) in modules {
            tx.execute(
                "INSERT OR REPLACE INTO repo_modules_legacy (repo_id, module_path, public_apis, extracted_at) VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![repo_id, module_path, public_apis, Utc::now().to_rfc3339()],
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

    pub fn save_relation(
        conn: &rusqlite::Connection,
        from: &str,
        to: &str,
        rel_type: &str,
        confidence: f64,
    ) -> anyhow::Result<()> {
        conn.execute(
            "INSERT OR REPLACE INTO repo_relations (from_repo_id, to_repo_id, relation_type, confidence, discovered_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![from, to, rel_type, confidence, Utc::now().to_rfc3339()],
        )?;
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
        let tags = if paper.tags.is_empty() {
            None
        } else {
            Some(paper.tags.join(","))
        };
        conn.execute(
            "INSERT OR REPLACE INTO papers (id, title, authors, venue, year, pdf_path, bibtex, tags, added_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            rusqlite::params![
                &paper.id,
                &paper.title,
                paper.authors.as_ref(),
                paper.venue.as_ref(),
                paper.year,
                paper.pdf_path.as_ref(),
                paper.bibtex.as_ref(),
                tags,
                paper.added_at.to_rfc3339()
            ],
        )?;
        Ok(())
    }

    pub fn list_papers(conn: &rusqlite::Connection) -> anyhow::Result<Vec<PaperEntry>> {
        let mut stmt = conn.prepare(
            "SELECT id, title, authors, venue, year, pdf_path, bibtex, tags, added_at FROM papers ORDER BY added_at DESC"
        )?;
        let rows = stmt.query_map([], |row| {
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
                        s.split(',')
                            .map(|t| t.trim().to_string())
                            .filter(|t| !t.is_empty())
                            .collect()
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
            "SELECT id, title, authors, venue, year, pdf_path, bibtex, tags, added_at FROM papers WHERE venue = ?1 ORDER BY year DESC"
        )?;
        let rows = stmt.query_map([venue], |row| {
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
                        s.split(',')
                            .map(|t| t.trim().to_string())
                            .filter(|t| !t.is_empty())
                            .collect()
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
    pub fn save_experiment(
        conn: &rusqlite::Connection,
        exp: &ExperimentEntry,
    ) -> anyhow::Result<()> {
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
                "INSERT INTO code_embeddings (repo_id, symbol_name, embedding, generated_at)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(repo_id, symbol_name) DO UPDATE SET
                 embedding = excluded.embedding,
                 generated_at = excluded.generated_at",
                rusqlite::params![repo_id, symbol_name, blob, &now],
            )?;
            inserted += 1;
        }
        tx.commit()?;
        Ok(inserted)
    }

    /// Hybrid search: vector similarity + keyword matching with RRF merge.
    /// Falls back to pure keyword search when no embeddings are available.
    pub fn hybrid_search_symbols(
        conn: &rusqlite::Connection,
        repo_id: &str,
        query_text: &str,
        query_embedding: Option<&[f32]>,
        limit: usize,
    ) -> anyhow::Result<Vec<crate::semantic_index::SemanticSearchRow>> {
        crate::search::hybrid::hybrid_search_symbols(conn, repo_id, query_text, query_embedding, limit)
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
            "SELECT ce.symbol_name, cs.file_path, cs.line_start, ce.embedding
             FROM code_embeddings ce
             JOIN code_symbols cs ON ce.repo_id = cs.repo_id
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
}
