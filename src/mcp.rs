use anyhow::Context;
use rusqlite::OptionalExtension;
use std::collections::HashMap;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};

pub trait McpTool: Send + Sync + Clone {
    fn name(&self) -> &'static str;
    fn schema(&self) -> serde_json::Value;
    async fn invoke(&self, args: serde_json::Value) -> anyhow::Result<serde_json::Value>;
}

#[derive(Clone)]
pub(crate) enum McpToolEnum {
    Scan(DevkitScanTool),
    Health(DevkitHealthTool),
    Sync(DevkitSyncTool),
    Query(DevkitQueryTool),
    Index(DevkitIndexTool),
    Note(DevkitNoteTool),
    Digest(DevkitDigestTool),
    Paper(DevkitPaperIndexTool),
    Experiment(DevkitExperimentLogTool),
    GithubInfo(DevkitGithubInfoTool),
}

impl McpTool for McpToolEnum {
    fn name(&self) -> &'static str {
        match self {
            McpToolEnum::Scan(t) => t.name(),
            McpToolEnum::Health(t) => t.name(),
            McpToolEnum::Sync(t) => t.name(),
            McpToolEnum::Query(t) => t.name(),
            McpToolEnum::Index(t) => t.name(),
            McpToolEnum::Note(t) => t.name(),
            McpToolEnum::Digest(t) => t.name(),
            McpToolEnum::Paper(t) => t.name(),
            McpToolEnum::Experiment(t) => t.name(),
            McpToolEnum::GithubInfo(t) => t.name(),
        }
    }

    fn schema(&self) -> serde_json::Value {
        match self {
            McpToolEnum::Scan(t) => t.schema(),
            McpToolEnum::Health(t) => t.schema(),
            McpToolEnum::Sync(t) => t.schema(),
            McpToolEnum::Query(t) => t.schema(),
            McpToolEnum::Index(t) => t.schema(),
            McpToolEnum::Note(t) => t.schema(),
            McpToolEnum::Digest(t) => t.schema(),
            McpToolEnum::Paper(t) => t.schema(),
            McpToolEnum::Experiment(t) => t.schema(),
            McpToolEnum::GithubInfo(t) => t.schema(),
        }
    }

    async fn invoke(&self, args: serde_json::Value) -> anyhow::Result<serde_json::Value> {
        match self {
            McpToolEnum::Scan(t) => t.invoke(args).await,
            McpToolEnum::Health(t) => t.invoke(args).await,
            McpToolEnum::Sync(t) => t.invoke(args).await,
            McpToolEnum::Query(t) => t.invoke(args).await,
            McpToolEnum::Index(t) => t.invoke(args).await,
            McpToolEnum::Note(t) => t.invoke(args).await,
            McpToolEnum::Digest(t) => t.invoke(args).await,
            McpToolEnum::Paper(t) => t.invoke(args).await,
            McpToolEnum::Experiment(t) => t.invoke(args).await,
            McpToolEnum::GithubInfo(t) => t.invoke(args).await,
        }
    }
}

pub struct McpServer {
    tools: HashMap<String, McpToolEnum>,
}

impl McpServer {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    pub fn register_tool(mut self, tool: McpToolEnum) -> Self {
        self.tools.insert(tool.name().to_string(), tool);
        self
    }

