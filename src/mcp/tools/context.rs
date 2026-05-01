use crate::mcp::McpTool;
use anyhow::Context;

#[derive(Clone)]
pub struct DevkitProjectContextTool;

impl McpTool for DevkitProjectContextTool {
    fn name(&self) -> &'static str {
        "devkit_project_context"
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "description": r#"Retrieve a unified context snapshot for a project by aggregating its repository metadata, linked vault notes, keyword-matched vault notes, and associated asset files. This is the single best starting point when you need to understand a project holistically.

Use this when the user wants to:
- Understand a project at a glance (repo info + docs + assets in one call)
- Prepare context before answering questions about a specific codebase
- Find all documentation and resources related to a project
- Build a project brief or summary without making multiple tool calls

Do NOT use this for:
- Searching across ALL repos (use devkit_query_repos instead)
- Full-text search in the vault without a specific project (use devkit_vault_search instead)
- Checking the health of multiple repos (use devkit_health instead)
- If you only need one specific piece of information (e.g., just stars count), use the specific tool instead to save context space

Parameters:
- project: Project identifier — can be a repo id, repo name, or vault note id/path. The tool matches by substring (case-insensitive).

Returns: JSON object with:
  - success: boolean
  - project: the matched project identifier
  - repo: repository metadata (id, path, language, tags, stars) or null if no repo matched
  - vault_notes: array of linked and keyword-matched notes (id, title, source: "link" or "search")
  - assets: array of files/folders from the project's assets directory
  - relations: array of knowledge-graph relations (from relations table) linking this entity to others
  - workflows: array of recent workflow executions for this repo"#,
            "inputSchema": {
                "type": "object",
                "properties": {
                    "project": {
                        "type": "string",
                        "description": "Project identifier (repo id, repo name, or vault note id/path)"
                    },
                    "goal": {
                        "type": "string",
                        "description": "Optional task description. When provided, symbols and calls are relevance-ranked against this goal instead of returned arbitrarily."
                    }
                },
                "required": ["project"]
            }
        })
    }

    async fn invoke(
        &self,
        args: serde_json::Value,
        ctx: &mut crate::storage::AppContext,
    ) -> anyhow::Result<serde_json::Value> {
        let project = args
            .get("project")
            .and_then(|v| v.as_str())
            .context("Missing required argument: project")?;
        let goal = args.get("goal").and_then(|v| v.as_str()).map(|s| s.to_string());

        let pool = ctx.pool();
        let result = tokio::task::spawn_blocking({
            let project = project.to_string();
            move || {
                let conn = pool.get()?;

                // 1. Find repo by exact id or path substring
                let repos = crate::registry::repo::list_repos(&conn)?;
                let matched_repo = repos.into_iter().find(|r| {
                    r.id.eq_ignore_ascii_case(&project)
                        || r.local_path
                            .to_string_lossy()
                            .to_lowercase()
                            .contains(&project.to_lowercase())
                });

                let repo_json = matched_repo.as_ref().map(|r| {
                    serde_json::json!({
                        "id": r.id,
                        "path": r.local_path,
                        "language": r.language,
                        "tags": r.tags,
                        "stars": r.stars,
                    })
                });

                let repo_id = matched_repo.as_ref().map(|r| r.id.clone());

                // 2. Linked vault notes (via vault_repo_links)
                let mut linked_vaults = Vec::new();
                if let Some(ref rid) = repo_id {
                    let notes =
                        crate::registry::links::get_linked_vault_notes(&conn, rid)?;
                    for (vid, vtitle) in notes {
                        linked_vaults.push(serde_json::json!({
                            "id": vid,
                            "title": vtitle,
                            "source": "link",
                        }));
                    }
                }

                // 3. Vault notes whose id/path contains the project name
                let all_notes = crate::registry::vault::list_vault_notes(&conn)?;
                for n in all_notes {
                    let hay = format!("{} {}", n.id, n.path).to_lowercase();
                    if hay.contains(&project.to_lowercase()) {
                        // Avoid duplicates
                        if !linked_vaults
                            .iter()
                            .any(|v| v.get("id").and_then(|x| x.as_str()) == Some(&n.id))
                        {
                            linked_vaults.push(serde_json::json!({
                                "id": n.id,
                                "title": n.title,
                                "source": "search",
                            }));
                        }
                    }
                }

                // 4. Code structure: modules
                let mut modules = Vec::new();
                if let Some(ref rid) = repo_id {
                    if let Ok(ms) = crate::registry::knowledge::list_modules(&conn, rid) {
                        for (name, kind, path) in ms {
                            modules.push(serde_json::json!({
                                "name": name,
                                "kind": kind,
                                // TODO(veto-audit-2026-04-26): RF-7 路径隐私 — path 可能为绝对路径，泄露用户目录结构。
                                // 修复: 将 dirs::home_dir() 前缀替换为 ~，或返回相对路径。
                                "path": path,
                            }));
                        }
                    }
                }

                // 5. Code symbols (top 50, relevance-ranked when goal is provided)
                let mut symbols = Vec::new();
                let mut symbol_names: std::collections::HashSet<String> = std::collections::HashSet::new();
                if let Some(ref rid) = repo_id {
                    if let Some(ref g) = goal {
                        match crate::registry::WorkspaceRegistry::hybrid_search_symbols(&conn, rid, g, None, 50) {
                            Ok(rows) => {
                                for (_srid, name, path, line, score) in rows {
                                    symbol_names.insert(name.clone());
                                    symbols.push(serde_json::json!({
                                        "name": name,
                                        "file": path,
                                        "line": line,
                                        "relevance_score": score,
                                    }));
                                }
                            }
                            Err(e) => {
                                tracing::warn!("hybrid_search_symbols failed for goal '{}': {}", g, e);
                            }
                        }
                    } else {
                        let mut stmt = conn.prepare(
                            "SELECT name, file_path, symbol_type, line_start, signature
                             FROM code_symbols WHERE repo_id = ?1 LIMIT 50"
                        )?;
                        let rows = stmt.query_map([rid], |row| {
                            Ok(serde_json::json!({
                                "name": row.get::<_, String>(0)?,
                                "file": row.get::<_, String>(1)?,
                                "type": row.get::<_, String>(2)?,
                                "line": row.get::<_, Option<i64>>(3)?,
                                "signature": row.get::<_, Option<String>>(4)?,
                            }))
                        })?;
                        for v in rows.flatten() {
                            if let Some(name) = v.get("name").and_then(|n| n.as_str()) {
                                symbol_names.insert(name.to_string());
                            }
                            symbols.push(v);
                        }
                    }
                }

                // 5b. Record symbol reads + apply behavioral boosting for non-goal path
                if let Some(ref rid) = repo_id {
                    if !symbols.is_empty() {
                        if let Ok(tx) = conn.unchecked_transaction() {
                            for sym in &symbols {
                                if let Some(name) = sym.get("name").and_then(|n| n.as_str()) {
                                    let _ = tx.execute(
                                        "INSERT INTO agent_symbol_reads (repo_id, symbol_name, read_at, context) VALUES (?1, ?2, datetime('now'), ?3)",
                                        rusqlite::params![rid, name, goal.as_deref().unwrap_or("project_context")],
                                    );
                                }
                            }
                            let _ = tx.commit();
                        }
                    }

                    // Behavioral boosting for non-goal path (goal path already boosted in hybrid_search)
                    if goal.is_none() && symbols.len() > 1 {
                        let names: Vec<String> = symbols.iter().filter_map(|s| s.get("name").and_then(|n| n.as_str()).map(|s| s.to_string())).collect();
                        if let Ok(counts) = crate::registry::WorkspaceRegistry::get_symbol_read_counts(&conn, rid, &names) {
                            let mut scored: Vec<(serde_json::Value, f32)> = symbols.into_iter().map(|s| {
                                let boost = s.get("name").and_then(|n| n.as_str())
                                    .and_then(|name| counts.get(name))
                                    .map(|cnt| (*cnt as f32 * 0.05).min(0.5))
                                    .unwrap_or(0.0);
                                (s, boost)
                            }).collect();
                            scored.sort_by(|a, b| {
                                b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal)
                            });
                            symbols = scored.into_iter().map(|(s, _)| s).collect();
                        }
                    }
                }

                // 6. Call graph edges (top 50, filtered to relevant symbols when goal is provided)
                let mut calls = Vec::new();
                if let Some(ref rid) = repo_id {
                    let mut stmt = conn.prepare(
                        "SELECT caller_file, caller_symbol, callee_name
                         FROM code_call_graph WHERE repo_id = ?1 LIMIT 200"
                    )?;
                    let rows = stmt.query_map([rid], |row| {
                        Ok(serde_json::json!({
                            "caller_file": row.get::<_, String>(0)?,
                            "caller": row.get::<_, String>(1)?,
                            "callee": row.get::<_, String>(2)?,
                        }))
                    })?;
                    for v in rows.flatten() {
                        if goal.is_some() {
                            let caller = v.get("caller").and_then(|s| s.as_str()).unwrap_or("");
                            let callee = v.get("callee").and_then(|s| s.as_str()).unwrap_or("");
                            if symbol_names.contains(caller) || symbol_names.contains(callee) {
                                calls.push(v);
                            }
                        } else {
                            calls.push(v);
                        }
                        if calls.len() >= 50 { break; }
                    }
                }

                // 7. Recent activity from oplog (last 10 events)
                let mut activity = Vec::new();
                if let Some(ref rid) = repo_id {
                    match crate::registry::workspace::list_oplog_by_repo(&conn, rid, 10) {
                        Ok(entries) => {
                            for entry in entries {
                                activity.push(serde_json::json!({
                                    "event_type": entry.event_type.as_str(),
                                    "timestamp": entry.timestamp.to_rfc3339(),
                                    "status": entry.status,
                                    "details": entry.details,
                                }));
                            }
                        }
                        Err(e) => {
                            tracing::warn!("list_oplog_by_repo failed for {}: {}", rid, e);
                        }
                    }
                }

                // 8. Symbol links (conceptual relationships for top symbols)
                let mut related_symbols = Vec::new();
                if let Some(ref rid) = repo_id {
                    let limit_per_symbol = if symbols.len() <= 5 { 4 } else { 2 };
                    for sym in &symbols {
                        if let Some(name) = sym.get("name").and_then(|n| n.as_str()) {
                            match crate::registry::WorkspaceRegistry::find_related_symbols(&conn, rid, name, limit_per_symbol) {
                                Ok(links) => {
                                    for (_src_repo, _src_sym, tgt_repo, tgt_sym, link_type, strength) in links {
                                        related_symbols.push(serde_json::json!({
                                            "from": name,
                                            "to": tgt_sym,
                                            "to_repo": tgt_repo,
                                            "link_type": link_type,
                                            "strength": strength,
                                        }));
                                    }
                                }
                                Err(e) => {
                                    tracing::warn!("find_related_symbols failed for {}: {}", name, e);
                                }
                            }
                        }
                        if related_symbols.len() >= 20 { break; }
                    }
                }
                // Deduplicate related_symbols by (from, to, link_type)
                let mut seen = std::collections::HashSet::new();
                related_symbols.retain(|item| {
                    let key = (
                        item.get("from").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                        item.get("to").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                        item.get("link_type").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    );
                    seen.insert(key)
                });

                // 9. Knowledge-graph relations (from relations table)
                let mut relations = Vec::new();
                if let Some(ref rid) = repo_id {
                    match crate::registry::relation::find_related_entities(&conn, rid, None) {
                        Ok(rows) => {
                            for (from, to, rt, conf, created) in rows {
                                relations.push(serde_json::json!({
                                    "from_entity_id": from,
                                    "to_entity_id": to,
                                    "relation_type": rt,
                                    "confidence": conf,
                                    "created_at": created,
                                }));
                            }
                        }
                        Err(e) => {
                            tracing::warn!("find_related_entities failed for {}: {}", rid, e);
                        }
                    }
                }

                // 10. Recent workflow executions for this repo
                let mut workflows = Vec::new();
                if let Some(ref rid) = repo_id {
                    let stmt = conn.prepare(
                        "SELECT id, workflow_id, status, current_step, started_at, finished_at, duration_ms
                         FROM workflow_executions WHERE workflow_id = ?1
                         ORDER BY started_at DESC LIMIT 5"
                    );
                    if let Ok(mut stmt) = stmt {
                        let rows = stmt.query_map([rid], |row| {
                            Ok(serde_json::json!({
                                "execution_id": row.get::<_, i64>(0)?,
                                "workflow_id": row.get::<_, String>(1)?,
                                "status": row.get::<_, String>(2)?,
                                "current_step": row.get::<_, Option<String>>(3)?,
                                "started_at": row.get::<_, String>(4)?,
                                "finished_at": row.get::<_, Option<String>>(5)?,
                                "duration_ms": row.get::<_, Option<i64>>(6)?,
                            }))
                        });
                        if let Ok(rows) = rows {
                            for v in rows.flatten() {
                                workflows.push(v);
                            }
                        }
                    }
                }

                // 11. Scan assets directory for project-related files
                let mut assets = Vec::new();
                if let Ok(ws) = crate::registry::WorkspaceRegistry::workspace_dir() {
                    let assets_dir = ws.join("assets");
                    if assets_dir.is_dir() {
                        // Try project-specific subdirectory first
                        let project_dir = assets_dir.join(&project);
                        let dirs_to_scan: Vec<_> = if project_dir.is_dir() {
                            vec![project_dir]
                        } else {
                            vec![assets_dir]
                        };
                        for dir in dirs_to_scan {
                            if let Ok(entries) = std::fs::read_dir(&dir) {
                                for entry in entries.flatten().take(20) {
                                    if let Ok(meta) = entry.metadata() {
                                        let name = entry.file_name().to_string_lossy().to_string();
                                        if meta.is_file() {
                                            assets.push(serde_json::json!({
                                                "name": name,
                                                "path": entry.path(),
                                            }));
                                        } else if meta.is_dir() {
                                            assets.push(serde_json::json!({
                                                "name": name,
                                                "path": entry.path(),
                                                "type": "folder",
                                            }));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                anyhow::Ok((repo_json, linked_vaults, modules, symbols, calls, assets, activity, related_symbols, relations, workflows))
            }
        })
        .await
        .map_err(|e| anyhow::anyhow!("spawn_blocking failed: {}", e))??;

        let (repo_json, linked_vaults, modules, symbols, calls, assets, activity, related_symbols, relations, workflows) = result;

        Ok(serde_json::json!({
            "success": true,
            "project": project,
            "repo": repo_json,
            "vault_notes": linked_vaults,
            "modules": modules,
            "symbols": symbols,
            "calls": calls,
            "activity": activity,
            "related_symbols": related_symbols,
            "relations": relations,
            "workflows": workflows,
            "assets": assets,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::McpTool;

    #[test]
    fn test_name() {
        let t = DevkitProjectContextTool;
        assert_eq!(t.name(), "devkit_project_context");
    }

    #[test]
    fn test_schema_is_object() {
        let t = DevkitProjectContextTool;
        let s = t.schema();
        assert!(s.is_object());
    }
}
