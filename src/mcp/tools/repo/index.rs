// SPDX-License-Identifier: MIT
// Copyright (c) 2026 juice094
use crate::clients::KnowledgeClient;
use crate::mcp::McpTool;
use crate::storage::AppContext;

#[derive(Clone)]
pub struct DevkitIndexTool;

impl McpTool for DevkitIndexTool {
    fn name(&self) -> &'static str {
        "devkit_index"
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "description": r#"Build or refresh the Tantivy full-text search index for repository summaries, README extracts, and module structures. This makes repos searchable via devkit_query and devkit_natural_language_query.

Use this when the user wants to:
- Make newly registered repos searchable
- Update the search index after significant code changes
- Enable full-text search across repo documentation

Do NOT use this for:
- Registering new repos (use devkit_scan instead)
- Querying repos directly (use devkit_query_repos or devkit_natural_language_query instead)
- Getting code metrics (use devkit_code_metrics instead)

Parameters:
- path: Specific repo path to index. If omitted, all registered repos are re-indexed.

Returns: JSON with indexed count and error count."#,
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

    async fn invoke(
        &self,
        args: serde_json::Value,
        ctx: &mut AppContext,
    ) -> anyhow::Result<serde_json::Value> {
        let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
        KnowledgeClient::run_index(ctx, path)
    }
}
