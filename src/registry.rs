// SPDX-License-Identifier: MIT
// Copyright (c) 2026 juice094
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteEntry {
    pub remote_name: String,
    pub upstream_url: Option<String>,
    pub default_branch: Option<String>,
    pub last_sync: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoEntry {
    pub id: String,
    pub local_path: PathBuf,
    pub tags: Vec<String>,
    pub discovered_at: DateTime<Utc>,
    pub language: Option<String>,
    pub workspace_type: String,
    pub data_tier: String,
    pub last_synced_at: Option<DateTime<Utc>>,
    pub stars: Option<u64>,
    pub remotes: Vec<RemoteEntry>,
}

impl RepoEntry {
    /// Return the 'origin' remote if present, otherwise the first remote.
    pub fn primary_remote(&self) -> Option<&RemoteEntry> {
        self.remotes
            .iter()
            .find(|r| r.remote_name == "origin")
            .or_else(|| self.remotes.first())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultNote {
    pub id: String,
    pub path: String,
    pub title: Option<String>,
    pub content: String,
    pub frontmatter: Option<String>,
    pub tags: Vec<String>,
    pub outgoing_links: Vec<String>,
    pub linked_repo: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaperEntry {
    pub id: String,
    pub title: String,
    pub authors: Option<String>,
    pub venue: Option<String>,
    pub year: Option<i32>,
    pub pdf_path: Option<String>,
    pub bibtex: Option<String>,
    pub tags: Vec<String>,
    pub added_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExperimentEntry {
    pub id: String,
    pub repo_id: Option<String>,
    pub paper_id: Option<String>,
    pub config_json: Option<String>,
    pub result_path: Option<String>,
    pub git_commit: Option<String>,
    pub syncthing_folder_id: Option<String>,
    pub status: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceRegistry {
    pub version: String,
    pub entries: Vec<RepoEntry>,
}

impl Default for WorkspaceRegistry {
    fn default() -> Self {
        Self {
            version: "0.1.0".to_string(),
            entries: Vec::new(),
        }
    }
}

pub use devbase_registry_health::HealthEntry;
pub use devbase_registry_metrics::CodeMetrics;
pub use devbase_registry_workspace::{OplogEntry, OplogEventType, WorkspaceSnapshot};

pub mod entity;
pub mod relation;

// Backward-compatible re-exports (migrated to entity.rs in v0.15).
pub use entity::{
    ENTITY_TYPE_PAPER, ENTITY_TYPE_REPO, ENTITY_TYPE_SKILL, ENTITY_TYPE_VAULT_NOTE,
    ENTITY_TYPE_WORKFLOW, upsert_entity,
};

pub mod call_graph;
pub mod code_symbols;
pub mod dead_code;
pub mod health;
pub mod knowledge;
pub mod knowledge_meta;
pub mod known_limits;
pub mod links;
pub mod metrics;
mod migrate;
pub mod migrations;
pub mod repo;
pub mod repos_toml;
pub mod vault;
pub mod workspace;

impl crate::clients::RegistryClient for crate::storage::AppContext {
    fn list_repos(&self, _filter: Option<&str>) -> anyhow::Result<serde_json::Value> {
        let conn = self.conn()?;
        let repos = crate::registry::repo::list_repos(&conn)?;
        let results: Vec<serde_json::Value> = repos
            .into_iter()
            .map(|r| {
                serde_json::json!({
                    "id": r.id,
                    "local_path": r.local_path,
                    "language": r.language,
                    "tags": r.tags,
                    "workspace_type": r.workspace_type,
                    "data_tier": r.data_tier,
                })
            })
            .collect();
        Ok(serde_json::json!({ "success": true, "count": results.len(), "repos": results }))
    }

    fn get_repo(&self, repo_id: &str) -> anyhow::Result<serde_json::Value> {
        let conn = self.conn()?;
        let repos = crate::registry::repo::list_repos(&conn)?;
        match repos.into_iter().find(|r| r.id == repo_id) {
            Some(r) => Ok(serde_json::json!({
                "success": true,
                "id": r.id,
                "local_path": r.local_path,
                "language": r.language,
                "tags": r.tags,
                "workspace_type": r.workspace_type,
                "data_tier": r.data_tier,
            })),
            None => Ok(serde_json::json!({ "success": false, "error": "repo not found" })),
        }
    }

    fn list_modules(&self, repo_id: &str) -> anyhow::Result<serde_json::Value> {
        let conn = self.conn()?;
        let modules = crate::registry::knowledge::list_modules(&conn, repo_id)?;
        let results: Vec<serde_json::Value> = modules
            .into_iter()
            .map(|(name, ty, path)| {
                serde_json::json!({
                    "name": name,
                    "type": ty,
                    "path": path,
                })
            })
            .collect();
        Ok(serde_json::json!({ "success": true, "count": results.len(), "modules": results }))
    }

    fn save_paper(&self, paper: &serde_json::Value) -> anyhow::Result<serde_json::Value> {
        let conn = self.conn()?;
        let paper_entry: crate::registry::PaperEntry = serde_json::from_value(paper.clone())?;
        crate::registry::knowledge::save_paper(&conn, &paper_entry)?;
        Ok(serde_json::json!({ "success": true }))
    }

    fn save_experiment(&self, exp: &serde_json::Value) -> anyhow::Result<serde_json::Value> {
        let conn = self.conn()?;
        let exp_entry: crate::registry::ExperimentEntry = serde_json::from_value(exp.clone())?;
        crate::registry::WorkspaceRegistry::save_experiment(&conn, &exp_entry)?;
        Ok(serde_json::json!({ "success": true }))
    }

    fn list_code_metrics(&self) -> anyhow::Result<serde_json::Value> {
        let conn = self.conn()?;
        let metrics = crate::registry::metrics::list_code_metrics(&conn)?;
        let repos: Vec<serde_json::Value> = metrics
            .into_iter()
            .map(|(id, m)| {
                serde_json::json!({
                    "repo_id": id,
                    "total_lines": m.total_lines,
                    "source_lines": m.source_lines,
                    "test_lines": m.test_lines,
                    "comment_lines": m.comment_lines,
                    "file_count": m.file_count,
                    "language_breakdown": m.language_breakdown,
                    "updated_at": m.updated_at.to_rfc3339()
                })
            })
            .collect();
        Ok(serde_json::json!({ "success": true, "count": repos.len(), "repos": repos }))
    }

    fn get_code_metrics(&self, repo_id: &str) -> anyhow::Result<serde_json::Value> {
        let conn = self.conn()?;
        match crate::registry::metrics::get_code_metrics(&conn, repo_id)? {
            Some(m) => Ok(serde_json::json!({
                "success": true,
                "repo_id": repo_id,
                "total_lines": m.total_lines,
                "source_lines": m.source_lines,
                "test_lines": m.test_lines,
                "comment_lines": m.comment_lines,
                "file_count": m.file_count,
                "language_breakdown": m.language_breakdown,
                "updated_at": m.updated_at.to_rfc3339()
            })),
            None => {
                Ok(serde_json::json!({ "success": false, "error": "No metrics found for repo" }))
            }
        }
    }

    fn get_health(&self, repo_id: &str) -> anyhow::Result<serde_json::Value> {
        let conn = self.conn()?;
        match crate::registry::health::get_health(&conn, repo_id)? {
            Some(h) => Ok(serde_json::json!({
                "success": true,
                "repo_id": repo_id,
                "status": h.status,
                "ahead": h.ahead,
                "behind": h.behind,
                "checked_at": h.checked_at.to_rfc3339()
            })),
            None => Ok(serde_json::json!({ "success": false, "error": "No health data found" })),
        }
    }

    fn query_call_graph(
        &self,
        repo_id: &str,
        callee: Option<&str>,
        caller: Option<&str>,
        file: Option<&str>,
        limit: usize,
    ) -> anyhow::Result<serde_json::Value> {
        let conn = self.conn()?;
        let edges = crate::registry::call_graph::query_call_edges(
            &conn,
            repo_id,
            callee.filter(|s| !s.is_empty()),
            caller.filter(|s| !s.is_empty()),
            file.filter(|s| !s.is_empty()),
            limit,
        )?;
        let calls: Vec<serde_json::Value> = edges
            .into_iter()
            .map(|e| {
                serde_json::json!({
                    "caller_file": e.caller_file,
                    "caller_symbol": e.caller_symbol,
                    "caller_line": e.caller_line,
                    "callee_name": e.callee_name,
                })
            })
            .collect();
        Ok(serde_json::json!({
            "success": true,
            "repo_id": repo_id,
            "count": calls.len(),
            "calls": calls
        }))
    }

    fn query_dependencies(
        &self,
        repo_id: &str,
        direction: &str,
        relation_type: Option<&str>,
    ) -> anyhow::Result<serde_json::Value> {
        let conn = self.conn()?;
        let rel_filter = relation_type.filter(|s| !s.is_empty());
        let label = if direction == "incoming" || direction == "reverse" {
            "reverse dependencies"
        } else {
            "dependencies"
        };
        let rows = if direction == "incoming" || direction == "reverse" {
            crate::dependency_graph::list_reverse_dependencies(&conn, repo_id)?
        } else {
            crate::dependency_graph::list_dependencies(&conn, repo_id)?
        };
        let deps: Vec<serde_json::Value> = rows
            .into_iter()
            .filter(|(_, rel, _)| rel_filter.is_none_or(|f| f == rel))
            .map(|(id, rel, conf)| {
                serde_json::json!({
                    "repo_id": id,
                    "relation_type": rel,
                    "confidence": conf,
                })
            })
            .collect();
        Ok(serde_json::json!({
            "success": true,
            "repo_id": repo_id,
            "direction": direction,
            "label": label,
            "count": deps.len(),
            "dependencies": deps
        }))
    }

    fn query_code_symbols(
        &self,
        repo_id: &str,
        name: Option<&str>,
        symbol_type: Option<&str>,
        file: Option<&str>,
        limit: usize,
    ) -> anyhow::Result<serde_json::Value> {
        let conn = self.conn()?;
        let mut sql = String::from(
            "SELECT file_path, symbol_type, name, line_start, line_end, signature \
             FROM code_symbols WHERE repo_id = ?1",
        );
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(repo_id.to_string())];
        if let Some(ty) = symbol_type.filter(|s| !s.is_empty()) {
            sql.push_str(" AND symbol_type = ?");
            sql.push_str(&(params.len() + 1).to_string());
            params.push(Box::new(ty.to_string()));
        }
        if let Some(n) = name.filter(|s| !s.is_empty()) {
            sql.push_str(" AND name LIKE ?");
            sql.push_str(&(params.len() + 1).to_string());
            params.push(Box::new(format!("%{}%", n)));
        }
        if let Some(f) = file.filter(|s| !s.is_empty()) {
            sql.push_str(" AND file_path LIKE ?");
            sql.push_str(&(params.len() + 1).to_string());
            params.push(Box::new(format!("%{}%", f)));
        }
        sql.push_str(&format!(" ORDER BY file_path, line_start LIMIT {}", limit.min(200)));

        let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(param_refs), |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, Option<String>>(5)?,
            ))
        })?;

        let mut symbols = Vec::new();
        for row in rows {
            symbols.push(row?);
        }

        let out: Vec<serde_json::Value> = symbols
            .iter()
            .map(|(fp, st, n, ls, le, sig)| {
                serde_json::json!({
                    "file_path": fp,
                    "symbol_type": st,
                    "name": n,
                    "line_start": ls,
                    "line_end": le,
                    "signature": sig,
                })
            })
            .collect();
        Ok(serde_json::json!({
            "success": true,
            "repo_id": repo_id,
            "count": out.len(),
            "symbols": out
        }))
    }

    fn query_dead_code(
        &self,
        repo_id: &str,
        include_pub: bool,
        limit: usize,
    ) -> anyhow::Result<serde_json::Value> {
        let conn = self.conn()?;
        let mut sql = String::from(
            "SELECT file_path, name, line_start, signature \
             FROM code_symbols cs \
             WHERE cs.repo_id = ?1 AND cs.symbol_type = 'function' \
             AND NOT EXISTS ( \
                 SELECT 1 FROM code_call_graph ccg \
                 WHERE ccg.repo_id = cs.repo_id AND ccg.callee_name = cs.name \
             )",
        );
        if !include_pub {
            sql.push_str(" AND (cs.signature IS NULL OR cs.signature NOT LIKE 'pub%fn%')");
        }
        sql.push_str(" AND cs.name != 'main'");
        sql.push_str(" AND cs.name NOT LIKE 'test_%'");
        sql.push_str(
            " AND cs.file_path NOT LIKE '%/tests.rs' AND cs.file_path NOT LIKE '%\\tests.rs'",
        );
        sql.push_str(" AND (cs.attributes IS NULL OR cs.attributes NOT LIKE '%#[test]%')");
        sql.push_str(&format!(" ORDER BY cs.file_path, cs.line_start LIMIT {}", limit.min(200)));

        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map([repo_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, Option<String>>(3)?,
            ))
        })?;

        let mut dead = Vec::new();
        for row in rows {
            dead.push(row?);
        }

        let out: Vec<serde_json::Value> = dead
            .iter()
            .map(|(fp, n, line, sig)| {
                serde_json::json!({
                    "file_path": fp,
                    "name": n,
                    "line_start": line,
                    "signature": sig,
                })
            })
            .collect();
        Ok(serde_json::json!({
            "success": true,
            "repo_id": repo_id,
            "count": out.len(),
            "dead_functions": out
        }))
    }
}

#[cfg(test)]
mod test_helpers;

#[cfg(test)]
mod tests;
