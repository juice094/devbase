// SPDX-License-Identifier: MIT
// Copyright (c) 2026 juice094
use crate::clients::{DigestClient, VaultClient};
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

        let ctx = ctx.clone();
        let query_owned = query.to_string();
        let results = tokio::task::spawn_blocking(move || {
            let value = ctx.list_vault_notes()?;
            let notes_arr =
                value.get("notes").and_then(|v| v.as_array()).cloned().unwrap_or_default();
            let keywords: Vec<&str> = query_owned.split_whitespace().collect();

            let filtered: Vec<_> = notes_arr
                .into_iter()
                .filter(|n| {
                    let id = n.get("id").and_then(|v| v.as_str()).unwrap_or("");
                    let path = n.get("path").and_then(|v| v.as_str()).unwrap_or("");
                    let title = n.get("title").and_then(|v| v.as_str()).unwrap_or("");
                    let tags = n
                        .get("tags")
                        .and_then(|v| v.as_array())
                        .map(|arr| {
                            arr.iter().filter_map(|t| t.as_str()).collect::<Vec<_>>().join(",")
                        })
                        .unwrap_or_default();
                    let content = ctx
                        .read_vault_note(path)
                        .ok()
                        .and_then(|v| v.get("content").and_then(|c| c.as_str()).map(String::from))
                        .unwrap_or_default();
                    let hay = format!("{} {} {} {}", id, title, tags, content).to_lowercase();
                    keywords.iter().all(|kw| hay.contains(&kw.to_lowercase()))
                })
                .collect();

            anyhow::Ok(filtered)
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
        ctx: &mut crate::storage::AppContext,
    ) -> anyhow::Result<serde_json::Value> {
        let path = args
            .get("path")
            .and_then(|v| v.as_str())
            .context("Missing required argument: path")?;

        let value = ctx
            .read_vault_note(path)
            .context("Failed to read note — file not found or unreadable")?;
        let body = value.get("content").cloned().unwrap_or(serde_json::json!(""));
        let frontmatter = value.get("frontmatter").cloned().unwrap_or(serde_json::json!(null));

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
        ctx: &mut crate::storage::AppContext,
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

        let target = resolve_vault_write_path(ctx, path)?;

        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }

        if append && target.exists() {
            let existing = std::fs::read_to_string(&target).unwrap_or_default();
            std::fs::write(&target, format!("{}\n{}", existing, content))?;
        } else {
            std::fs::write(&target, content)?;
        }

        Ok(serde_json::json!({
            "success": true,
            "path": target.to_string_lossy().to_string(),
            "append": append,
        }))
    }
}

/// Resolve a vault write path by checking entities.local_path first,
/// then falling back to configured vault roots.
fn resolve_vault_write_path(
    ctx: &crate::storage::AppContext,
    path: &str,
) -> anyhow::Result<std::path::PathBuf> {
    // 1. Check if this note already exists in entities (has local_path)
    if let Ok(conn) = ctx.conn() {
        let local_path: Option<String> = conn
            .query_row(
                "SELECT local_path FROM entities WHERE entity_type = ?1 AND (id = ?2 OR name = ?2)",
                rusqlite::params![crate::registry::ENTITY_TYPE_VAULT_NOTE, path],
                |row| row.get(0),
            )
            .ok();
        if let Some(lp) = local_path {
            let p = std::path::PathBuf::from(lp);
            if p.exists() || p.parent().map(|d| d.exists()).unwrap_or(false) {
                return Ok(p);
            }
        }
    }

    // 2. Fall back to vault roots from config
    let vault_roots = match crate::config::Config::load() {
        Ok(cfg) if !cfg.vault.roots.is_empty() => {
            cfg.vault.roots.iter().map(std::path::PathBuf::from).collect()
        }
        _ => {
            vec![ctx
                .storage
                .workspace_dir()
                .map(|ws| ws.join("vault"))
                .unwrap_or_else(|_| std::path::PathBuf::from("vault"))]
        }
    };

    let relative = std::path::Path::new(path);
    for root in &vault_roots {
        let target = resolve_vault_relative_path(relative, root)?;
        if target.starts_with(root) {
            return Ok(target);
        }
    }

    anyhow::bail!("Path '{}' cannot be resolved under any configured vault root", path)
}

