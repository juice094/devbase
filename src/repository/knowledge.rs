//! Repository for knowledge base operations (summaries, modules, papers, experiments).

use crate::registry::{ExperimentEntry, PaperEntry, ENTITY_TYPE_PAPER};
use chrono::{DateTime, Utc};
use serde_json::Value;

pub struct KnowledgeRepository<'a>(&'a rusqlite::Connection);

impl<'a> KnowledgeRepository<'a> {
    pub fn new(conn: &'a rusqlite::Connection) -> Self {
        Self(conn)
    }

    /// Save or update a repo summary.
    pub fn save_summary(
        &self,
        repo_id: &str,
        summary: &str,
        keywords: &str,
    ) -> anyhow::Result<()> {
        self.0.execute(
            "INSERT OR REPLACE INTO repo_summaries (repo_id, summary, keywords, generated_at) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![repo_id, summary, keywords, Utc::now().to_rfc3339()],
        )?;
        Ok(())
    }

    /// Save module structure for a repo.
    pub fn save_modules(
        &self,
        repo_id: &str,
        modules: &[(String, String)],
    ) -> anyhow::Result<()> {
        let tx = self.0.unchecked_transaction()?;
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

    /// List modules for a repo.
    pub fn list_modules(&self, repo_id: &str) -> anyhow::Result<Vec<(String, String, String)>> {
        let mut stmt = self.0.prepare(
            "SELECT module_name, module_type, module_path FROM repo_modules WHERE repo_id = ?1",
        )?;
        let rows = stmt.query_map([repo_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?))
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    /// List papers with optional filter.
    pub fn list_papers(&self, filter: Option<&str>) -> anyhow::Result<Value> {
        let mut sql = String::from(
            "SELECT e.id, e.name, json_extract(e.metadata, '$.authors'),
                    json_extract(e.metadata, '$.venue'), json_extract(e.metadata, '$.year'),
                    e.local_path, json_extract(e.metadata, '$.bibtex'),
                    json_extract(e.metadata, '$.tags'), json_extract(e.metadata, '$.added_at')
             FROM entities e
             WHERE e.entity_type = ?1",
        );
        let mut params: Vec<Box<dyn rusqlite::ToSql>> =
            vec![Box::new(ENTITY_TYPE_PAPER.to_string())];
        if let Some(f) = filter {
            sql.push_str(
                " AND (e.name LIKE ?2 OR json_extract(e.metadata, '$.authors') LIKE ?2)",
            );
            params.push(Box::new(format!("%{}%", f)));
        }
        sql.push_str(" ORDER BY json_extract(e.metadata, '$.added_at') DESC");
        let mut stmt = self.0.prepare(&sql)?;
        let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        let rows = stmt.query_map(param_refs.as_slice(), |row| {
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
        let papers: Vec<PaperEntry> = rows.collect::<Result<Vec<_>, _>>()?;
        Ok(serde_json::to_value(papers)?)
    }

    /// Save a paper entry.
    pub fn save_paper(&self, paper: &Value) -> anyhow::Result<Value> {
        let paper: PaperEntry = serde_json::from_value(paper.clone())?;
        let metadata = serde_json::json!({
            "authors": paper.authors,
            "venue": paper.venue,
            "year": paper.year,
            "bibtex": paper.bibtex,
            "tags": paper.tags,
            "added_at": paper.added_at.to_rfc3339(),
        });
        crate::registry::upsert_entity(
            self.0,
            &paper.id,
            ENTITY_TYPE_PAPER,
            &paper.title,
            paper.pdf_path.as_deref(),
            &metadata,
        )?;
        Ok(serde_json::to_value(paper)?)
    }

    /// Save an experiment log.
    pub fn save_experiment(&self, exp: &Value) -> anyhow::Result<Value> {
        let exp: ExperimentEntry = serde_json::from_value(exp.clone())?;
        self.0.execute(
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
        Ok(serde_json::to_value(exp)?)
    }

    /// Generate a knowledge report for a repo.
    pub fn generate_report(&self, _repo_id: &str) -> anyhow::Result<Value> {
        // TODO: migrate from mcp/tools/repo.rs DevkitKnowledgeReportTool
        todo!()
    }
}

impl<'a> super::Repository for KnowledgeRepository<'a> {
    fn conn(&self) -> &rusqlite::Connection {
        self.0
    }
}
