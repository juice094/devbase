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
            "description": "Get unified project context combining repo metadata, linked vault notes, and assets",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "project": {
                        "type": "string",
                        "description": "Project identifier (repo id, repo name, or vault note id/path)"
                    }
                },
                "required": ["project"]
            }
        })
    }

    async fn invoke(&self, args: serde_json::Value) -> anyhow::Result<serde_json::Value> {
        let project = args
            .get("project")
            .and_then(|v| v.as_str())
            .context("Missing required argument: project")?;

        let result = tokio::task::spawn_blocking({
            let project = project.to_string();
            move || {
                let conn = crate::registry::WorkspaceRegistry::init_db()?;

                // 1. Find repo by exact id or path substring
                let repos = crate::registry::WorkspaceRegistry::list_repos(&conn)?;
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
                        crate::registry::WorkspaceRegistry::get_linked_vault_notes(&conn, rid)?;
                    for (vid, vtitle) in notes {
                        linked_vaults.push(serde_json::json!({
                            "id": vid,
                            "title": vtitle,
                            "source": "link",
                        }));
                    }
                }

                // 3. Vault notes whose id/path contains the project name
                let all_notes = crate::registry::WorkspaceRegistry::list_vault_notes(&conn)?;
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

                // 4. Scan assets directory for project-related files
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

                anyhow::Ok((repo_json, linked_vaults, assets))
            }
        })
        .await
        .map_err(|e| anyhow::anyhow!("spawn_blocking failed: {}", e))??;

        let (repo_json, linked_vaults, assets) = result;

        Ok(serde_json::json!({
            "success": true,
            "project": project,
            "repo": repo_json,
            "vault_notes": linked_vaults,
            "assets": assets,
        }))
    }
}