/// Resolve a relative path under a single vault root.
fn resolve_vault_relative_path(
    relative_path: &std::path::Path,
    vault_root: &std::path::Path,
) -> anyhow::Result<std::path::PathBuf> {
    let path = relative_path;
    if path.is_absolute() {
        anyhow::bail!("Absolute paths are not allowed in vault: {}", relative_path.display());
    }
    let s = relative_path.to_string_lossy();
    if s.starts_with('/') || s.starts_with('\\') {
        anyhow::bail!("Absolute paths are not allowed in vault: {}", relative_path.display());
    }

    let mut normalized = std::path::PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::Normal(name) => normalized.push(name),
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                if !normalized.pop() {
                    anyhow::bail!("Path escapes vault root: {}", relative_path.display());
                }
            }
            _ => anyhow::bail!("Invalid path component in: {}", relative_path.display()),
        }
    }

    let target = vault_root.join(&normalized);
    if !target.starts_with(vault_root) {
        anyhow::bail!("Path escapes vault root: {}", relative_path.display());
    }

    Ok(target)
}

/// Resolve a vault-relative path, enforcing that it stays within the vault root.
#[allow(dead_code)]
fn resolve_vault_path(
    relative_path: &str,
    vault_root: &std::path::Path,
) -> anyhow::Result<std::path::PathBuf> {
    let path = std::path::Path::new(relative_path);

    // Reject absolute paths and paths that start with a separator
    if path.is_absolute() || relative_path.starts_with('/') || relative_path.starts_with('\\') {
        anyhow::bail!("Absolute paths are not allowed in vault: {}", relative_path);
    }

    // Manual normalization: resolve . and .. components ourselves
    let mut normalized = std::path::PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::Normal(name) => normalized.push(name),
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                if !normalized.pop() {
                    // Too many ".." — would escape vault root
                    anyhow::bail!("Path escapes vault root: {}", relative_path);
                }
            }
            _ => anyhow::bail!("Invalid path component in: {}", relative_path),
        }
    }

    let target = vault_root.join(&normalized);

    // Final guard: starts_with is component-level comparison
    if !target.starts_with(vault_root) {
        anyhow::bail!("Path escapes vault root: {}", relative_path);
    }

    Ok(target)
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
        ctx: &mut crate::storage::AppContext,
    ) -> anyhow::Result<serde_json::Value> {
        let note_id = args
            .get("note_id")
            .and_then(|v| v.as_str())
            .context("Missing required argument: note_id")?;

        let ctx = ctx.clone();
        let note_id = note_id.to_string();
        let value = tokio::task::spawn_blocking(move || ctx.get_backlinks(&note_id))
            .await
            .map_err(|e| anyhow::anyhow!("spawn_blocking failed: {}", e))??;
        Ok(value)
    }
}

#[derive(Clone)]
pub struct DevkitVaultDailyTool;

impl McpTool for DevkitVaultDailyTool {
    fn name(&self) -> &'static str {
        "devkit_vault_daily"
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "description": r#"Generate a daily note in the vault under 99-Meta/Daily/YYYY-MM-DD.md. The note includes YAML frontmatter (date, tags: ["daily"]) and the day's devbase digest. If the file already exists, the digest is appended instead of overwriting.

Use this when the user wants to:
- Create a daily log entry summarizing devbase activity
- Persist the daily digest as a vault note for long-term reference

Parameters: none.

Returns: JSON with success status and the generated file path."#,
            "inputSchema": {
                "type": "object",
                "properties": {}
            }
        })
    }

    async fn invoke(
        &self,
        _args: serde_json::Value,
        ctx: &mut crate::storage::AppContext,
    ) -> anyhow::Result<serde_json::Value> {
        let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
        let rel_path = format!("99-Meta/Daily/{}.md", today);

        let ctx = ctx.clone();
        let today_owned = today.clone();
        let vault_root = ctx
            .storage
            .workspace_dir()
            .map(|ws| ws.join("vault"))
            .unwrap_or_else(|_| std::path::PathBuf::from("vault"));
        let file_path = tokio::task::spawn_blocking(move || {
            let digest = ctx.generate_daily_digest()?;
            let digest_str = digest.get("digest").and_then(|v| v.as_str()).unwrap_or("");

            let target = resolve_vault_relative_path(std::path::Path::new(&rel_path), &vault_root)?;

            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)?;
            }

            let content = if target.exists() {
                let existing = std::fs::read_to_string(&target)?;
                format!("{}\n\n{}", existing, digest_str)
            } else {
                format!("---\ndate: {}\ntags: [\"daily\"]\n---\n\n{}", today_owned, digest_str)
            };

            std::fs::write(&target, content)?;
            anyhow::Ok(target.to_string_lossy().to_string())
        })
        .await
        .map_err(|e| anyhow::anyhow!("spawn_blocking failed: {}", e))??;

        Ok(serde_json::json!({
            "success": true,
            "path": file_path,
            "date": today,
        }))
    }
}

