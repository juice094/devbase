// SPDX-License-Identifier: MIT
// Copyright (c) 2026 juice094
//! Registry layer: SQLite-backed entity storage and domain-specific submodules.
//!
//! Central types (`RepoEntry`, `VaultNote`, `PaperEntry`, etc.) and the
//! [`RegistryClient`] trait implementation on [`AppContext`].
//! Submodules cover repos, health, knowledge, code metrics, call graphs,
//! dead-code analysis, and migrations.

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
        let symbols = crate::registry::code_symbols::query_code_symbols(
            &conn,
            repo_id,
            name,
            symbol_type,
            file,
            limit,
        )?;
        let out: Vec<serde_json::Value> = symbols
            .iter()
            .map(|s| {
                serde_json::json!({
                    "file_path": s.file_path,
                    "symbol_type": s.symbol_type,
                    "name": s.name,
                    "line_start": s.line_start,
                    "line_end": s.line_end,
                    "signature": s.signature,
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
        let dead = crate::registry::dead_code::query_dead_code(&conn, repo_id, include_pub, limit)?;
        let out: Vec<serde_json::Value> = dead
            .iter()
            .map(|d| {
                serde_json::json!({
                    "file_path": d.file_path,
                    "name": d.name,
                    "line_start": d.line_start,
                    "signature": d.signature,
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

    fn save_relation(
        &self,
        from: &str,
        to: &str,
        relation_type: &str,
        confidence: f64,
    ) -> anyhow::Result<serde_json::Value> {
        let conn = self.conn()?;
        crate::registry::relation::save_relation(&conn, from, to, relation_type, confidence)?;
        Ok(serde_json::json!({ "success": true }))
    }

    fn query_relations(
        &self,
        entity_id: &str,
        direction: &str,
        relation_type: Option<&str>,
    ) -> anyhow::Result<serde_json::Value> {
        let conn = self.conn()?;
        let results = match direction {
            "bidirectional" => {
                let rows = crate::registry::relation::find_related_entities(
                    &conn,
                    entity_id,
                    relation_type,
                )?;
                rows.into_iter()
                    .map(|(from, to, rt, conf, created)| {
                        serde_json::json!({
                            "from_entity_id": from,
                            "to_entity_id": to,
                            "relation_type": rt,
                            "confidence": conf,
                            "created_at": created
                        })
                    })
                    .collect::<Vec<_>>()
            }
            "incoming" => {
                let mut stmt = conn.prepare(
                    "SELECT from_entity_id, relation_type, confidence, created_at FROM relations
                     WHERE to_entity_id = ?1
                     ORDER BY confidence DESC",
                )?;
                let rows = stmt.query_map([entity_id], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, f64>(2)?,
                        row.get::<_, String>(3)?,
                    ))
                })?;
                let filtered: Vec<_> = if let Some(rt) = relation_type.filter(|s| !s.is_empty()) {
                    rows.filter(|r| r.as_ref().map(|(_, t, _, _)| t == rt).unwrap_or(false))
                        .collect::<Result<Vec<_>, _>>()?
                } else {
                    rows.collect::<Result<Vec<_>, _>>()?
                };
                filtered
                    .into_iter()
                    .map(|(from, rt, conf, created)| {
                        serde_json::json!({
                            "from_entity_id": from,
                            "relation_type": rt,
                            "confidence": conf,
                            "created_at": created
                        })
                    })
                    .collect::<Vec<_>>()
            }
            _ => {
                let rows =
                    crate::registry::relation::list_relations(&conn, entity_id, relation_type)?;
                rows.into_iter()
                    .map(|(to, rt, conf, created)| {
                        serde_json::json!({
                            "to_entity_id": to,
                            "relation_type": rt,
                            "confidence": conf,
                            "created_at": created
                        })
                    })
                    .collect::<Vec<_>>()
            }
        };
        Ok(serde_json::json!({ "success": true, "relations": results }))
    }

    fn delete_relations(
        &self,
        from: &str,
        to: &str,
        relation_type: Option<&str>,
    ) -> anyhow::Result<serde_json::Value> {
        let conn = self.conn()?;
        let count = match relation_type.filter(|s| !s.is_empty()) {
            Some(rt) => conn.execute(
                "DELETE FROM relations WHERE from_entity_id = ?1 AND to_entity_id = ?2 AND relation_type = ?3",
                rusqlite::params![from, to, rt],
            )?,
            None => conn.execute(
                "DELETE FROM relations WHERE from_entity_id = ?1 AND to_entity_id = ?2",
                rusqlite::params![from, to],
            )?,
        };
        Ok(serde_json::json!({ "success": true, "deleted": count }))
    }

    fn list_vault_notes(&self) -> anyhow::Result<serde_json::Value> {
        let conn = self.conn()?;
        let notes = crate::registry::vault::list_vault_notes(&conn)?;
        let results: Vec<serde_json::Value> = notes
            .into_iter()
            .map(|n| {
                serde_json::json!({
                    "id": n.id,
                    "path": n.path,
                    "title": n.title,
                    "tags": n.tags,
                })
            })
            .collect();
        Ok(serde_json::json!({ "success": true, "count": results.len(), "notes": results }))
    }
}

#[cfg(test)]
mod test_helpers;

#[cfg(test)]
mod tests;
