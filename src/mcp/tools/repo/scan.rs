// SPDX-License-Identifier: MIT
// Copyright (c) 2026 juice094
use crate::clients::ScanClient;
use crate::mcp::McpTool;
use crate::storage::AppContext;
use anyhow::Context;

#[derive(Clone)]
pub struct DevkitScanTool;

impl McpTool for DevkitScanTool {
    fn name(&self) -> &'static str {
        "devkit_scan"
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "description": r#"Scan a directory to discover Git repositories and non-Git workspaces (e.g., openclaw, generic projects marked by SOUL.md or .devbase files).

Use this when the user wants to:
- Discover repositories in a directory for the first time
- Add newly cloned or downloaded projects to the devbase workspace
- Find ZIP-snapshot folders (named with -main/-master suffix) that need Git migration

Do NOT use this for:
- Listing already-registered repos (use devkit_query_repos instead)
- Checking repo status (use devkit_health instead)
- Searching across repos (use devkit_query_repos or devkit_natural_language_query instead)

Parameters:
- path: Directory to scan (absolute or relative). Defaults to current directory.
- register: If true, discovered repos are persisted to the devbase SQLite registry. If false, returns a preview only.

Returns: JSON array of discovered repos with id, path, language, source_type, and whether registration succeeded."#,
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

    async fn invoke(
        &self,
        args: serde_json::Value,
        ctx: &mut AppContext,
    ) -> anyhow::Result<serde_json::Value> {
        let path = args
            .get("path")
            .and_then(|v| v.as_str())
            .context("Missing required argument: path")?;
        let register = args.get("register").and_then(|v| v.as_bool()).unwrap_or(false);
        ctx.scan_directory(path, register).await
    }
}
