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
            "description": r#"Search the devbase Vault (Markdown notes) by keywords across note titles, tags, and full content. This is the primary discovery tool for the knowledge base.

Use this when the user wants to:
- Find notes related to a topic, architecture decision, or project
- Discover linked concepts via tags or wikilinks
- Locate a note when you only remember fragments of its content
- Check if a topic has been documented before writing a new note

Do NOT use this for:
- Reading the full content of a known note (use devkit_vault_read instead)
- Writing or updating notes (use devkit_vault_write instead)
- Finding backlinks to a specific note (use devkit_vault_backlinks instead)
- Searching across code repositories (use devkit_query_repos or devkit_natural_language_query instead)

Parameters:
- query: Space-separated keywords. All keywords must match (AND logic). Case-insensitive.

Returns: JSON array of matching notes. Each includes: id, title, path, and tags. Use devkit_vault_read with the id or path to retrieve full content."#,
            "inputSchema": {
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Search keywords" }
                },
                "required": ["query"]
            }
        })
    }

    async fn invoke(
        &self,
        args: serde_json::Value,
        ctx: &mut crate::storage::AppContext,
    ) -> anyhow::Result<serde_json::Value> {
        let query = args
            .get("query")
            .and_then(|v| v.as_str())
            .context("Missing required argument: query")?;

        let pool = ctx.pool();
        let results = tokio::task::spawn_blocking({
            let query = query.to_string();
            move || {
                let conn = pool.get()?;
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
            "description": r#"Read the complete Markdown content of a vault note, including its YAML frontmatter and body. This is the primary tool for retrieving knowledge base documents.

Use this when the user wants to:
- Read a specific note after finding it via devkit_vault_search
- Retrieve project documentation, architecture decisions, or design notes
- Extract the frontmatter metadata (tags, repo links, ai_context) from a note

Do NOT use this for:
- Searching for notes (use devkit_vault_search instead)
- Writing or updating notes (use devkit_vault_write instead)
- Finding backlinks (use devkit_vault_backlinks instead)
- Reading code files (use filesystem tools or devkit_project_context instead)

Parameters:
- path: Vault note file path or note id (e.g., "references/mcp-integration.md" or "mcp-integration-guide").

Returns: JSON with frontmatter (id, repo, tags, ai_context, created, updated) and body (Markdown content)."#,
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "File path or note id" }
                },
                "required": ["path"]
            }
        })
    }

    async fn invoke(
        &self,
        args: serde_json::Value,
        _ctx: &mut crate::storage::AppContext,
    ) -> anyhow::Result<serde_json::Value> {
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
            "description": r#"Write new content to a vault note or append to an existing one. Creates the file and directory structure automatically if needed. This is the primary tool for maintaining the knowledge base.

Use this when the user wants to:
- Create a new knowledge base document
- Update existing documentation with new findings
- Append a log entry or observation to a running note
- Save AI-generated analysis or summaries as persistent notes

Do NOT use this for:
- Attaching short annotations to repos (use devkit_note instead)
- Modifying code files (use git or filesystem tools)
- Deleting notes (not supported; move to archive manually)

Parameters:
- path: Target file path relative to the vault root (e.g., "ideas/new-feature.md").
- content: Markdown content to write.
- append: If true, appends to existing content. If false (default), overwrites.

Returns: JSON with success status and the written file path."#,
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

    async fn invoke(
        &self,
        args: serde_json::Value,
        _ctx: &mut crate::storage::AppContext,
    ) -> anyhow::Result<serde_json::Value> {
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
            "description": r#"Find all vault notes that contain wikilink references to a specific target note. This reveals the knowledge graph connections around a topic.

Use this when the user wants to:
- Discover which notes reference a given concept or project
- Map the knowledge graph around a central topic
- Find related documentation before making changes

Do NOT use this for:
- Keyword search across notes (use devkit_vault_search instead)
- Reading note content (use devkit_vault_read instead)
- Finding repo-to-repo relationships (not supported)

Parameters:
- note_id: Target note id or path (e.g., "01-Projects/devbase.md").

Returns: JSON array of backlinking notes, each with id, title, and path."#,
            "inputSchema": {
                "type": "object",
                "properties": {
                    "note_id": { "type": "string", "description": "Target note id or path (e.g., '01-Projects/devbase.md')" }
                },
                "required": ["note_id"]
            }
        })
    }

    async fn invoke(
        &self,
        args: serde_json::Value,
        _ctx: &mut crate::storage::AppContext,
    ) -> anyhow::Result<serde_json::Value> {
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