#[derive(Clone)]
pub struct DevkitVaultGraphTool;

impl McpTool for DevkitVaultGraphTool {
    fn name(&self) -> &'static str {
        "devkit_vault_graph"
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "description": r#"Export the vault knowledge graph as a JSON structure of nodes (notes) and edges (wikilink relationships).

Use this when the user wants to:
- Visualize or analyze the structure of the knowledge base
- Export vault connections for external graph tools
- Understand the connectivity between topics and projects
- Traverse links starting from a specific note (bidirectional BFS)

Parameters:
- repo_id: Optional — if provided, only return notes linked to this repo and their mutual relationships.
- note_id: Optional — starting note ID for BFS traversal. If omitted, returns the full graph.
- depth: Optional — BFS depth (1-3). Default 1. Only effective when note_id is provided.

Returns: JSON with nodes (id, title) and edges (source, target)."#,
            "inputSchema": {
                "type": "object",
                "properties": {
                    "repo_id": { "type": "string", "description": "Optional repo ID to filter notes" },
                    "note_id": { "type": "string", "description": "Optional starting note ID for traversal" },
                    "depth": { "type": "integer", "description": "BFS depth 1-3 (default 1)", "minimum": 1, "maximum": 3 }
                }
            }
        })
    }

    async fn invoke(
        &self,
        args: serde_json::Value,
        ctx: &mut crate::storage::AppContext,
    ) -> anyhow::Result<serde_json::Value> {
        let repo_id = args.get("repo_id").and_then(|v| v.as_str()).map(|s| s.to_string());
        let note_id = args.get("note_id").and_then(|v| v.as_str()).map(|s| s.to_string());
        let depth = args.get("depth").and_then(|v| v.as_u64()).unwrap_or(1) as usize;

        let ctx = ctx.clone();
        let graph = tokio::task::spawn_blocking(move || {
            ctx.build_vault_graph(repo_id.as_deref(), note_id.as_deref(), depth)
        })
        .await
        .map_err(|e| anyhow::anyhow!("spawn_blocking failed: {}", e))??;

        Ok(graph)
    }
}

#[derive(Clone)]
pub struct DevkitVaultExportTool;

impl McpTool for DevkitVaultExportTool {
    fn name(&self) -> &'static str {
        "devkit_vault_export"
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "description": r#"Export the devbase Vault to a directory with integrity validation.

Copies all Markdown notes preserving PARA directory structure, validates wikilink targets, and checks frontmatter YAML parseability.

Use this when:
- Creating a backup of your knowledge base
- Migrating notes to Obsidian / Logseq / other Markdown tools
- Verifying vault integrity (broken links, malformed frontmatter)

Parameters:
- output_dir: Destination directory for the export (created if missing)

Returns: export statistics including file count, total bytes, broken links, and frontmatter errors."#,
            "inputSchema": {
                "type": "object",
                "properties": {
                    "output_dir": {
                        "type": "string",
                        "description": "Destination directory for the exported vault"
                    }
                },
                "required": ["output_dir"]
            }
        })
    }

    async fn invoke(
        &self,
        args: serde_json::Value,
        ctx: &mut crate::storage::AppContext,
    ) -> anyhow::Result<serde_json::Value> {
        let output_dir = args
            .get("output_dir")
            .and_then(|v| v.as_str())
            .context("Missing required argument: output_dir")?;
        ctx.export_vault(output_dir)
    }
}

#[derive(Clone)]
pub struct DevkitVaultHistoryTool;

