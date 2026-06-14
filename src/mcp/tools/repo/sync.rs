// SPDX-License-Identifier: MIT
// Copyright (c) 2026 juice094
use crate::clients::SyncClient;
use crate::mcp::{McpTool, check_destructive_enabled};
use crate::storage::AppContext;

#[derive(Clone)]
pub struct DevkitSyncTool;

impl McpTool for DevkitSyncTool {
    fn name(&self) -> &'static str {
        "devkit_sync"
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "description": r#"Synchronize registered repositories with their upstream remotes by pulling and/or pushing commits according to each repo's inferred SyncPolicy (Mirror / Conservative / Rebase / Merge, determined by tags).

This is a WRITE operation. By default it runs in dry-run mode (no files are modified) for safety.

⚠️ SECURITY: This tool modifies Git state (pull/push/rebase/merge). Managed-gate skips untagged repos automatically. Set DEVBASE_MCP_ENABLE_DESTRUCTIVE=1 if this tool is unavailable.

Use this when the user wants to:
- Update local repos to match their remotes (git pull)
- Push local commits to remotes (git push)
- Preview what a sync would do before executing it
- Batch-sync multiple repos filtered by tags

Do NOT use this for:
- Checking repo status without modifying anything (use devkit_health instead)
- Scanning or registering new repos (use devkit_scan instead)
- Repos with dirty working directories — these are automatically skipped for safety
- Repos with diverged histories under Conservative policy — these are also skipped

Parameters:
- dry_run: Defaults to true. When true, previews the sync plan without modifying any files. Set to false to execute.
- filter_tags: Comma-separated tags to limit which repos are synced (e.g., "third-party,reference").

Returns: JSON object with per-repo sync results including: repo_id, action (pull/push/skipped), status (success/conflict/error), and safety_reason if skipped."#,
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

    async fn invoke(
        &self,
        args: serde_json::Value,
        ctx: &mut AppContext,
    ) -> anyhow::Result<serde_json::Value> {
        check_destructive_enabled()?;
        let dry_run = args.get("dry_run").and_then(|v| v.as_bool()).unwrap_or(true);
        let filter_tags = args.get("filter_tags").and_then(|v| v.as_str());
        let filter_tags_vec = filter_tags.map(|s| {
            s.split(',')
                .map(|t| t.trim().to_string())
                .filter(|t| !t.is_empty())
                .collect::<Vec<_>>()
        });
        SyncClient::sync_repos(ctx, dry_run, filter_tags_vec).await
    }
}
