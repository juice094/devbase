use anyhow::Context;
use std::collections::HashMap;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

pub trait McpTool: Send + Sync {
    fn name(&self) -> &'static str;
    fn schema(&self) -> serde_json::Value;
    fn invoke(&self, args: serde_json::Value) -> impl std::future::Future<Output = anyhow::Result<serde_json::Value>> + Send;
}

pub(crate) enum McpToolEnum {
    Scan(DevkitScanTool),
    Health(DevkitHealthTool),
    Sync(DevkitSyncTool),
    Query(DevkitQueryTool),
    Index(DevkitIndexTool),
    Note(DevkitNoteTool),
    Digest(DevkitDigestTool),
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

    async fn handle_request(&self, req: serde_json::Value) -> anyhow::Result<serde_json::Value> {
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
}

pub fn format_mcp_message(body: &serde_json::Value) -> String {
    let body_str = body.to_string();
    format!("Content-Length: {}\r\n\r\n{}\n", body_str.len(), body_str)
}

pub async fn run_stdio() -> anyhow::Result<()> {
    let server = build_server();
    let stdin = tokio::io::stdin();
    let mut stdout = tokio::io::stdout();
    let reader = BufReader::new(stdin);
    let mut lines = reader.lines();

    while let Some(line) = lines.next_line().await? {
        let line: &str = line.trim();
        if line.is_empty() {
            continue;
        }
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
    }

    Ok(())
}

// ------------------------------------------------------------------
// Tools
// ------------------------------------------------------------------

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
        crate::health::run_json(detail).await
    }
}

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
        let strategy = args
            .get("strategy")
            .and_then(|v| v.as_str())
            .unwrap_or("fetch-only");
        let filter_tags = args.get("filter_tags").and_then(|v| v.as_str());
        crate::sync::run_json(dry_run, strategy, filter_tags).await
    }
}

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
            let text = crate::digest::generate_daily_digest(&conn)?;
            Ok::<_, anyhow::Error>(serde_json::json!({ "success": true, "digest": text }))
        })
        .await
        .map_err(|e| anyhow::anyhow!("spawn_blocking failed: {}", e))?
    }
}

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
            rt.block_on(crate::query::run_json(&expression))
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
        assert_eq!(tools.len(), 7);
        let names: Vec<&str> = tools.iter().map(|t| t.get("name").unwrap().as_str().unwrap()).collect();
        assert!(names.contains(&"devkit_scan"));
        assert!(names.contains(&"devkit_health"));
        assert!(names.contains(&"devkit_sync"));
        assert!(names.contains(&"devkit_query"));
        assert!(names.contains(&"devkit_index"));
        assert!(names.contains(&"devkit_note"));
        assert!(names.contains(&"devkit_digest"));
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
