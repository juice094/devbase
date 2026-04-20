use anyhow::Context;
use rusqlite::OptionalExtension;
use std::collections::HashMap;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};

#[allow(async_fn_in_trait)]
pub trait McpTool: Send + Sync + Clone {
    fn name(&self) -> &'static str;
    fn schema(&self) -> serde_json::Value;
    async fn invoke(&self, args: serde_json::Value) -> anyhow::Result<serde_json::Value>;
}

#[derive(Clone)]
pub enum McpToolEnum {
    Scan(DevkitScanTool),
    Health(DevkitHealthTool),
    Sync(DevkitSyncTool),
    Query(DevkitQueryTool),
    QueryRepos(DevkitQueryReposTool),
    Index(DevkitIndexTool),
    Note(DevkitNoteTool),
    Digest(DevkitDigestTool),
    Paper(DevkitPaperIndexTool),
    Experiment(DevkitExperimentLogTool),
    GithubInfo(DevkitGithubInfoTool),
    CodeMetrics(DevkitCodeMetricsTool),
    ModuleGraph(DevkitModuleGraphTool),
    NaturalLanguageQuery(DevkitNaturalLanguageQueryTool),
}

impl McpTool for McpToolEnum {
    fn name(&self) -> &'static str {
        match self {
            McpToolEnum::Scan(t) => t.name(),
            McpToolEnum::Health(t) => t.name(),
            McpToolEnum::Sync(t) => t.name(),
            McpToolEnum::Query(t) => t.name(),
            McpToolEnum::QueryRepos(t) => t.name(),
            McpToolEnum::Index(t) => t.name(),
            McpToolEnum::Note(t) => t.name(),
            McpToolEnum::Digest(t) => t.name(),
            McpToolEnum::Paper(t) => t.name(),
            McpToolEnum::Experiment(t) => t.name(),
            McpToolEnum::GithubInfo(t) => t.name(),
            McpToolEnum::CodeMetrics(t) => t.name(),
            McpToolEnum::ModuleGraph(t) => t.name(),
            McpToolEnum::NaturalLanguageQuery(t) => t.name(),
        }
    }

    fn schema(&self) -> serde_json::Value {
        match self {
            McpToolEnum::Scan(t) => t.schema(),
            McpToolEnum::Health(t) => t.schema(),
            McpToolEnum::Sync(t) => t.schema(),
            McpToolEnum::Query(t) => t.schema(),
            McpToolEnum::QueryRepos(t) => t.schema(),
            McpToolEnum::Index(t) => t.schema(),
            McpToolEnum::Note(t) => t.schema(),
            McpToolEnum::Digest(t) => t.schema(),
            McpToolEnum::Paper(t) => t.schema(),
            McpToolEnum::Experiment(t) => t.schema(),
            McpToolEnum::GithubInfo(t) => t.schema(),
            McpToolEnum::CodeMetrics(t) => t.schema(),
            McpToolEnum::ModuleGraph(t) => t.schema(),
            McpToolEnum::NaturalLanguageQuery(t) => t.schema(),
        }
    }

    async fn invoke(&self, args: serde_json::Value) -> anyhow::Result<serde_json::Value> {
        match self {
            McpToolEnum::Scan(t) => t.invoke(args).await,
            McpToolEnum::Health(t) => t.invoke(args).await,
            McpToolEnum::Sync(t) => t.invoke(args).await,
            McpToolEnum::Query(t) => t.invoke(args).await,
            McpToolEnum::QueryRepos(t) => t.invoke(args).await,
            McpToolEnum::Index(t) => t.invoke(args).await,
            McpToolEnum::Note(t) => t.invoke(args).await,
            McpToolEnum::Digest(t) => t.invoke(args).await,
            McpToolEnum::Paper(t) => t.invoke(args).await,
            McpToolEnum::Experiment(t) => t.invoke(args).await,
            McpToolEnum::GithubInfo(t) => t.invoke(args).await,
            McpToolEnum::CodeMetrics(t) => t.invoke(args).await,
            McpToolEnum::ModuleGraph(t) => t.invoke(args).await,
            McpToolEnum::NaturalLanguageQuery(t) => t.invoke(args).await,
        }
    }
}

pub struct McpServer {
    tools: HashMap<String, McpToolEnum>,
}

impl Default for McpServer {
    fn default() -> Self {
        Self::new()
    }
}

impl McpServer {
    pub fn new() -> Self {
        Self { tools: HashMap::new() }
    }

    pub fn register_tool(mut self, tool: McpToolEnum) -> Self {
        self.tools.insert(tool.name().to_string(), tool);
        self
    }

