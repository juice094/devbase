use crate::mcp::McpTool;
use crate::registry::known_limits::KnownLimit;

#[derive(Clone)]
pub struct DevkitKnownLimitStoreTool;

impl McpTool for DevkitKnownLimitStoreTool {
    fn name(&self) -> &'static str {
        "devkit_known_limit_store"
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "description": r#"Store or update a known limit (L3 risk layer entry) in the devbase registry.

Use this when the user wants to:
- Record a system constraint or hard veto
- Track a known bug or external dependency risk
- Document a boundary that should not be crossed

Parameters:
- id: Unique identifier for the limit (kebab-case recommended)
- category: 'hard-veto' | 'known-bug' | 'external-dep'
- description: Human-readable description of the limit
- source: Optional source reference (e.g., 'AGENTS.md', 'oplog')
- severity: Optional severity 1-5

Returns: success boolean and stored limit id."#,
            "inputSchema": {
                "type": "object",
                "properties": {
                    "id": { "type": "string" },
                    "category": { "type": "string" },
                    "description": { "type": "string" },
                    "source": { "type": "string" },
                    "severity": { "type": "integer" }
                },
                "required": ["id", "category", "description"]
            }
        })
    }

    async fn invoke(
        &self,
        args: serde_json::Value,
        ctx: &mut crate::storage::AppContext,
    ) -> anyhow::Result<serde_json::Value> {
        let id = args.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let category = args.get("category").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let description =
            args.get("description").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let source = args.get("source").and_then(|v| v.as_str()).map(|s| s.to_string());
        let severity = args.get("severity").and_then(|v| v.as_i64()).map(|i| i as i32);

        if id.is_empty() || category.is_empty() || description.is_empty() {
            return Ok(serde_json::json!({
                "success": false,
                "error": "id, category, and description are required"
            }));
        }

        let limit = KnownLimit {
            id: id.clone(),
            category,
            description,
            source,
            severity,
            first_seen_at: chrono::Utc::now(),
            last_checked_at: None,
            mitigated: false,
        };

        let conn = ctx.conn()?;
        crate::registry::known_limits::save_known_limit(&conn, &limit)?;

        Ok(serde_json::json!({ "success": true, "id": id }))
    }
}

#[derive(Clone)]
pub struct DevkitKnownLimitListTool;

impl McpTool for DevkitKnownLimitListTool {
    fn name(&self) -> &'static str {
        "devkit_known_limit_list"
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "description": r#"List known limits (L3 risk layer entries) from the devbase registry.

Use this when the user wants to:
- Review current system constraints and hard vetoes
- Check mitigated vs open risks
- Audit boundary decisions

Parameters:
- category: Optional filter by category
- mitigated: Optional filter — true for resolved, false for open

Returns: JSON array of known limits."#,
            "inputSchema": {
                "type": "object",
                "properties": {
                    "category": { "type": "string" },
                    "mitigated": { "type": "boolean" }
                }
            }
        })
    }

    async fn invoke(
        &self,
        args: serde_json::Value,
        ctx: &mut crate::storage::AppContext,
    ) -> anyhow::Result<serde_json::Value> {
        let category = args.get("category").and_then(|v| v.as_str());
        let mitigated = args.get("mitigated").and_then(|v| v.as_bool());

        let conn = ctx.conn()?;
        let limits =
            crate::registry::known_limits::list_known_limits(&conn, category, mitigated)?;

        let results: Vec<serde_json::Value> = limits
            .into_iter()
            .map(|l| {
                serde_json::json!({
                    "id": l.id,
                    "category": l.category,
                    "description": l.description,
                    "source": l.source,
                    "severity": l.severity,
                    "first_seen_at": l.first_seen_at.to_rfc3339(),
                    "last_checked_at": l.last_checked_at.map(|d| d.to_rfc3339()),
                    "mitigated": l.mitigated,
                })
            })
            .collect();

        Ok(serde_json::json!({ "success": true, "limits": results, "count": results.len() }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::McpTool;

    #[test]
    fn test_name() {
        let t = DevkitKnownLimitStoreTool;
        assert_eq!(t.name(), "devkit_known_limit_store");
    }

    #[test]
    fn test_schema_is_object() {
        let t = DevkitKnownLimitStoreTool;
        let s = t.schema();
        assert!(s.is_object());
    }
}
