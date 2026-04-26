use std::collections::{HashMap, HashSet};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};

pub use tools::*;

/// Phase of a streaming tool invocation.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StreamPhase {
    /// Progress update (e.g., "Indexing repo 3/10...").
    Progress,
    /// Intermediate partial result.
    Partial,
    /// Final result — stream ends after this.
    Done,
    /// Error occurred — stream ends after this.
    Error,
}

/// A single event in a streaming tool invocation.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ToolStreamEvent {
    pub phase: StreamPhase,
    pub payload: serde_json::Value,
}

#[allow(async_fn_in_trait)]
pub trait McpTool: Send + Sync + Clone {
    fn name(&self) -> &'static str;
    fn schema(&self) -> serde_json::Value;
    async fn invoke(&self, args: serde_json::Value) -> anyhow::Result<serde_json::Value>;

    /// Optional streaming interface for long-running operations.
    ///
    /// Default implementation delegates to `invoke` and emits a single `Done` event.
    /// Override this for tools that support progressive output (e.g., indexing,
    /// syncing large batches, or long-running analysis).
    async fn invoke_stream(&self, args: serde_json::Value) -> anyhow::Result<Vec<ToolStreamEvent>> {
        let result = self.invoke(args).await?;
        Ok(vec![ToolStreamEvent {
            phase: StreamPhase::Done,
            payload: result,
        }])
    }
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
    VaultSearch(DevkitVaultSearchTool),
    VaultRead(DevkitVaultReadTool),
    VaultWrite(DevkitVaultWriteTool),
    VaultBacklinks(DevkitVaultBacklinksTool),
    ProjectContext(DevkitProjectContextTool),
    CodeSymbols(DevkitCodeSymbolsTool),
    DependencyGraph(DevkitDependencyGraphTool),
    CallGraph(DevkitCallGraphTool),
    DeadCode(DevkitDeadCodeTool),
    SemanticSearch(DevkitSemanticSearchTool),
    ArxivFetch(DevkitArxivFetchTool),
    EmbeddingStore(DevkitEmbeddingStoreTool),
    EmbeddingSearch(DevkitEmbeddingSearchTool),
    CrossRepoSearch(DevkitCrossRepoSearchTool),
    KnowledgeReport(DevkitKnowledgeReportTool),
    RelatedSymbols(DevkitRelatedSymbolsTool),
    HybridSearch(DevkitHybridSearchTool),
    SkillList(DevkitSkillListTool),
    SkillSearch(DevkitSkillSearchTool),
    SkillRun(DevkitSkillRunTool),
    SkillDiscover(DevkitSkillDiscoverTool),
    KnownLimitStore(DevkitKnownLimitStoreTool),
    KnownLimitList(DevkitKnownLimitListTool),
}

/// Stability tier for MCP tools.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ToolTier {
    Stable,
    Beta,
    Experimental,
}

impl std::str::FromStr for ToolTier {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "stable" => Ok(ToolTier::Stable),
            "beta" => Ok(ToolTier::Beta),
            "experimental" => Ok(ToolTier::Experimental),
            _ => Err(()),
        }
    }
}