    pub async fn handle_request(&self, req: serde_json::Value) -> anyhow::Result<serde_json::Value> {
        let id = req.get("id").cloned().unwrap_or(serde_json::Value::Null);
        let method = req
            .get("method")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        match method {
            "initialize" => Ok(serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "protocolVersion": "2024-11-05",
                    "capabilities": {
                        "tools": {}
                    },
                    "serverInfo": {
                        "name": "devbase",
                        "version": "0.1.0"
                    }
                }
            })),
            "tools/list" => {
                let tools: Vec<serde_json::Value> = self
                    .tools
                    .values()
                    .map(|t| {
                        let schema = t.schema();
                        serde_json::json!({
                            "name": t.name(),
                            "description": schema.get("description").and_then(|v| v.as_str()).unwrap_or(""),
                            "inputSchema": schema.get("inputSchema").cloned().unwrap_or(serde_json::json!({}))
                        })
                    })
                    .collect();
                Ok(serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": { "tools": tools }
                }))
            }
            "tools/call" => {
                let params = req.get("params").cloned().unwrap_or(serde_json::Value::Null);
                let name = params
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let args = params
                    .get("arguments")
                    .cloned()
                    .unwrap_or(serde_json::Value::Null);

                match self.tools.get(name) {
                    Some(tool) => match tool.invoke(args).await {
                        Ok(result) => {
                            let text = result.to_string();
                            let is_error = !result.get("success").and_then(|v: &serde_json::Value| v.as_bool()).unwrap_or(false);
                            let content = serde_json::json!({
                                "type": "text",
                                "text": text
                            });
                            Ok(serde_json::json!({
                                "jsonrpc": "2.0",
                                "id": id,
                                "result": {
                                    "content": [content],
                                    "isError": is_error
                                }
                            }))
                        }
                        Err(e) => {
                            let payload = serde_json::json!({ "success": false, "error": e.to_string() });
                            let text = serde_json::to_string(&payload)?;
                            let content = serde_json::json!({ "type": "text", "text": text });
                            Ok(serde_json::json!({
                                "jsonrpc": "2.0",
                                "id": id,
                                "result": {
                                    "content": [content],
                                    "isError": true
                                }
                            }))
                        }
                    },
                    None => Ok(serde_json::json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "error": {
                            "code": -32602,
                            "message": format!("Tool '{}' not found", name)
                        }
                    })),
                }
            }
            _ => Ok(serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": {
                    "code": -32601,
                    "message": format!("Method '{}' not found", method)
                }
            })),
        }
    }

}

pub fn build_server() -> McpServer {
    McpServer::new()
        .register_tool(McpToolEnum::Scan(DevkitScanTool))
        .register_tool(McpToolEnum::Health(DevkitHealthTool))
        .register_tool(McpToolEnum::Sync(DevkitSyncTool))
        .register_tool(McpToolEnum::Query(DevkitQueryTool))
        .register_tool(McpToolEnum::Index(DevkitIndexTool))
        .register_tool(McpToolEnum::Note(DevkitNoteTool))
        .register_tool(McpToolEnum::Digest(DevkitDigestTool))
        .register_tool(McpToolEnum::Paper(DevkitPaperIndexTool))
        .register_tool(McpToolEnum::Experiment(DevkitExperimentLogTool))
        .register_tool(McpToolEnum::GithubInfo(DevkitGithubInfoTool))
}

pub fn format_mcp_message(body: &serde_json::Value) -> String {
    let body_str = body.to_string();
    format!("Content-Length: {}\r\n\r\n{}\n", body_str.len(), body_str)
}

