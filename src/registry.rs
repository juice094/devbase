// SPDX-License-Identifier: MIT
// Copyright (c) 2026 juice094
//! Registry layer: SQLite-backed entity storage and domain-specific submodules.
//!
//! Central types (`RepoEntry`, `VaultNote`, `PaperEntry`, etc.) and the
//! [`RegistryClient`] trait implementation on [`AppContext`].
//! Submodules cover repos, health, knowledge, code metrics, call graphs,
//! dead-code analysis, and migrations.

use crate::clients::RegistryClient;
use crate::storage::AppContext;
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

/// Tags that mark a repository as "managed" for sync purposes.
/// Stored in the `repo_tags` table (not metadata) because tags are the
/// queryable, filterable dimension — metadata is for opaque JSON.
pub const MANAGED_TAGS: &[&str] = &[
    "mirror",
    "reference",
    "third-party",
    "collaborative",
    "team",
    "own-project",
    "tool",
    "active",
    "managed",
];

impl RepoEntry {
    /// Return the 'origin' remote if present, otherwise the first remote.
    pub fn primary_remote(&self) -> Option<&RemoteEntry> {
        self.remotes
            .iter()
            .find(|r| r.remote_name == "origin")
            .or_else(|| self.remotes.first())
    }

    /// Whether this repo is considered "managed" for sync/health automation.
    /// Managed status is determined by the presence of any tag in [`MANAGED_TAGS`].
    pub fn is_managed(&self) -> bool {
        self.tags.iter().any(|t| MANAGED_TAGS.contains(&t.as_str()))
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
    pub block_refs: Vec<String>,
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

pub use devbase_registry::health::HealthEntry;
pub use devbase_registry::metrics::CodeMetrics;
pub use devbase_registry::workspace::{OplogEntry, OplogEventType, WorkspaceSnapshot};

pub mod entity;
pub mod relation;

// Backward-compatible re-exports (migrated to entity.rs in v0.15).
pub use entity::{
    ENTITY_TYPE_PAPER, ENTITY_TYPE_REPO, ENTITY_TYPE_SKILL, ENTITY_TYPE_VAULT_NOTE,
    ENTITY_TYPE_WORKFLOW, upsert_entity,
};

pub mod agent_context;
pub mod call_graph;
pub mod code_symbols;
pub mod dead_code;
pub mod health;
pub mod import_ontology;
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

impl RegistryClient for AppContext {
    fn list_repos(&self, _filter: Option<&str>) -> anyhow::Result<serde_json::Value> {
        let conn = self.conn()?;
        let repos = repo::list_repos(&conn)?;
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
        let repos = repo::list_repos(&conn)?;
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
        let modules = knowledge::list_modules(&conn, repo_id)?;
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
        let paper_entry: PaperEntry = serde_json::from_value(paper.clone())?;
        knowledge::save_paper(&conn, &paper_entry)?;
        Ok(serde_json::json!({ "success": true }))
    }

    fn save_experiment(&self, exp: &serde_json::Value) -> anyhow::Result<serde_json::Value> {
        let conn = self.conn()?;
        let exp_entry: ExperimentEntry = serde_json::from_value(exp.clone())?;
        WorkspaceRegistry::save_experiment(&conn, &exp_entry)?;
        Ok(serde_json::json!({ "success": true }))
    }

    fn list_code_metrics(&self) -> anyhow::Result<serde_json::Value> {
        let conn = self.conn()?;
        let metrics = metrics::list_code_metrics(&conn)?;
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
        match metrics::get_code_metrics(&conn, repo_id)? {
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
        match health::get_health(&conn, repo_id)? {
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
        let edges = call_graph::query_call_edges(
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
        let symbols =
            code_symbols::query_code_symbols(&conn, repo_id, name, symbol_type, file, limit)?;
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
        let dead = dead_code::query_dead_code(&conn, repo_id, include_pub, limit)?;
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
        relation::save_relation(&conn, from, to, relation_type, confidence)?;
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
                let rows = relation::find_related_entities(&conn, entity_id, relation_type)?;
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
                let rows = relation::list_relations(&conn, entity_id, relation_type)?;
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
        let notes = vault::list_vault_notes(&conn)?;
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

use crate::clients::MemoryClient;

impl MemoryClient for AppContext {
    fn link_memories(
        &self,
        from_id: i64,
        to_id: i64,
        relation_type: &str,
        confidence: f64,
        evidence: Option<&str>,
    ) -> anyhow::Result<serde_json::Value> {
        let conn = self.conn()?;
        let id = agent_context::link_memories(
            &conn,
            from_id,
            to_id,
            relation_type,
            confidence,
            evidence,
        )?;
        Ok(serde_json::json!({
            "success": true,
            "relation_id": id,
            "from_memory_id": from_id,
            "to_memory_id": to_id,
            "relation_type": relation_type,
            "confidence": confidence
        }))
    }

    fn query_memory_relations(
        &self,
        memory_id: i64,
        direction: &str,
        relation_type: Option<&str>,
        limit: usize,
    ) -> anyhow::Result<serde_json::Value> {
        let conn = self.conn()?;
        let rels = agent_context::query_memory_relations(
            &conn,
            memory_id,
            direction,
            relation_type,
            limit,
        )?;
        let results: Vec<serde_json::Value> = rels
            .into_iter()
            .map(|r| {
                serde_json::json!({
                    "id": r.id,
                    "from_memory_id": r.from_memory_id,
                    "to_memory_id": r.to_memory_id,
                    "relation_type": r.relation_type,
                    "confidence": r.confidence,
                    "evidence": r.evidence,
                    "created_at": r.created_at.to_rfc3339(),
                })
            })
            .collect();
        Ok(serde_json::json!({ "success": true, "count": results.len(), "relations": results }))
    }

    fn build_memory_graph(
        &self,
        root_memory_id: i64,
        depth: u32,
    ) -> anyhow::Result<serde_json::Value> {
        let conn = self.conn()?;
        let graph = agent_context::build_memory_graph(&conn, root_memory_id, depth)?;
        let nodes: Vec<serde_json::Value> = graph
            .nodes
            .iter()
            .map(|n| {
                serde_json::json!({
                    "memory_id": n.memory_id,
                    "depth": n.depth,
                    "memory_type": n.memory_type,
                    "content_preview": n.content_preview,
                    "importance": n.importance,
                })
            })
            .collect();
        let edges: Vec<serde_json::Value> = graph
            .edges
            .iter()
            .map(|e| {
                serde_json::json!({
                    "relation_id": e.relation_id,
                    "from_id": e.from_id,
                    "to_id": e.to_id,
                    "relation_type": e.relation_type,
                    "confidence": e.confidence,
                })
            })
            .collect();
        Ok(serde_json::json!({
            "success": true,
            "root_id": graph.root_id,
            "node_count": nodes.len(),
            "edge_count": edges.len(),
            "nodes": nodes,
            "edges": edges
        }))
    }

    fn dedup_memories(
        &self,
        context_id: &str,
        threshold: f32,
    ) -> anyhow::Result<serde_json::Value> {
        let conn = self.conn()?;
        let memories = agent_context::list_memories(&conn, context_id)?;
        let mut duplicates: Vec<serde_json::Value> = Vec::new();
        let mut seen: Vec<&agent_context::AgentMemory> = Vec::new();

        for mem in &memories {
            let emb = match &mem.embedding {
                Some(e) => e.clone(),
                None => {
                    seen.push(mem);
                    continue;
                }
            };
            let emb_f32 = crate::embedding::bytes_to_embedding(&emb);
            for prev in &seen {
                if let Some(prev_emb) = &prev.embedding {
                    let prev_f32 = crate::embedding::bytes_to_embedding(prev_emb);
                    let sim = crate::embedding::cosine_similarity(&emb_f32, &prev_f32);
                    if sim >= threshold {
                        duplicates.push(serde_json::json!({
                            "memory_a": { "id": prev.id, "type": prev.memory_type, "content_preview": &prev.content[..prev.content.len().min(100)] },
                            "memory_b": { "id": mem.id, "type": mem.memory_type, "content_preview": &mem.content[..mem.content.len().min(100)] },
                            "similarity": sim
                        }));
                    }
                }
            }
            seen.push(mem);
        }

        Ok(serde_json::json!({
            "success": true,
            "context_id": context_id,
            "threshold": threshold,
            "duplicate_count": duplicates.len(),
            "duplicates": duplicates
        }))
    }

    fn merge_memories(
        &self,
        primary_id: i64,
        secondary_id: i64,
        strategy: &str,
    ) -> anyhow::Result<serde_json::Value> {
        let conn = self.conn()?;
        let primary = agent_context::get_memory_by_id(&conn, primary_id)?
            .ok_or_else(|| anyhow::anyhow!("primary memory {} not found", primary_id))?;
        let secondary = agent_context::get_memory_by_id(&conn, secondary_id)?
            .ok_or_else(|| anyhow::anyhow!("secondary memory {} not found", secondary_id))?;

        match strategy {
            "supersede" => {
                agent_context::link_memories(
                    &conn,
                    primary_id,
                    secondary_id,
                    "SUPERSEDES",
                    1.0,
                    Some("auto-merge"),
                )?;
                agent_context::archive_memory(&conn, secondary_id)?;
            }
            "merge_content" => {
                let merged = format!(
                    "{}\\n\\n---\\n[Merged from memory #{}]\\n{}",
                    primary.content, secondary_id, secondary.content
                );
                let token_count = (merged.len() as f64 / 2.5).ceil() as i64;
                conn.execute(
                    "UPDATE agent_memories SET content = ?1, token_count = ?2 WHERE id = ?3",
                    rusqlite::params![merged, token_count, primary_id],
                )?;
                agent_context::link_memories(
                    &conn,
                    primary_id,
                    secondary_id,
                    "SUPERSEDES",
                    1.0,
                    Some("merged content"),
                )?;
                agent_context::archive_memory(&conn, secondary_id)?;
            }
            _ => {
                // "keep_both": just link
                agent_context::link_memories(
                    &conn,
                    primary_id,
                    secondary_id,
                    "RELATES_TO",
                    1.0,
                    Some("manual link (keep_both)"),
                )?;
            }
        }

        Ok(serde_json::json!({
            "success": true,
            "primary_id": primary_id,
            "secondary_id": secondary_id,
            "strategy": strategy
        }))
    }

    fn apply_memory_decay(&self, context_id: &str) -> anyhow::Result<serde_json::Value> {
        let conn = self.conn()?;
        let archived = agent_context::apply_memory_decay(&conn, context_id)?;
        Ok(serde_json::json!({
            "success": true,
            "context_id": context_id,
            "archived_count": archived.len(),
            "archived_ids": archived
        }))
    }

    fn memory_stats(&self, context_id: &str) -> anyhow::Result<serde_json::Value> {
        let conn = self.conn()?;
        let stats = agent_context::memory_stats(&conn, context_id)?;
        Ok(serde_json::json!({
            "success": true,
            "context_id": context_id,
            "total_count": stats.total_count,
            "archived_count": stats.archived_count,
            "total_tokens_estimate": stats.total_tokens_estimate,
            "avg_quality": stats.avg_quality,
            "avg_importance": stats.avg_importance,
        }))
    }
}

#[cfg(test)]
pub mod test_helpers;

#[cfg(test)]
mod tests;