impl McpToolEnum {
    pub fn tier(&self) -> ToolTier {
        match self {
            // Stable: battle-tested, schema frozen, unit-tested
            McpToolEnum::Health(_) => ToolTier::Stable,
            McpToolEnum::QueryRepos(_) => ToolTier::Stable,
            McpToolEnum::VaultSearch(_) => ToolTier::Stable,
            McpToolEnum::VaultRead(_) => ToolTier::Stable,
            McpToolEnum::ProjectContext(_) => ToolTier::Stable,
            // Beta: validated but schema may微调, limited edge-case tests
            McpToolEnum::Scan(_) => ToolTier::Beta,
            McpToolEnum::Sync(_) => ToolTier::Beta,
            McpToolEnum::Query(_) => ToolTier::Beta,
            McpToolEnum::Index(_) => ToolTier::Beta,
            McpToolEnum::Note(_) => ToolTier::Beta,
            McpToolEnum::VaultWrite(_) => ToolTier::Beta,
            McpToolEnum::VaultBacklinks(_) => ToolTier::Beta,
            McpToolEnum::NaturalLanguageQuery(_) => ToolTier::Beta,
            McpToolEnum::GithubInfo(_) => ToolTier::Beta,
            // Experimental: new, behavior may change, pending prod validation
            McpToolEnum::Digest(_) => ToolTier::Experimental,
            McpToolEnum::Paper(_) => ToolTier::Experimental,
            McpToolEnum::Experiment(_) => ToolTier::Experimental,
            McpToolEnum::CodeMetrics(_) => ToolTier::Experimental,
            McpToolEnum::ModuleGraph(_) => ToolTier::Experimental,
            McpToolEnum::CodeSymbols(_) => ToolTier::Beta,
            McpToolEnum::DependencyGraph(_) => ToolTier::Beta,
            McpToolEnum::CallGraph(_) => ToolTier::Experimental,
            McpToolEnum::DeadCode(_) => ToolTier::Experimental,
            McpToolEnum::SemanticSearch(_) => ToolTier::Beta,
            McpToolEnum::ArxivFetch(_) => ToolTier::Beta,
            McpToolEnum::EmbeddingStore(_) => ToolTier::Beta,
            McpToolEnum::EmbeddingSearch(_) => ToolTier::Beta,
            McpToolEnum::CrossRepoSearch(_) => ToolTier::Beta,
            McpToolEnum::KnowledgeReport(_) => ToolTier::Beta,
            McpToolEnum::RelatedSymbols(_) => ToolTier::Experimental,
            McpToolEnum::HybridSearch(_) => ToolTier::Beta,
            McpToolEnum::SkillList(_) => ToolTier::Beta,
            McpToolEnum::SkillSearch(_) => ToolTier::Beta,
            McpToolEnum::SkillRun(_) => ToolTier::Beta,
            McpToolEnum::SkillDiscover(_) => ToolTier::Beta,
            McpToolEnum::KnownLimitStore(_) => ToolTier::Beta,
            McpToolEnum::KnownLimitList(_) => ToolTier::Beta,
        }
    }
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
            McpToolEnum::VaultSearch(t) => t.name(),
            McpToolEnum::VaultRead(t) => t.name(),
            McpToolEnum::VaultWrite(t) => t.name(),
            McpToolEnum::VaultBacklinks(t) => t.name(),
            McpToolEnum::ProjectContext(t) => t.name(),
            McpToolEnum::CodeSymbols(t) => t.name(),
            McpToolEnum::DependencyGraph(t) => t.name(),
            McpToolEnum::CallGraph(t) => t.name(),
            McpToolEnum::DeadCode(t) => t.name(),
            McpToolEnum::SemanticSearch(t) => t.name(),
            McpToolEnum::ArxivFetch(t) => t.name(),
            McpToolEnum::EmbeddingStore(t) => t.name(),
            McpToolEnum::EmbeddingSearch(t) => t.name(),
            McpToolEnum::CrossRepoSearch(t) => t.name(),
            McpToolEnum::KnowledgeReport(t) => t.name(),
            McpToolEnum::RelatedSymbols(t) => t.name(),
            McpToolEnum::HybridSearch(t) => t.name(),
            McpToolEnum::SkillList(t) => t.name(),
            McpToolEnum::SkillSearch(t) => t.name(),
            McpToolEnum::SkillRun(t) => t.name(),
            McpToolEnum::SkillDiscover(t) => t.name(),
            McpToolEnum::KnownLimitStore(t) => t.name(),
            McpToolEnum::KnownLimitList(t) => t.name(),
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
            McpToolEnum::VaultSearch(t) => t.schema(),
            McpToolEnum::VaultRead(t) => t.schema(),
            McpToolEnum::VaultWrite(t) => t.schema(),
            McpToolEnum::VaultBacklinks(t) => t.schema(),
            McpToolEnum::ProjectContext(t) => t.schema(),
            McpToolEnum::CodeSymbols(t) => t.schema(),
            McpToolEnum::DependencyGraph(t) => t.schema(),
            McpToolEnum::CallGraph(t) => t.schema(),
            McpToolEnum::DeadCode(t) => t.schema(),
            McpToolEnum::SemanticSearch(t) => t.schema(),
            McpToolEnum::ArxivFetch(t) => t.schema(),
            McpToolEnum::EmbeddingStore(t) => t.schema(),
            McpToolEnum::EmbeddingSearch(t) => t.schema(),
            McpToolEnum::CrossRepoSearch(t) => t.schema(),
            McpToolEnum::KnowledgeReport(t) => t.schema(),
            McpToolEnum::RelatedSymbols(t) => t.schema(),
            McpToolEnum::HybridSearch(t) => t.schema(),
            McpToolEnum::SkillList(t) => t.schema(),
            McpToolEnum::SkillSearch(t) => t.schema(),
            McpToolEnum::SkillRun(t) => t.schema(),
            McpToolEnum::SkillDiscover(t) => t.schema(),
            McpToolEnum::KnownLimitStore(t) => t.schema(),
            McpToolEnum::KnownLimitList(t) => t.schema(),
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
            McpToolEnum::VaultSearch(t) => t.invoke(args).await,
            McpToolEnum::VaultRead(t) => t.invoke(args).await,
            McpToolEnum::VaultWrite(t) => t.invoke(args).await,
            McpToolEnum::VaultBacklinks(t) => t.invoke(args).await,
            McpToolEnum::ProjectContext(t) => t.invoke(args).await,
            McpToolEnum::CodeSymbols(t) => t.invoke(args).await,
            McpToolEnum::DependencyGraph(t) => t.invoke(args).await,
            McpToolEnum::CallGraph(t) => t.invoke(args).await,
            McpToolEnum::DeadCode(t) => t.invoke(args).await,
            McpToolEnum::SemanticSearch(t) => t.invoke(args).await,
            McpToolEnum::ArxivFetch(t) => t.invoke(args).await,
            McpToolEnum::EmbeddingStore(t) => t.invoke(args).await,
            McpToolEnum::EmbeddingSearch(t) => t.invoke(args).await,
            McpToolEnum::CrossRepoSearch(t) => t.invoke(args).await,
            McpToolEnum::KnowledgeReport(t) => t.invoke(args).await,
            McpToolEnum::RelatedSymbols(t) => t.invoke(args).await,
            McpToolEnum::HybridSearch(t) => t.invoke(args).await,
            McpToolEnum::SkillList(t) => t.invoke(args).await,
            McpToolEnum::SkillSearch(t) => t.invoke(args).await,
            McpToolEnum::SkillRun(t) => t.invoke(args).await,
            McpToolEnum::SkillDiscover(t) => t.invoke(args).await,
            McpToolEnum::KnownLimitStore(t) => t.invoke(args).await,
            McpToolEnum::KnownLimitList(t) => t.invoke(args).await,
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