impl McpTool for DevkitVaultHistoryTool {
    fn name(&self) -> &'static str {
        "devkit_vault_history"
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "description": r#"Get the Git revision history for a vault note.

Returns commit history (author, timestamp, message, insertions/deletions) for the specified note. Requires the vault directory to be a Git repository.

Parameters:
- note_id: Required — the vault note path (e.g., "ideas/note.md")

Returns: JSON with history array and count."#,
            "inputSchema": {
                "type": "object",
                "properties": {
                    "note_id": { "type": "string", "description": "Vault note path" }
                },
                "required": ["note_id"]
            }
        })
    }

    async fn invoke(
        &self,
        args: serde_json::Value,
        ctx: &mut crate::storage::AppContext,
    ) -> anyhow::Result<serde_json::Value> {
        let note_id = args
            .get("note_id")
            .and_then(|v| v.as_str())
            .context("Missing required argument: note_id")?;
        ctx.get_vault_history(note_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::StorageBackend;
    use std::time::Instant;

    fn vault_root() -> std::path::PathBuf {
        std::path::PathBuf::from("C:\\Users\\devbase\\workspace\\vault")
    }

    #[test]
    fn test_resolve_vault_path_normal() {
        let root = vault_root();
        let result = resolve_vault_path("ideas/note.md", &root).unwrap();
        assert_eq!(result, root.join("ideas/note.md"));
    }

    #[test]
    fn test_resolve_vault_path_nested() {
        let root = vault_root();
        let result = resolve_vault_path("01-Projects/rust/devbase.md", &root).unwrap();
        assert_eq!(result, root.join("01-Projects/rust/devbase.md"));
    }

    #[test]
    fn test_resolve_vault_path_with_dot() {
        let root = vault_root();
        let result = resolve_vault_path("./ideas/note.md", &root).unwrap();
        assert_eq!(result, root.join("ideas/note.md"));
    }

    #[test]
    fn test_resolve_vault_path_traversal_blocked() {
        let root = vault_root();
        assert!(resolve_vault_path("../../../etc/passwd", &root).is_err());
        assert!(resolve_vault_path("ideas/../../../../../.bashrc", &root).is_err());
    }

    #[test]
    fn test_resolve_vault_path_absolute_blocked() {
        let root = vault_root();
        assert!(resolve_vault_path("/etc/passwd", &root).is_err());
        assert!(resolve_vault_path("C:\\Windows\\System32\\drivers\\etc\\hosts", &root).is_err());
        assert!(resolve_vault_path("\\\\server\\share\\file.txt", &root).is_err());
    }

    #[test]
    fn test_resolve_vault_path_dotdot_within_bounds() {
        let root = vault_root();
        // "ideas/foo/../note.md" should resolve to "ideas/note.md"
        let result = resolve_vault_path("ideas/foo/../note.md", &root).unwrap();
        assert_eq!(result, root.join("ideas/note.md"));
    }

    #[test]
    fn test_resolve_vault_path_empty() {
        let root = vault_root();
        let result = resolve_vault_path("", &root).unwrap();
        assert_eq!(result, root);
    }

    #[test]
    fn test_resolve_vault_path_performance() {
        let root = vault_root();
        let iterations = 100_000;
        let start = Instant::now();
        for _ in 0..iterations {
            let _ = resolve_vault_path("01-Projects/rust/devbase.md", &root).unwrap();
        }
        let elapsed = start.elapsed();
        let avg_ns = elapsed.as_nanos() as f64 / iterations as f64;
        println!(
            "resolve_vault_path: {} iterations in {:?} (avg {:.0} ns/op)",
            iterations, elapsed, avg_ns
        );
        // Guard: must complete within reasonable time (< 1s for 100k ops)
        assert!(elapsed.as_secs() < 1, "resolve_vault_path too slow: {:?}", elapsed);
    }

    #[tokio::test]
    async fn test_vault_daily_creates_file() {
        let backend = std::sync::Arc::new(crate::storage::TempStorageBackend::new());
        let mut ctx = crate::storage::AppContext::with_storage(backend).unwrap();
        let tool = DevkitVaultDailyTool;
        let result = tool.invoke(serde_json::json!({}), &mut ctx).await.unwrap();

        assert_eq!(result.get("success").unwrap(), true);
        let path = result.get("path").unwrap().as_str().unwrap();
        assert!(path.contains("99-Meta"));
        assert!(path.contains("Daily"));

        let content = std::fs::read_to_string(path).unwrap();
        assert!(content.contains("---"));
        assert!(content.contains("tags: [\"daily\"]"));
        // Digest contains repo count regardless of language
        assert!(content.contains("repos in db"));
    }

    #[tokio::test]
    async fn test_vault_daily_appends_to_existing() {
        let backend = std::sync::Arc::new(crate::storage::TempStorageBackend::new());
        let mut ctx = crate::storage::AppContext::with_storage(backend).unwrap();
        let tool = DevkitVaultDailyTool;

        // First call creates the file
        let _ = tool.invoke(serde_json::json!({}), &mut ctx).await.unwrap();

        // Second call appends
        let result = tool.invoke(serde_json::json!({}), &mut ctx).await.unwrap();
        assert_eq!(result.get("success").unwrap(), true);

        let path = result.get("path").unwrap().as_str().unwrap();
        let content = std::fs::read_to_string(path).unwrap();
        let count = content.matches("repos in db").count();
        assert!(count >= 2, "Expected at least 2 digest occurrences, found {}", count);
    }

    #[tokio::test]
    async fn test_vault_graph_basic() {
        let backend = std::sync::Arc::new(crate::storage::TempStorageBackend::new());
        let vault_dir = backend.workspace_dir().unwrap().join("vault");
        std::fs::create_dir_all(&vault_dir).unwrap();

        std::fs::write(
            vault_dir.join("a.md"),
            "---\ntitle: Note A\n---\n\nLinks to [[b]] and [[c]].\n",
        )
        .unwrap();
        std::fs::write(vault_dir.join("b.md"), "---\ntitle: Note B\n---\n\nLinks to [[c]].\n")
            .unwrap();
        std::fs::write(vault_dir.join("c.md"), "---\ntitle: Note C\n---\n\nNo links.\n").unwrap();

        let mut ctx = crate::storage::AppContext::with_storage(backend).unwrap();
        let pool = ctx.pool();
        let vd = vault_dir.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get()?;
            crate::vault::scanner::scan_vault(&mut conn, Some(&vd))
        })
        .await
        .unwrap()
        .unwrap();

        let tool = DevkitVaultGraphTool;
        let result = tool.invoke(serde_json::json!({}), &mut ctx).await.unwrap();

        assert_eq!(result.get("success").unwrap(), true);
        let nodes = result.get("nodes").unwrap().as_array().unwrap();
        let edges = result.get("edges").unwrap().as_array().unwrap();

        assert_eq!(nodes.len(), 3);
        let titles: Vec<&str> =
            nodes.iter().map(|n| n.get("title").unwrap().as_str().unwrap()).collect();
        assert!(titles.contains(&"Note A"));
        assert!(titles.contains(&"Note B"));
        assert!(titles.contains(&"Note C"));

        // Edges: a->b, a->c, b->c
        assert_eq!(edges.len(), 3);
    }

    #[tokio::test]
    async fn test_vault_graph_filtered_by_repo() {
        let backend = std::sync::Arc::new(crate::storage::TempStorageBackend::new());
        let vault_dir = backend.workspace_dir().unwrap().join("vault");
        std::fs::create_dir_all(&vault_dir).unwrap();

        std::fs::write(
            vault_dir.join("repo-a-note.md"),
            "---\ntitle: Repo A Note\nrepo: repo-a\n---\n\nLinks to [[repo-b-note]].\n",
        )
        .unwrap();
        std::fs::write(
            vault_dir.join("repo-b-note.md"),
            "---\ntitle: Repo B Note\nrepo: repo-b\n---\n\nNo links.\n",
        )
        .unwrap();

        let mut ctx = crate::storage::AppContext::with_storage(backend).unwrap();
        let pool = ctx.pool();
        let vd = vault_dir.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get()?;
            crate::vault::scanner::scan_vault(&mut conn, Some(&vd))
        })
        .await
        .unwrap()
        .unwrap();

        let tool = DevkitVaultGraphTool;
        let result =
            tool.invoke(serde_json::json!({ "repo_id": "repo-a" }), &mut ctx).await.unwrap();

        assert_eq!(result.get("success").unwrap(), true);
        let nodes = result.get("nodes").unwrap().as_array().unwrap();
        let edges = result.get("edges").unwrap().as_array().unwrap();

        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].get("id").unwrap().as_str().unwrap(), "repo-a-note.md");
        assert_eq!(edges.len(), 0);
    }

    #[tokio::test]
    async fn test_vault_graph_bfs_traversal() {
        let backend = std::sync::Arc::new(crate::storage::TempStorageBackend::new());
        let vault_dir = backend.workspace_dir().unwrap().join("vault");
        std::fs::create_dir_all(&vault_dir).unwrap();

        std::fs::write(
            vault_dir.join("a.md"),
            "---\ntitle: Note A\n---\n\nLinks to [[b]] and [[c]].\n",
        )
        .unwrap();
        std::fs::write(vault_dir.join("b.md"), "---\ntitle: Note B\n---\n\nLinks to [[d]].\n")
            .unwrap();
        std::fs::write(vault_dir.join("c.md"), "---\ntitle: Note C\n---\n\nNo links.\n").unwrap();
        std::fs::write(vault_dir.join("d.md"), "---\ntitle: Note D\n---\n\nNo links.\n").unwrap();

        let mut ctx = crate::storage::AppContext::with_storage(backend).unwrap();
        let pool = ctx.pool();
        let vd = vault_dir.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get()?;
            crate::vault::scanner::scan_vault(&mut conn, Some(&vd))
        })
        .await
        .unwrap()
        .unwrap();

        let tool = DevkitVaultGraphTool;

        // Depth 1: a -> b, c
        let result = tool
            .invoke(serde_json::json!({ "note_id": "a.md", "depth": 1 }), &mut ctx)
            .await
            .unwrap();
        assert_eq!(result.get("success").unwrap(), true);
        let nodes = result.get("nodes").unwrap().as_array().unwrap();
        let edges = result.get("edges").unwrap().as_array().unwrap();
        assert_eq!(nodes.len(), 3);
        assert_eq!(edges.len(), 2);

        // Depth 2: a -> b -> d
        let result = tool
            .invoke(serde_json::json!({ "note_id": "a.md", "depth": 2 }), &mut ctx)
            .await
            .unwrap();
        assert_eq!(result.get("success").unwrap(), true);
        let nodes = result.get("nodes").unwrap().as_array().unwrap();
        let edges = result.get("edges").unwrap().as_array().unwrap();
        assert_eq!(nodes.len(), 4);
        assert_eq!(edges.len(), 3);
    }

    #[tokio::test]
    async fn test_vault_history_tool() {
        let backend = std::sync::Arc::new(crate::storage::TempStorageBackend::new());
        let vault_dir = backend.workspace_dir().unwrap().join("vault");
        let repo = git2::Repository::init(&vault_dir).unwrap();
        let sig = git2::Signature::now("Test", "test@example.com").unwrap();

        {
            let mut index = repo.index().unwrap();
            std::fs::write(vault_dir.join("note.md"), "Hello world\n").unwrap();
            index.add_path(std::path::Path::new("note.md")).unwrap();
            index.write().unwrap();
            let tree_id = index.write_tree().unwrap();
            let tree = repo.find_tree(tree_id).unwrap();
            repo.commit(Some("HEAD"), &sig, &sig, "Initial", &tree, &[]).unwrap();
        }
        {
            let mut index = repo.index().unwrap();
            std::fs::write(vault_dir.join("note.md"), "Hello world\nMore lines\n").unwrap();
            index.add_path(std::path::Path::new("note.md")).unwrap();
            index.write().unwrap();
            let tree_id = index.write_tree().unwrap();
            let tree = repo.find_tree(tree_id).unwrap();
            let parent = repo.head().unwrap().peel_to_commit().unwrap();
            repo.commit(Some("HEAD"), &sig, &sig, "Update", &tree, &[&parent]).unwrap();
        }

        let mut ctx = crate::storage::AppContext::with_storage(backend).unwrap();
        let tool = DevkitVaultHistoryTool;
        let result = tool
            .invoke(serde_json::json!({ "note_id": "note.md" }), &mut ctx)
            .await
            .unwrap();

        assert_eq!(result.get("success").unwrap(), true);
        let history = result.get("history").unwrap().as_array().unwrap();
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].get("message").unwrap().as_str().unwrap(), "Initial");
        assert_eq!(history[1].get("message").unwrap().as_str().unwrap(), "Update");
        assert!(history[1].get("insertions").unwrap().as_u64().unwrap() > 0);
    }
}