    pub async fn handle_request(
        &self,
        req: serde_json::Value,
    ) -> anyhow::Result<serde_json::Value> {
        let id = req.get("id").cloned().unwrap_or(serde_json::Value::Null);
        let method = req.get("method").and_then(|v| v.as_str()).unwrap_or("");

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
                let name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
                let args = params.get("arguments").cloned().unwrap_or(serde_json::Value::Null);

                match self.tools.get(name) {
                    Some(tool) => match tool.invoke(args).await {
                        Ok(result) => {
                            let text = result.to_string();
                            let is_error = !result
                                .get("success")
                                .and_then(|v: &serde_json::Value| v.as_bool())
                                .unwrap_or(false);
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
                            let payload =
                                serde_json::json!({ "success": false, "error": e.to_string() });
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
        .register_tool(McpToolEnum::QueryRepos(DevkitQueryReposTool))
        .register_tool(McpToolEnum::Index(DevkitIndexTool))
        .register_tool(McpToolEnum::Note(DevkitNoteTool))
        .register_tool(McpToolEnum::Digest(DevkitDigestTool))
        .register_tool(McpToolEnum::Paper(DevkitPaperIndexTool))
        .register_tool(McpToolEnum::Experiment(DevkitExperimentLogTool))
        .register_tool(McpToolEnum::GithubInfo(DevkitGithubInfoTool))
        .register_tool(McpToolEnum::CodeMetrics(DevkitCodeMetricsTool))
        .register_tool(McpToolEnum::ModuleGraph(DevkitModuleGraphTool))
        .register_tool(McpToolEnum::NaturalLanguageQuery(DevkitNaturalLanguageQueryTool))
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
            line.strip_prefix("Content-Length: ").and_then(|v| v.parse::<usize>().ok())
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
            "description": "Syncs registered repos according to their inferred SyncPolicy (Mirror/Conservative/Rebase/Merge based on tags). dry_run=true by default for safety.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "dry_run": {
                        "type": "boolean",
                        "description": "Preview mode: do not modify any files",
                        "default": true
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
        let count =
            tokio::task::spawn_blocking(move || crate::knowledge_engine::run_index(&path_owned))
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
    fn name(&self) -> &'static str {
        "devkit_digest"
    }
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
    fn name(&self) -> &'static str {
        "devkit_paper_index"
    }
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
        let tags: Vec<String> = tags_str
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        let path = if path.starts_with("~/") {
            dirs::home_dir()
                .map(|d| d.join(&path[2..]))
                .unwrap_or_else(|| std::path::PathBuf::from(path))
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
                        let title =
                            if id.chars().filter(|c| c.is_numeric() || *c == '.').count() > 5 {
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
    fn name(&self) -> &'static str {
        "devkit_experiment_log"
    }
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
            syncthing_folder_id: args
                .get("syncthing_folder_id")
                .and_then(|v| v.as_str())
                .map(String::from),
            status: args.get("status").and_then(|v| v.as_str()).unwrap_or("running").to_string(),
            timestamp: chrono::Utc::now(),
        };
        tokio::task::spawn_blocking(move || {
            let mut conn = crate::registry::WorkspaceRegistry::init_db()?;
            crate::registry::WorkspaceRegistry::save_experiment(&conn, &exp)?;
            if tag_repo
                && let Some(ref rid) = repo_id {
                    let tx = conn.transaction()?;
                    tx.execute("DELETE FROM repo_tags WHERE repo_id = ?1 AND tag = 'experiment-active'", [rid])?;
                    tx.execute("INSERT OR REPLACE INTO repo_tags (repo_id, tag) VALUES (?1, 'experiment-active')", [rid])?;
                    tx.commit()?;
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
    fn name(&self) -> &'static str {
        "devkit_github_info"
    }
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
        let repo_id = args
            .get("repo_id")
            .and_then(|v| v.as_str())
            .context("repo_id required")?
            .to_string();
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
        let (owner, repo_name) = parse_github_repo(&upstream_url)
            .context("Failed to parse GitHub owner/repo from upstream_url")?;

        let config = crate::config::Config::load()?;
        let client = reqwest::Client::new();
        let mut req = client
            .get(format!("https://api.github.com/repos/{}/{}", owner, repo_name))
            .header("User-Agent", "devbase/0.1.0");
        if let Some(token) = config.github.token.as_deref() {
            req = req.header("Authorization", format!("Bearer {}", token));
        }
        let resp = req.send().await?;
        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Ok(
                serde_json::json!({ "success": false, "error": format!("GitHub API error {}: {}", status, text) }),
            );
        }
        let data: serde_json::Value = resp.json().await?;

        let stars = data.get("stargazers_count").and_then(|v| v.as_i64());
        let forks = data.get("forks_count").and_then(|v| v.as_i64());
        let description = data.get("description").and_then(|v| v.as_str()).map(String::from);
        let language = data.get("language").and_then(|v| v.as_str()).map(String::from);
        let open_issues = data.get("open_issues_count").and_then(|v| v.as_i64());
        let updated_at = data.get("updated_at").and_then(|v| v.as_str()).map(String::from);
        let html_url = data.get("html_url").and_then(|v| v.as_str()).map(String::from);

        if write_summary && let Some(ref desc) = description {
            let repo_id2 = repo_id.clone();
            let desc2 = desc.clone();
            tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
                let conn = crate::registry::WorkspaceRegistry::init_db()?;
                crate::registry::WorkspaceRegistry::save_summary(&conn, &repo_id2, &desc2, "")?;
                Ok(())
            })
            .await
            .map_err(|e| anyhow::anyhow!("spawn_blocking failed: {}", e))??;
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

#[derive(Clone)]
pub struct DevkitCodeMetricsTool;

impl McpTool for DevkitCodeMetricsTool {
    fn name(&self) -> &'static str {
        "devkit_code_metrics"
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "description": "Get code metrics (lines, files, languages) for registered repositories",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "repo_id": { "type": "string", "description": "Specific repo ID; if omitted, returns all repos", "default": "" }
                }
            }
        })
    }

    async fn invoke(&self, args: serde_json::Value) -> anyhow::Result<serde_json::Value> {
        let repo_id = args.get("repo_id").and_then(|v| v.as_str()).unwrap_or("").to_string();

        tokio::task::spawn_blocking(move || {
            let conn = crate::registry::WorkspaceRegistry::init_db()?;
            if repo_id.is_empty() {
                let metrics = crate::registry::WorkspaceRegistry::list_code_metrics(&conn)?;
                let repos: Vec<serde_json::Value> = metrics.into_iter().map(|(id, m)| {
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
                }).collect();
                Ok::<_, anyhow::Error>(serde_json::json!({ "success": true, "count": repos.len(), "repos": repos }))
            } else {
                match crate::registry::WorkspaceRegistry::get_code_metrics(&conn, &repo_id)? {
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
                    None => Ok(serde_json::json!({ "success": false, "error": "No metrics found for repo" })),
                }
            }
        })
        .await
        .map_err(|e| anyhow::anyhow!("spawn_blocking failed: {}", e))?
    }
}

