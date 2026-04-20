use std::collections::HashMap;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};

pub use tools::*;

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

#[cfg(test)]
pub mod tests;
pub mod tools;