pub async fn run_stdio() -> anyhow::Result<()> {
    let server = build_server();
    let stdin = tokio::io::stdin();
    let mut stdout = tokio::io::stdout();
    let mut reader = BufReader::new(stdin);
    let mut line_buf = String::new();

    loop {
        line_buf.clear();
        // Read header line to get Content-Length
        let n = reader.read_line(&mut line_buf).await?;
        if n == 0 {
            break; // EOF
        }
        let line = line_buf.trim();
        if line.is_empty() {
            continue;
        }
        
        let content_length = if line.starts_with("Content-Length: ") {
            line.strip_prefix("Content-Length: ")
                .and_then(|v| v.parse::<usize>().ok())
        } else {
            // Fallback: parse raw JSON line for backward compatibility
            let req: serde_json::Value = match serde_json::from_str(line) {
                Ok(v) => v,
                Err(e) => {
                    let resp = serde_json::json!({
                        "jsonrpc": "2.0",
                        "id": null,
                        "error": {
                            "code": -32700,
                            "message": format!("Parse error: {}", e)
                        }
                    });
                    let msg = format_mcp_message(&resp);
                    let _ = stdout.write_all(msg.as_bytes()).await;
                    let _ = stdout.flush().await;
                    continue;
                }
            };
            let resp = server.handle_request(req).await.unwrap_or_else(|e| {
                serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": null,
                    "error": {
                        "code": -32603,
                        "message": format!("Internal error: {}", e)
                    }
                })
            });
            let msg = format_mcp_message(&resp);
            let _ = stdout.write_all(msg.as_bytes()).await;
            let _ = stdout.flush().await;
            continue;
        };
        
        let content_length = match content_length {
            Some(len) => len,
            None => {
                let resp = serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": null,
                    "error": {
                        "code": -32700,
                        "message": format!("Invalid Content-Length header: {}", line)
                    }
                });
                let msg = format_mcp_message(&resp);
                let _ = stdout.write_all(msg.as_bytes()).await;
                let _ = stdout.flush().await;
                continue;
            }
        };
        
        // Read the empty line (\r\n or \n)
        line_buf.clear();
        let _ = reader.read_line(&mut line_buf).await;
        
        // Read the exact number of bytes
        let mut body_buf = vec![0u8; content_length];
        if let Err(e) = reader.read_exact(&mut body_buf).await {
            let resp = serde_json::json!({
                "jsonrpc": "2.0",
                "id": null,
                "error": {
                    "code": -32700,
                    "message": format!("Failed to read request body: {}", e)
                }
            });
            let msg = format_mcp_message(&resp);
            let _ = stdout.write_all(msg.as_bytes()).await;
            let _ = stdout.flush().await;
            continue;
        }
        
        // Some clients include a trailing newline after the body; consume it if present
        line_buf.clear();
        let _ = reader.read_line(&mut line_buf).await;
        
        let req: serde_json::Value = match String::from_utf8(body_buf) {
            Ok(body) => match serde_json::from_str(&body) {
                Ok(v) => v,
                Err(e) => {
                    let resp = serde_json::json!({
                        "jsonrpc": "2.0",
                        "id": null,
                        "error": {
                            "code": -32700,
                            "message": format!("Parse error: {}", e)
                        }
                    });
                    let msg = format_mcp_message(&resp);
                    let _ = stdout.write_all(msg.as_bytes()).await;
                    let _ = stdout.flush().await;
                    continue;
                }
            },
            Err(e) => {
                let resp = serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": null,
                    "error": {
                        "code": -32700,
                        "message": format!("Invalid UTF-8: {}", e)
                    }
                });
                let msg = format_mcp_message(&resp);
                let _ = stdout.write_all(msg.as_bytes()).await;
                let _ = stdout.flush().await;
                continue;
            }
        };

        let resp = server.handle_request(req).await.unwrap_or_else(|e| {
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": null,
                "error": {
                    "code": -32603,
                    "message": format!("Internal error: {}", e)
                }
            })
        });

        let msg = format_mcp_message(&resp);
        let _ = stdout.write_all(msg.as_bytes()).await;
        let _ = stdout.flush().await;
    }

    Ok(())
}

// ------------------------------------------------------------------
// Tools
// ------------------------------------------------------------------

#[derive(Clone)]
pub struct DevkitScanTool;

impl McpTool for DevkitScanTool {
    fn name(&self) -> &'static str {
        "devkit_scan"
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "description": "Scan a directory for Git repositories and optionally register them",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Directory to scan",
                        "default": "."
                    },
                    "register": {
                        "type": "boolean",
                        "description": "Register discovered repos into the database",
                        "default": false
                    }
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
        let register = args.get("register").and_then(|v| v.as_bool()).unwrap_or(false);
        crate::scan::run_json(path, register).await
    }
}

#[derive(Clone)]
pub struct DevkitHealthTool;