    pub fn register_tool(&mut self, tool: McpToolEnum) {
        self.tools.insert(tool.name().to_string(), tool);
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

    /// Invoke a tool in streaming mode and return a sequence of events.
    ///
    /// This is used by the SSE transport to push progressive updates.
    /// If the tool does not override `invoke_stream`, the default implementation
    /// delegates to `invoke` and wraps the result as a single `Done` event.
    pub async fn handle_streaming_call(
        &self,
        name: &str,
        args: serde_json::Value,
    ) -> anyhow::Result<Vec<ToolStreamEvent>> {
        match self.tools.get(name) {
            Some(tool) => tool.invoke_stream(args).await,
            None => Err(anyhow::anyhow!("Tool '{}' not found", name)),
        }
    }
}

/// Build an MCP server with optional tier filtering.
///
/// If `tiers` is `None`, all 37 tools are registered (backward compatible).
/// If `tiers` is provided, only tools whose tier is in the set are registered.
pub fn build_server_with_tiers(tiers: Option<&HashSet<ToolTier>>) -> McpServer {
    let mut server = McpServer::new();
    let all_tools = [
        McpToolEnum::Scan(DevkitScanTool),
        McpToolEnum::Health(DevkitHealthTool),
        McpToolEnum::Sync(DevkitSyncTool),
        McpToolEnum::Query(DevkitQueryTool),
        McpToolEnum::QueryRepos(DevkitQueryReposTool),
        McpToolEnum::Index(DevkitIndexTool),
        McpToolEnum::Note(DevkitNoteTool),
        McpToolEnum::Digest(DevkitDigestTool),
        McpToolEnum::Paper(DevkitPaperIndexTool),
        McpToolEnum::Experiment(DevkitExperimentLogTool),
        McpToolEnum::GithubInfo(DevkitGithubInfoTool),
        McpToolEnum::CodeMetrics(DevkitCodeMetricsTool),
        McpToolEnum::ModuleGraph(DevkitModuleGraphTool),
        McpToolEnum::NaturalLanguageQuery(DevkitNaturalLanguageQueryTool),
        McpToolEnum::VaultSearch(DevkitVaultSearchTool),
        McpToolEnum::VaultRead(DevkitVaultReadTool),
        McpToolEnum::VaultWrite(DevkitVaultWriteTool),
        McpToolEnum::VaultBacklinks(DevkitVaultBacklinksTool),
        McpToolEnum::ProjectContext(DevkitProjectContextTool),
        McpToolEnum::CodeSymbols(DevkitCodeSymbolsTool),
        McpToolEnum::DependencyGraph(DevkitDependencyGraphTool),
        McpToolEnum::CallGraph(DevkitCallGraphTool),
        McpToolEnum::DeadCode(DevkitDeadCodeTool),
        McpToolEnum::SemanticSearch(DevkitSemanticSearchTool),
        McpToolEnum::ArxivFetch(DevkitArxivFetchTool),
        McpToolEnum::EmbeddingStore(DevkitEmbeddingStoreTool),
        McpToolEnum::EmbeddingSearch(DevkitEmbeddingSearchTool),
        McpToolEnum::CrossRepoSearch(DevkitCrossRepoSearchTool),
        McpToolEnum::KnowledgeReport(DevkitKnowledgeReportTool),
        McpToolEnum::RelatedSymbols(DevkitRelatedSymbolsTool),
        McpToolEnum::HybridSearch(DevkitHybridSearchTool),
        McpToolEnum::SkillList(DevkitSkillListTool),
        McpToolEnum::SkillSearch(DevkitSkillSearchTool),
        McpToolEnum::SkillRun(DevkitSkillRunTool),
        McpToolEnum::SkillDiscover(DevkitSkillDiscoverTool),
        McpToolEnum::KnownLimitStore(DevkitKnownLimitStoreTool),
        McpToolEnum::KnownLimitList(DevkitKnownLimitListTool),
    ];
    for tool in all_tools {
        if let Some(allowed) = tiers
            && !allowed.contains(&tool.tier())
        {
            continue;
        }
        server.register_tool(tool);
    }
    server
}

/// Build an MCP server with all tools (backward compatible).
pub fn build_server() -> McpServer {
    build_server_with_tiers(None)
}

pub fn format_mcp_message(body: &serde_json::Value) -> String {
    let body_str = body.to_string();
    format!("Content-Length: {}\r\n\r\n{}\n", body_str.len(), body_str)
}

/// Parse tool tiers from a comma-separated string (e.g. "stable,beta").
fn parse_tool_tiers(s: &str) -> HashSet<ToolTier> {
    s.split(',')
        .map(|t| t.trim())
        .filter(|t| !t.is_empty())
        .filter_map(|s| s.parse().ok())
        .collect()
}

pub async fn run_stdio() -> anyhow::Result<()> {
    let tiers: Option<HashSet<ToolTier>> = std::env::var("DEVBASE_MCP_TOOL_TIERS")
        .ok()
        .map(|s| parse_tool_tiers(&s))
        .filter(|set| !set.is_empty());
    let server = build_server_with_tiers(tiers.as_ref());
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