#[derive(Clone)]
pub struct DevkitModuleGraphTool;

impl McpTool for DevkitModuleGraphTool {
    fn name(&self) -> &'static str {
        "devkit_module_graph"
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "description": "Get module/target structure for Rust repositories (from cargo metadata)",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "repo_id": { "type": "string", "description": "Repository ID", "default": "" }
                }
            }
        })
    }

    async fn invoke(&self, args: serde_json::Value) -> anyhow::Result<serde_json::Value> {
        let repo_id = args.get("repo_id").and_then(|v| v.as_str()).unwrap_or("").to_string();

        tokio::task::spawn_blocking(move || {
            let conn = crate::registry::WorkspaceRegistry::init_db()?;
            if repo_id.is_empty() {
                let repos = crate::registry::WorkspaceRegistry::list_repos(&conn)?;
                let mut all_modules = vec![];
                for repo in repos {
                    if repo.language.as_deref() == Some("Rust") {
                        let modules = crate::registry::WorkspaceRegistry::list_modules(&conn, &repo.id)?;
                        if !modules.is_empty() {
                            all_modules.push(serde_json::json!({
                                "repo_id": repo.id,
                                "modules": modules.iter().map(|(n, t, p)| serde_json::json!({
                                    "name": n, "type": t, "path": p
                                })).collect::<Vec<_>>()
                            }));
                        }
                    }
                }
                Ok::<_, anyhow::Error>(serde_json::json!({ "success": true, "count": all_modules.len(), "repos": all_modules }))
            } else {
                let modules = crate::registry::WorkspaceRegistry::list_modules(&conn, &repo_id)?;
                Ok(serde_json::json!({
                    "success": true,
                    "repo_id": repo_id,
                    "modules": modules.iter().map(|(n, t, p)| serde_json::json!({
                        "name": n, "type": t, "path": p
                    })).collect::<Vec<_>>()
                }))
            }
        })
        .await
        .map_err(|e| anyhow::anyhow!("spawn_blocking failed: {}", e))?
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
pub struct DevkitQueryReposTool;

impl McpTool for DevkitQueryReposTool {
    fn name(&self) -> &'static str {
        "devkit_query_repos"
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "description": "Query registered repositories with filters. Returns structured metadata including Git status, tags, language, and health.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "language": { "type": "string", "description": "Filter by programming language (e.g., 'rust', 'go', 'python')", "default": "" },
                    "tag": { "type": "string", "description": "Filter by tag", "default": "" },
                    "status": { "type": "string", "enum": ["dirty", "ahead", "behind", "diverged", "up_to_date", ""], "description": "Filter by Git status", "default": "" },
                    "limit": { "type": "integer", "description": "Max results", "default": 50 }
                }
            }
        })
    }

    async fn invoke(&self, args: serde_json::Value) -> anyhow::Result<serde_json::Value> {
        let language = args.get("language").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let tag = args.get("tag").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let status = args.get("status").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let limit = args.get("limit").and_then(|v| v.as_i64()).unwrap_or(50) as usize;

        tokio::task::spawn_blocking(move || {
            let conn = crate::registry::WorkspaceRegistry::init_db()?;
            let repos = crate::registry::WorkspaceRegistry::list_repos(&conn)?;

            let mut results = Vec::new();
            for repo in repos {
                // Filter by language (case-insensitive)
                if !language.is_empty() {
                    match &repo.language {
                        Some(lang) if lang.eq_ignore_ascii_case(&language) => {}
                        _ => continue,
                    }
                }

                // Filter by tag (case-insensitive)
                if !tag.is_empty() && !repo.tags.iter().any(|t| t.eq_ignore_ascii_case(&tag)) {
                    continue;
                }

                // Gather status
                let (ahead, behind, dirty) = if repo.workspace_type == "git" {
                    let (st, ah, bh) =
                        match crate::registry::WorkspaceRegistry::get_health(&conn, &repo.id)? {
                            Some(health) => (health.status.clone(), health.ahead, health.behind),
                            None => {
                                let path = repo.local_path.to_string_lossy();
                                let primary = repo.primary_remote();
                                let upstream_url = primary.and_then(|r| r.upstream_url.as_deref());
                                let default_branch =
                                    primary.and_then(|r| r.default_branch.as_deref());
                                crate::health::analyze_repo(&path, upstream_url, default_branch)
                            }
                        };
                    let dirty = st == "dirty" || st == "changed";
                    (ah, bh, dirty)
                } else {
                    let dirty = match crate::health::compute_workspace_hash(&repo.local_path) {
                        Ok(current_hash) => {
                            match crate::registry::WorkspaceRegistry::get_latest_workspace_snapshot(
                                &conn, &repo.id,
                            )? {
                                Some(prev) => prev.file_hash != current_hash,
                                None => true,
                            }
                        }
                        Err(_) => false,
                    };
                    (0, 0, dirty)
                };

                // Filter by conceptual status
                if !status.is_empty() {
                    let matches = match status.as_str() {
                        "dirty" => dirty,
                        "ahead" => !dirty && ahead > 0 && behind == 0,
                        "behind" => !dirty && behind > 0 && ahead == 0,
                        "diverged" => !dirty && ahead > 0 && behind > 0,
                        "up_to_date" => !dirty && ahead == 0 && behind == 0,
                        _ => true,
                    };
                    if !matches {
                        continue;
                    }
                }

                results.push(serde_json::json!({
                    "id": repo.id,
                    "path": repo.local_path,
                    "language": repo.language,
                    "tags": repo.tags,
                    "status": {
                        "dirty": dirty,
                        "ahead": ahead,
                        "behind": behind,
                    },
                    "stars": repo.stars,
                }));

                if limit > 0 && results.len() >= limit {
                    break;
                }
            }

            Ok::<_, anyhow::Error>(serde_json::json!({
                "success": true,
                "count": results.len(),
                "repos": results,
            }))
        })
        .await
        .map_err(|e| anyhow::anyhow!("spawn_blocking failed: {}", e))?
    }
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