impl McpTool for DevkitHealthTool {
    fn name(&self) -> &'static str {
        "devkit_health"
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "description": "Check the health of registered repositories and the environment",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "detail": {
                        "type": "boolean",
                        "description": "Show detailed per-repo status",
                        "default": false
                    }
                }
            }
        })
    }

    async fn invoke(&self, args: serde_json::Value) -> anyhow::Result<serde_json::Value> {
        let detail = args.get("detail").and_then(|v| v.as_bool()).unwrap_or(false);
        let config = crate::config::Config::load()?;
        crate::health::run_json(detail, 0, 1, config.cache.ttl_seconds).await
    }

}

#[derive(Clone)]
pub struct DevkitSyncTool;

impl McpTool for DevkitSyncTool {
    fn name(&self) -> &'static str {
        "devkit_sync"
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "description": "Sync registered repositories with their upstream remotes",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "dry_run": {
                        "type": "boolean",
                        "description": "Preview mode: do not modify any files",
                        "default": true
                    },
                    "strategy": {
                        "type": "string",
                        "enum": ["fetch-only", "auto-pull", "ask"],
                        "description": "Sync strategy",
                        "default": "fetch-only"
                    },
                    "filter_tags": {
                        "type": "string",
                        "description": "Comma-separated tags to filter repos",
                        "default": ""
                    }
                }
            }
        })
    }

    async fn invoke(&self, args: serde_json::Value) -> anyhow::Result<serde_json::Value> {
        let dry_run = args.get("dry_run").and_then(|v| v.as_bool()).unwrap_or(true);
        let filter_tags = args.get("filter_tags").and_then(|v| v.as_str());
        crate::sync::run_json(dry_run, filter_tags, None).await
    }
}

#[derive(Clone)]
pub struct DevkitIndexTool;

impl McpTool for DevkitIndexTool {
    fn name(&self) -> &'static str {
        "devkit_index"
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "description": "Index repository summaries and module structures",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Specific path to index; if omitted, index all registered repos",
                        "default": ""
                    }
                }
            }
        })
    }

    async fn invoke(&self, args: serde_json::Value) -> anyhow::Result<serde_json::Value> {
        let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
        let path_owned = path.to_string();
        let count = tokio::task::spawn_blocking(move || crate::knowledge_engine::run_index(&path_owned))
            .await
            .map_err(|e| anyhow::anyhow!("spawn_blocking failed: {}", e))??;
        Ok(serde_json::json!({ "success": true, "indexed": count, "errors": 0 }))
    }
}

#[derive(Clone)]
pub struct DevkitNoteTool;

impl McpTool for DevkitNoteTool {
    fn name(&self) -> &'static str {
        "devkit_note"
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "description": "Add a note to a repository",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "repo_id": { "type": "string" },
                    "text": { "type": "string" },
                    "author": { "type": "string", "default": "ai" }
                },
                "required": ["repo_id", "text"]
            }
        })
    }

    async fn invoke(&self, args: serde_json::Value) -> anyhow::Result<serde_json::Value> {
        let repo_id = args.get("repo_id").and_then(|v| v.as_str()).context("repo_id required")?;
        let text = args.get("text").and_then(|v| v.as_str()).context("text required")?;
        let author = args.get("author").and_then(|v| v.as_str()).unwrap_or("ai");
        let repo_id = repo_id.to_string();
        let text = text.to_string();
        let author = author.to_string();
        tokio::task::spawn_blocking(move || {
            let conn = crate::registry::WorkspaceRegistry::init_db()?;
            crate::registry::WorkspaceRegistry::save_note(&conn, &repo_id, &text, &author)?;
            Ok::<_, anyhow::Error>(serde_json::json!({ "success": true }))
        })
        .await
        .map_err(|e| anyhow::anyhow!("spawn_blocking failed: {}", e))?
    }
}

#[derive(Clone)]
pub struct DevkitDigestTool;

