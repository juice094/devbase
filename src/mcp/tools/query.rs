use crate::mcp::McpTool;
use anyhow::Context;

#[derive(Clone)]
pub struct DevkitQueryTool;

impl McpTool for DevkitQueryTool {
    fn name(&self) -> &'static str {
        "devkit_query"
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "description": r#"Execute a structured query against the devbase knowledge base (repos + vault notes). Supports filter expressions for language, stale status, behind/ahead counts, tags, and keywords.

Use this when the user wants to:
- Run precise filter queries like "lang:rust stale:>30" or "behind:>10"
- Combine multiple conditions in a single structured expression
- Export or script queries that need exact semantics

Do NOT use this for:
- Natural language questions (use devkit_natural_language_query instead)
- Searching only vault notes (use devkit_vault_search instead)
- Querying only repos with structured filters (use devkit_query_repos instead)

Parameters:
- query: Query expression. Examples: "lang:rust", "stale:>30", "behind:>10", "tag:third-party", "note:agri".
- limit: Maximum results. Default 50.

Returns: JSON array of matching items, each with type (repo or note), id, title, and relevance metadata."#,
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

    async fn invoke(
        &self,
        args: serde_json::Value,
        ctx: &mut crate::storage::AppContext,
    ) -> anyhow::Result<serde_json::Value> {
        let expression = args
            .get("expression")
            .and_then(|v| v.as_str())
            .context("Missing required argument: expression")?;
        let expression = expression.to_string();
        let pool = ctx.pool();
        tokio::task::spawn_blocking(move || {
            let rt = tokio::runtime::Handle::current();
            let config = crate::config::Config::load()?;
            let conn = pool.get()?;
            rt.block_on(crate::query::run_json(&conn, &expression, 0, 1, &config))
        })
        .await
        .map_err(|e| anyhow::anyhow!("spawn_blocking failed: {}", e))?
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::McpTool;

    #[test]
    fn test_name() {
        let t = DevkitQueryTool;
        assert_eq!(t.name(), "devkit_query");
    }

    #[test]
    fn test_schema_is_object() {
        let t = DevkitQueryTool;
        let s = t.schema();
        assert!(s.is_object());
    }
}
