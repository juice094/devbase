use crate::mcp::McpTool;
use anyhow::Context;

#[derive(Clone)]
pub struct DevkitVaultSearchTool;

impl McpTool for DevkitVaultSearchTool {
    fn name(&self) -> &'static str {
        "devkit_vault_search"
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "description": "Search vault notes by keywords in title, tags, or content",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Search keywords" }
                },
                "required": ["query"]
            }
        })
    }

    async fn invoke(&self, args: serde_json::Value) -> anyhow::Result<serde_json::Value> {
        let query = args
            .get("query")
            .and_then(|v| v.as_str())
            .context("Missing required argument: query")?;

        let results = tokio::task::spawn_blocking({
            let query = query.to_string();
            move || {
                let conn = crate::registry::WorkspaceRegistry::init_db()?;
                let notes = crate::registry::WorkspaceRegistry::list_vault_notes(&conn)?;
                let keywords: Vec<&str> = query.split_whitespace().collect();

                let filtered: Vec<_> = notes
                    .into_iter()
                    .filter(|n| {
                        let content = crate::vault::fs_io::read_note_body(&n.path)
                            .map(|(body, _fm)| body)
                            .unwrap_or_default();
                        let hay = format!(
                            "{} {} {} {}",
                            n.id,
                            n.title.as_deref().unwrap_or(""),
                            n.tags.join(","),
                            content
                        )
                        .to_lowercase();
                        keywords.iter().all(|kw| hay.contains(&kw.to_lowercase()))
                    })
                    .map(|n| {
                        serde_json::json!({
                            "id": n.id,
                            "title": n.title,
                            "path": n.path,
                            "tags": n.tags,
                        })
                    })
                    .collect();

                anyhow::Ok(filtered)
            }
        })
        .await
        .map_err(|e| anyhow::anyhow!("spawn_blocking failed: {}", e))??;

        Ok(serde_json::json!({
            "success": true,
            "count": results.len(),
            "query": query,
            "notes": results,
        }))
    }
}

#[derive(Clone)]
pub struct DevkitVaultReadTool;

impl McpTool for DevkitVaultReadTool {
    fn name(&self) -> &'static str {
        "devkit_vault_read"
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "description": "Read the full content of a vault note by its path or id",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "File path or note id" }
                },
                "required": ["path"]
            }
        })
    }

    async fn invoke(&self, args: serde_json::Value) -> anyhow::Result<serde_json::Value> {
        let path = args
            .get("path")
            .and_then(|v| v.as_str())
            .context("Missing required argument: path")?;

        let (body, frontmatter) = crate::vault::fs_io::read_note_body(path)
            .context("Failed to read note — file not found or unreadable")?;

        Ok(serde_json::json!({
            "success": true,
            "path": path,
            "frontmatter": frontmatter,
            "content": body,
        }))
    }
}

#[derive(Clone)]
pub struct DevkitVaultWriteTool;

impl McpTool for DevkitVaultWriteTool {
    fn name(&self) -> &'static str {
        "devkit_vault_write"
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "description": "Write or append content to a vault note. Creates the file if it does not exist.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Target file path" },
                    "content": { "type": "string", "description": "Content to write" },
                    "append": { "type": "boolean", "description": "If true, append instead of overwrite", "default": false }
                },
                "required": ["path", "content"]
            }
        })
    }

    async fn invoke(&self, args: serde_json::Value) -> anyhow::Result<serde_json::Value> {
        let path = args
            .get("path")
            .and_then(|v| v.as_str())
            .context("Missing required argument: path")?;
        let content = args
            .get("content")
            .and_then(|v| v.as_str())
            .context("Missing required argument: content")?;
        let append = args.get("append").and_then(|v| v.as_bool()).unwrap_or(false);

        let path = std::path::Path::new(path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        if append && path.exists() {
            let existing = std::fs::read_to_string(path).unwrap_or_default();
            std::fs::write(path, format!("{}\n{}", existing, content))?;
        } else {
            std::fs::write(path, content)?;
        }

        Ok(serde_json::json!({
            "success": true,
            "path": path,
            "append": append,
        }))
    }
}

#[derive(Clone)]
pub struct DevkitVaultBacklinksTool;

impl McpTool for DevkitVaultBacklinksTool {
    fn name(&self) -> &'static str {
        "devkit_vault_backlinks"
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "description": "Find all vault notes that link to a given note (backlinks)",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "note_id": { "type": "string", "description": "Target note id or path (e.g., '01-Projects/devbase.md')" }
                },
                "required": ["note_id"]
            }
        })
    }

    async fn invoke(&self, args: serde_json::Value) -> anyhow::Result<serde_json::Value> {
        let note_id = args
            .get("note_id")
            .and_then(|v| v.as_str())
            .context("Missing required argument: note_id")?;

        let backlinks = tokio::task::spawn_blocking({
            let note_id = note_id.to_string();
            move || {
                let vault_dir = crate::registry::WorkspaceRegistry::workspace_dir()
                    .ok()
                    .map(|ws| ws.join("vault"));
                if let Some(vd) = vault_dir {
                    match crate::vault::backlinks::build_backlink_index(&vd) {
                        Ok(index) => crate::vault::backlinks::get_backlinks(&index, &note_id),
                        Err(_) => Vec::new(),
                    }
                } else {
                    Vec::new()
                }
            }
        })
        .await
        .map_err(|e| anyhow::anyhow!("spawn_blocking failed: {}", e))?;

        Ok(serde_json::json!({
            "success": true,
            "target": note_id,
            "count": backlinks.len(),
            "backlinks": backlinks,
        }))
    }
}