impl McpTool for DevkitDigestTool {
    fn name(&self) -> &'static str { "devkit_digest" }
    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "description": "Generate daily knowledge digest",
            "inputSchema": { "type": "object", "properties": {} }
        })
    }
    async fn invoke(&self, _args: serde_json::Value) -> anyhow::Result<serde_json::Value> {
        tokio::task::spawn_blocking(|| {
            let conn = crate::registry::WorkspaceRegistry::init_db()?;
            let config = crate::config::Config::load()?;
            let text = crate::digest::generate_daily_digest(&conn, &config)?;
            Ok::<_, anyhow::Error>(serde_json::json!({ "success": true, "digest": text }))
        })
        .await
        .map_err(|e| anyhow::anyhow!("spawn_blocking failed: {}", e))?
    }
}

#[derive(Clone)]
pub struct DevkitPaperIndexTool;

impl McpTool for DevkitPaperIndexTool {
    fn name(&self) -> &'static str { "devkit_paper_index" }
    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "description": "Scan a directory for PDF papers and index them",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Directory containing PDFs", "default": "~/papers" },
                    "tags": { "type": "string", "description": "Comma-separated tags to apply", "default": "" }
                }
            }
        })
    }
    async fn invoke(&self, args: serde_json::Value) -> anyhow::Result<serde_json::Value> {
        let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("~/papers");
        let tags_str = args.get("tags").and_then(|v| v.as_str()).unwrap_or("");
        let tags: Vec<String> = tags_str.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
        let path = if path.starts_with("~/") {
            dirs::home_dir().map(|d| d.join(&path[2..])).unwrap_or_else(|| std::path::PathBuf::from(path))
        } else {
            std::path::PathBuf::from(path)
        };

        tokio::task::spawn_blocking(move || {
            let conn = crate::registry::WorkspaceRegistry::init_db()?;
            let mut count = 0;
            if path.is_dir() {
                for entry in std::fs::read_dir(&path)? {
                    let entry = entry?;
                    let name = entry.file_name().to_string_lossy().to_string();
                    if name.to_lowercase().ends_with(".pdf") {
                        let id = name.trim_end_matches(".pdf").trim_end_matches(".PDF").to_string();
                        // Simple heuristic: if filename contains arXiv format (e.g., 2507.03616)
                        let title = if id.chars().filter(|c| c.is_numeric() || *c == '.').count() > 5 {
                            format!("arXiv:{}", id)
                        } else {
                            id.clone()
                        };
                        let paper = crate::registry::PaperEntry {
                            id: id.clone(),
                            title,
                            authors: None,
                            venue: None,
                            year: None,
                            pdf_path: Some(entry.path().to_string_lossy().to_string()),
                            bibtex: None,
                            tags: tags.clone(),
                            added_at: chrono::Utc::now(),
                        };
                        crate::registry::WorkspaceRegistry::save_paper(&conn, &paper)?;
                        count += 1;
                    }
                }
            }
            Ok::<_, anyhow::Error>(serde_json::json!({ "success": true, "indexed": count }))
        })
        .await
        .map_err(|e| anyhow::anyhow!("spawn_blocking failed: {}", e))?
    }
}

#[derive(Clone)]
pub struct DevkitExperimentLogTool;

