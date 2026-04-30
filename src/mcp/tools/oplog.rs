use crate::mcp::McpTool;

#[derive(Clone)]
pub struct DevkitOplogQueryTool;

impl McpTool for DevkitOplogQueryTool {
    fn name(&self) -> &'static str {
        "devkit_oplog_query"
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "description": r#"Query the operation log (OpLog) for recent devbase activities.

Use this when the user wants to:
- See what actions devbase has performed recently (index, sync, scan, etc.)
- Debug why something did or did not happen
- Audit the history of workspace operations
- Check the status of recent background tasks

Parameters:
- limit: Maximum number of events to return (default: 20, max: 100)
- repo_id: Optional filter by repository ID. If omitted, returns workspace-wide activity.

Returns: JSON array of OpLog entries. Each entry includes:
  - id: event id
  - event_type: operation category (e.g. "index", "sync", "scan")
  - repo_id: affected repository, if any
  - status: "success" | "error" | "pending"
  - timestamp: ISO 8601 timestamp
  - duration_ms: execution time in milliseconds, if recorded
  - details: JSON object with operation-specific metadata"#,
            "inputSchema": {
                "type": "object",
                "properties": {
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of events to return (default: 20, max: 100)"
                    },
                    "repo_id": {
                        "type": "string",
                        "description": "Optional repository ID to filter by"
                    }
                }
            }
        })
    }

    async fn invoke(
        &self,
        args: serde_json::Value,
        ctx: &mut crate::storage::AppContext,
    ) -> anyhow::Result<serde_json::Value> {
        let limit = args
            .get("limit")
            .and_then(|v| v.as_i64())
            .map(|v| v.clamp(1, 100))
            .unwrap_or(20);
        let repo_id = args.get("repo_id").and_then(|v| v.as_str());

        let pool = ctx.pool();
        let entries = tokio::task::spawn_blocking({
            let repo_id = repo_id.map(|s| s.to_string());
            move || {
                let conn = pool.get()?;
                match repo_id {
                    Some(r) => crate::registry::WorkspaceRegistry::list_oplog_by_repo(&conn, &r, limit),
                    None => crate::registry::WorkspaceRegistry::list_oplog(&conn, limit),
                }
            }
        })
        .await
        .map_err(|e| anyhow::anyhow!("spawn_blocking failed: {}", e))??;

        let mut out = Vec::new();
        for e in entries {
            out.push(serde_json::json!({
                "id": e.id,
                "event_type": e.event_type,
                "repo_id": e.repo_id,
                "status": e.status,
                "timestamp": e.timestamp.to_rfc3339(),
                "duration_ms": e.duration_ms,
                "details": e.details,
            }));
        }

        Ok(serde_json::json!({
            "success": true,
            "events": out,
        }))
    }
}
