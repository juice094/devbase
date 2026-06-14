// SPDX-License-Identifier: MIT
// Copyright (c) 2026 juice094
use crate::clients::HealthClient;
use crate::mcp::McpTool;
use crate::storage::AppContext;

#[derive(Clone)]
pub struct DevkitHealthTool;

impl McpTool for DevkitHealthTool {
    fn name(&self) -> &'static str {
        "devkit_health"
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "description": r#"Check the health status of all registered repositories in the devbase workspace. This is a read-only diagnostic tool.

Use this when the user wants to:
- Get an overview of all tracked repos and their Git status
- Identify repos that are dirty (uncommitted changes), ahead (local commits not pushed), behind (remote commits not pulled), or diverged
- Check environment prerequisites (Rust, Go, Node.js, CMake versions)
- Find repos that need attention before a sync

Do NOT use this for:
- Pulling or pushing changes (use devkit_sync instead)
- Searching repos by language or tag (use devkit_query_repos instead)
- Scanning new directories (use devkit_scan instead)

Parameters:
- detail: If true, returns per-repo Git status (dirty/ahead/behind/diverged), last sync time, and file count. If false, returns a summary only.

Returns: JSON object with workspace summary and per-repo health records. Each repo includes: id, path, language, tags, git_status (dirty/ahead/behind/diverged/up_to_date), last_synced_at, file_count, and health score."#,
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

    async fn invoke(
        &self,
        args: serde_json::Value,
        ctx: &mut AppContext,
    ) -> anyhow::Result<serde_json::Value> {
        let detail = args.get("detail").and_then(|v| v.as_bool()).unwrap_or(false);
        ctx.check_health(detail).await
    }
}