impl McpTool for DevkitExperimentLogTool {
    fn name(&self) -> &'static str { "devkit_experiment_log" }
    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "description": "Log an experiment run",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "id": { "type": "string", "description": "Experiment identifier" },
                    "repo_id": { "type": "string" },
                    "paper_id": { "type": "string" },
                    "config_json": { "type": "string" },
                    "result_path": { "type": "string" },
                    "git_commit": { "type": "string" },
                    "syncthing_folder_id": { "type": "string" },
                    "status": { "type": "string", "default": "running" },
                    "tag_repo": { "type": "boolean", "default": false, "description": "Tag the associated repo with experiment-active" }
                },
                "required": ["id"]
            }
        })
    }
    async fn invoke(&self, args: serde_json::Value) -> anyhow::Result<serde_json::Value> {
        let id = args.get("id").and_then(|v| v.as_str()).context("id required")?.to_string();
        let repo_id = args.get("repo_id").and_then(|v| v.as_str()).map(String::from);
        let tag_repo = args.get("tag_repo").and_then(|v| v.as_bool()).unwrap_or(false);
        let exp = crate::registry::ExperimentEntry {
            id,
            repo_id: repo_id.clone(),
            paper_id: args.get("paper_id").and_then(|v| v.as_str()).map(String::from),
            config_json: args.get("config_json").and_then(|v| v.as_str()).map(String::from),
            result_path: args.get("result_path").and_then(|v| v.as_str()).map(String::from),
            git_commit: args.get("git_commit").and_then(|v| v.as_str()).map(String::from),
            syncthing_folder_id: args.get("syncthing_folder_id").and_then(|v| v.as_str()).map(String::from),
            status: args.get("status").and_then(|v| v.as_str()).unwrap_or("running").to_string(),
            timestamp: chrono::Utc::now(),
        };
        tokio::task::spawn_blocking(move || {
            let mut conn = crate::registry::WorkspaceRegistry::init_db()?;
            crate::registry::WorkspaceRegistry::save_experiment(&conn, &exp)?;
            if tag_repo {
                if let Some(ref rid) = repo_id {
                    let tx = conn.transaction()?;
                    tx.execute("DELETE FROM repo_tags WHERE repo_id = ?1 AND tag = 'experiment-active'", [rid])?;
                    tx.execute("INSERT OR REPLACE INTO repo_tags (repo_id, tag) VALUES (?1, 'experiment-active')", [rid])?;
                    tx.commit()?;
                }
            }
            Ok::<_, anyhow::Error>(serde_json::json!({ "success": true }))
        })
        .await
        .map_err(|e| anyhow::anyhow!("spawn_blocking failed: {}", e))?
    }
}

#[derive(Clone)]
pub struct DevkitGithubInfoTool;

impl McpTool for DevkitGithubInfoTool {
    fn name(&self) -> &'static str { "devkit_github_info" }
    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "description": "Fetch live repository metadata from GitHub API",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "repo_id": { "type": "string", "description": "Registered repository ID in devbase" },
                    "write_summary": { "type": "boolean", "description": "Write GitHub description into repo summary", "default": false }
                },
                "required": ["repo_id"]
            }
        })
    }
    async fn invoke(&self, args: serde_json::Value) -> anyhow::Result<serde_json::Value> {
        let repo_id = args.get("repo_id").and_then(|v| v.as_str()).context("repo_id required")?.to_string();
        let write_summary = args.get("write_summary").and_then(|v| v.as_bool()).unwrap_or(false);

        let upstream_url = tokio::task::spawn_blocking({
            let repo_id = repo_id.clone();
            move || -> anyhow::Result<Option<String>> {
                let conn = crate::registry::WorkspaceRegistry::init_db()?;
                let mut stmt = conn.prepare("SELECT upstream_url FROM repo_remotes WHERE repo_id = ?1 AND remote_name = 'origin'")?;
                let url: Option<String> = stmt.query_row([&repo_id], |row| row.get(0)).optional()?;
                Ok(url)
            }
        }).await.map_err(|e| anyhow::anyhow!("spawn_blocking failed: {}", e))??;

        let upstream_url = upstream_url.context("No origin remote found for repo")?;
        let (owner, repo_name) = parse_github_repo(&upstream_url).context("Failed to parse GitHub owner/repo from upstream_url")?;

        let config = crate::config::Config::load()?;
        let client = reqwest::Client::new();
        let mut req = client.get(format!("https://api.github.com/repos/{}/{}", owner, repo_name))
            .header("User-Agent", "devbase/0.1.0");
        if let Some(token) = config.github.token.as_deref() {
            req = req.header("Authorization", format!("Bearer {}", token));
        }
        let resp = req.send().await?;
        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Ok(serde_json::json!({ "success": false, "error": format!("GitHub API error {}: {}", status, text) }));
        }
        let data: serde_json::Value = resp.json().await?;

        let stars = data.get("stargazers_count").and_then(|v| v.as_i64());
        let forks = data.get("forks_count").and_then(|v| v.as_i64());
        let description = data.get("description").and_then(|v| v.as_str()).map(String::from);
        let language = data.get("language").and_then(|v| v.as_str()).map(String::from);
        let open_issues = data.get("open_issues_count").and_then(|v| v.as_i64());
        let updated_at = data.get("updated_at").and_then(|v| v.as_str()).map(String::from);
        let html_url = data.get("html_url").and_then(|v| v.as_str()).map(String::from);

        if write_summary {
            if let Some(ref desc) = description {
                let repo_id2 = repo_id.clone();
                let desc2 = desc.clone();
                tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
                    let conn = crate::registry::WorkspaceRegistry::init_db()?;
                    crate::registry::WorkspaceRegistry::save_summary(&conn, &repo_id2, &desc2, "")?;
                    Ok(())
                }).await.map_err(|e| anyhow::anyhow!("spawn_blocking failed: {}", e))??;
            }
        }

        Ok(serde_json::json!({
            "success": true,
            "owner": owner,
            "repo": repo_name,
            "stars": stars,
            "forks": forks,
            "description": description,
            "language": language,
            "open_issues": open_issues,
            "updated_at": updated_at,
            "html_url": html_url,
            "raw": data
        }))
    }
}