#[derive(Clone)]
pub struct DevkitNaturalLanguageQueryTool;

impl McpTool for DevkitNaturalLanguageQueryTool {
    fn name(&self) -> &'static str {
        "devkit_natural_language_query"
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "description": "Query repositories using natural language (e.g., 'show dirty rust repos', 'repos with more than 100 stars')",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Natural language query" }
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
        let query = query.to_string();

        tokio::task::spawn_blocking(move || {
            let conn = crate::registry::WorkspaceRegistry::init_db()?;
            let repos = crate::registry::WorkspaceRegistry::list_repos(&conn)?;
            let filtered = nl_filter_repos(&query, &repos, &conn)?;

            let results: Vec<serde_json::Value> = filtered
                .into_iter()
                .map(|repo| {
                    serde_json::json!({
                        "id": repo.id,
                        "path": repo.local_path,
                        "language": repo.language,
                        "tags": repo.tags,
                        "stars": repo.stars,
                    })
                })
                .collect();

            Ok::<_, anyhow::Error>(serde_json::json!({
                "success": true,
                "count": results.len(),
                "query": query,
                "repos": results,
            }))
        })
        .await
        .map_err(|e| anyhow::anyhow!("spawn_blocking failed: {}", e))?
    }
}

fn nl_filter_repos(
    query: &str,
    repos: &[crate::registry::RepoEntry],
    conn: &rusqlite::Connection,
) -> anyhow::Result<Vec<crate::registry::RepoEntry>> {
    let q = query.to_lowercase();
    let stars_cond = parse_stars_condition(&q);
    let explicit_tag = extract_tag_from_query(&q);

    let mut results = Vec::new();
    for repo in repos {
        // Language filter
        let lang_keywords = [
            ("rust", "rust"),
            ("go", "go"),
            ("golang", "go"),
            ("python", "python"),
            ("typescript", "typescript"),
            ("ts", "typescript"),
            ("javascript", "javascript"),
            ("js", "javascript"),
            ("cpp", "c++"),
            ("c++", "c++"),
            ("java", "java"),
        ];
        let mut lang_matched = true;
        for &(kw, expected) in &lang_keywords {
            if q.contains(kw) && repo.language.as_deref() != Some(expected) {
                lang_matched = false;
                break;
            }
        }
        if !lang_matched {
            continue;
        }

        // Tag filter
        if let Some(ref tag) = explicit_tag
            && !repo.tags.iter().any(|t| t.eq_ignore_ascii_case(tag))
        {
            continue;
        }

        // Stars filter
        if let Some((op, val)) = stars_cond {
            let stars = repo.stars.unwrap_or(0);
            match op {
                '>' => {
                    if stars <= val {
                        continue;
                    }
                }
                '<' => {
                    if stars >= val {
                        continue;
                    }
                }
                '=' => {
                    if stars != val {
                        continue;
                    }
                }
                _ => {}
            }
        }

        // Status filters (need health data)
        if q.contains("dirty")
            || q.contains("behind")
            || q.contains("ahead")
            || q.contains("diverged")
            || q.contains("up to date")
        {
            let (st, ah, bh) = match crate::registry::WorkspaceRegistry::get_health(conn, &repo.id)?
            {
                Some(h) => (h.status.clone(), h.ahead, h.behind),
                None => {
                    let path = repo.local_path.to_string_lossy();
                    let primary = repo.primary_remote();
                    let upstream_url = primary.and_then(|r| r.upstream_url.as_deref());
                    let default_branch = primary.and_then(|r| r.default_branch.as_deref());
                    crate::health::analyze_repo(&path, upstream_url, default_branch)
                }
            };
            let dirty = st == "dirty" || st == "changed";

            if q.contains("dirty") && !dirty {
                continue;
            }
            if q.contains("behind") && !q.contains("ahead") && bh == 0 {
                continue;
            }
            if q.contains("ahead") && !q.contains("behind") && ah == 0 {
                continue;
            }
            if q.contains("diverged") && (ah == 0 || bh == 0) {
                continue;
            }
            if (q.contains("up to date") || q.contains("uptodate")) && (dirty || ah > 0 || bh > 0) {
                continue;
            }
        }

        results.push(repo.clone());
    }

    Ok(results)
}

fn parse_stars_condition(query: &str) -> Option<(char, u64)> {
    let lower = query.to_lowercase();
    if !lower.contains("stars") && !lower.contains("star") {
        return None;
    }
    let digits: String = lower
        .chars()
        .skip_while(|c| !c.is_ascii_digit())
        .take_while(|c| c.is_ascii_digit())
        .collect();
    let num = digits.parse::<u64>().ok()?;

    if lower.contains(">") || lower.contains("more than") || lower.contains("over") {
        Some(('>', num))
    } else if lower.contains("<") || lower.contains("less than") || lower.contains("under") {
        Some(('<', num))
    } else {
        Some(('=', num))
    }
}

fn extract_tag_from_query(q: &str) -> Option<String> {
    if let Some(pos) = q.find("tag ") {
        let rest = &q[pos + 4..];
        rest.split_whitespace().next().map(|s| s.to_string())
    } else if let Some(pos) = q.find("with tag ") {
        let rest = &q[pos + 9..];
        rest.split_whitespace().next().map(|s| s.to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests;