fn parse_github_repo(url: &str) -> Option<(String, String)> {
    let url = url.trim_end_matches(".git");
    if let Some(rest) = url.strip_prefix("https://github.com/") {
        let parts: Vec<&str> = rest.split('/').collect();
        if parts.len() >= 2 && !parts[0].is_empty() && !parts[1].is_empty() {
            return Some((parts[0].to_string(), parts[1].to_string()));
        }
    }
    if let Some(rest) = url.strip_prefix("http://github.com/") {
        let parts: Vec<&str> = rest.split('/').collect();
        if parts.len() >= 2 && !parts[0].is_empty() && !parts[1].is_empty() {
            return Some((parts[0].to_string(), parts[1].to_string()));
        }
    }
    if let Some(rest) = url.strip_prefix("git@github.com:") {
        let parts: Vec<&str> = rest.split('/').collect();
        if parts.len() >= 2 && !parts[0].is_empty() && !parts[1].is_empty() {
            return Some((parts[0].to_string(), parts[1].to_string()));
        }
    }
    None
}

#[derive(Clone)]
pub struct DevkitQueryTool;

impl McpTool for DevkitQueryTool {
    fn name(&self) -> &'static str {
        "devkit_query"
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "description": "Query the knowledge base",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "expression": {
                        "type": "string",
                        "description": "Query expression"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of results",
                        "default": 50
                    }
                },
                "required": ["expression"]
            }
        })
    }

    async fn invoke(&self, args: serde_json::Value) -> anyhow::Result<serde_json::Value> {
        let expression = args
            .get("expression")
            .and_then(|v| v.as_str())
            .context("Missing required argument: expression")?;
        let expression = expression.to_string();
        tokio::task::spawn_blocking(move || {
            let rt = tokio::runtime::Handle::current();
            let config = crate::config::Config::load()?;
            rt.block_on(crate::query::run_json(&expression, 0, 1, &config))
        })
        .await
        .map_err(|e| anyhow::anyhow!("spawn_blocking failed: {}", e))?
    }

}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_initialize() {
        let server = build_server();
        let req = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize"
        });
        let resp = server.handle_request(req).await.unwrap();
        assert_eq!(resp.get("jsonrpc").unwrap(), "2.0");
        let result = resp.get("result").unwrap();
        assert_eq!(result.get("protocolVersion").unwrap(), "2024-11-05");
        assert_eq!(result.get("serverInfo").unwrap().get("name").unwrap(), "devbase");
        assert!(result.get("capabilities").unwrap().get("tools").is_some());
    }

    #[tokio::test]
    async fn test_tools_list() {
        let server = build_server();
        let req = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/list"
        });
        let resp = server.handle_request(req).await.unwrap();
        let tools = resp.get("result").unwrap().get("tools").unwrap().as_array().unwrap();
        assert_eq!(tools.len(), 10);
        let names: Vec<&str> = tools.iter().map(|t| t.get("name").unwrap().as_str().unwrap()).collect();
        assert!(names.contains(&"devkit_scan"));
        assert!(names.contains(&"devkit_health"));
        assert!(names.contains(&"devkit_sync"));
        assert!(names.contains(&"devkit_query"));
        assert!(names.contains(&"devkit_index"));
        assert!(names.contains(&"devkit_note"));
        assert!(names.contains(&"devkit_digest"));
        assert!(names.contains(&"devkit_paper_index"));
        assert!(names.contains(&"devkit_experiment_log"));
        assert!(names.contains(&"devkit_github_info"));
        for tool in tools {
            assert!(tool.get("name").is_some());
            assert!(tool.get("description").is_some());
            assert!(tool.get("inputSchema").is_some());
        }
    }

    #[tokio::test]
    async fn test_tools_call_devkit_health() {
        let server = build_server();
        let req = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {
                "name": "devkit_health",
                "arguments": { "detail": false }
            }
        });
        let resp = server.handle_request(req).await.unwrap();
        let result = resp.get("result").unwrap();
        let content = result.get("content").unwrap().as_array().unwrap();
        let text = content[0].get("text").unwrap().as_str().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
        assert_eq!(parsed.get("success").unwrap(), true);
        let summary = parsed.get("summary").unwrap();
        assert!(summary.get("total_repos").unwrap().as_i64().unwrap() >= 0);
    }

    #[tokio::test]
    async fn test_tools_call_devkit_query() {
        let server = build_server();
        let req = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 4,
            "method": "tools/call",
            "params": {
                "name": "devkit_query",
                "arguments": { "expression": "lang:rust" }
            }
        });
        let resp = server.handle_request(req).await.unwrap();
        let result = resp.get("result").unwrap();
        let content = result.get("content").unwrap().as_array().unwrap();
        let text = content[0].get("text").unwrap().as_str().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
        assert_eq!(parsed.get("success").unwrap(), true);
        assert!(parsed.get("count").unwrap().as_i64().unwrap() >= 0);
    }

    #[tokio::test]
    async fn test_tools_call_unknown_tool() {
        let server = build_server();
        let req = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 5,
            "method": "tools/call",
            "params": {
                "name": "unknown_tool",
                "arguments": {}
            }
        });
        let resp = server.handle_request(req).await.unwrap();
        assert!(resp.get("error").is_some());
        let error = resp.get("error").unwrap();
        assert_eq!(error.get("code").unwrap().as_i64().unwrap(), -32602);
    }

    #[tokio::test]
    async fn test_unknown_method() {
        let server = build_server();
        let req = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 6,
            "method": "unknown/method"
        });
        let resp = server.handle_request(req).await.unwrap();
        assert!(resp.get("error").is_some());
        let error = resp.get("error").unwrap();
        assert_eq!(error.get("code").unwrap().as_i64().unwrap(), -32601);
    }

    #[tokio::test]
    async fn test_stdio_content_length_format() {
        let body = serde_json::json!({ "jsonrpc": "2.0", "id": 1, "result": {} });
        let msg = format_mcp_message(&body);
        assert!(msg.starts_with("Content-Length: "));
        let parts: Vec<&str> = msg.split("\r\n\r\n").collect();
        assert_eq!(parts.len(), 2);
        let body_part = parts[1];
        assert!(body_part.ends_with("\n"));
        let parsed: serde_json::Value = serde_json::from_str(body_part.trim_end()).unwrap();
        assert_eq!(parsed, body);
    }

}
