use crate::mcp::{McpTool, ToolStreamEvent, StreamPhase};
use crate::storage::AppContext;

#[derive(Clone)]
pub struct DevkitStatusTool;

impl McpTool for DevkitStatusTool {
    fn name(&self) -> &'static str {
        "devkit_status"
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "description": r#"Get the index status of one or all registered repositories.

Use this when the user or a sub-agent needs to check whether a repo is fresh,
stale, missing, or unknown before performing code analysis or search.

Parameters:
- repo_id: Optional specific repo ID. If omitted, returns status for all repos.

Returns: JSON object with repo state (fresh/stale/missing/unknown), last indexed
hash, current HEAD hash, symbol count, embedding count, and changed files."#,
            "inputSchema": {
                "type": "object",
                "properties": {
                    "repo_id": {
                        "type": "string",
                        "description": "Specific repo ID to query. Omit to query all."
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
        let conn = ctx.conn()?;
        let repos = crate::registry::repo::list_repos(&conn)?;
        let target_id = args.get("repo_id").and_then(|v| v.as_str());

        #[derive(serde::Serialize)]
        struct RepoStatus {
            id: String,
            path: String,
            state: crate::knowledge_engine::index_state::IndexState,
            last_indexed_hash: Option<String>,
            current_head_hash: Option<String>,
            indexed_at: Option<String>,
            symbols_count: usize,
            embeddings_count: usize,
        }

        let mut statuses = Vec::new();
        for repo in &repos {
            if let Some(id) = target_id {
                if repo.id != id {
                    continue;
                }
            }

            let state = crate::knowledge_engine::index_state::get_repo_index_state(&conn, repo);
            let (last_hash, indexed_at) = conn
                .query_row(
                    "SELECT last_commit_hash, indexed_at FROM repo_index_state WHERE repo_id = ?1",
                    [&repo.id],
                    |row| Ok((row.get::<_, Option<String>>(0)?, row.get::<_, Option<String>>(1)?)),
                )
                .unwrap_or((None, None));

            let current_head = crate::semantic_index::git_diff::current_head_hash(&repo.local_path)
                .ok()
                .flatten();

            let symbols_count: usize = conn
                .query_row(
                    "SELECT COUNT(*) FROM code_symbols WHERE repo_id = ?1",
                    [&repo.id],
                    |row| row.get(0),
                )
                .unwrap_or(0);

            let embeddings_count: usize = conn
                .query_row(
                    "SELECT COUNT(*) FROM code_embeddings WHERE repo_id = ?1",
                    [&repo.id],
                    |row| row.get(0),
                )
                .unwrap_or(0);

            statuses.push(RepoStatus {
                id: repo.id.clone(),
                path: repo.local_path.to_string_lossy().to_string(),
                state,
                last_indexed_hash: last_hash,
                current_head_hash: current_head,
                indexed_at,
                symbols_count,
                embeddings_count,
            });
        }

        let overall = if statuses.iter().all(|s| s.state.is_fresh()) {
            "fresh"
        } else if statuses.iter().any(|s| matches!(s.state, crate::knowledge_engine::index_state::IndexState::Stale { .. })) {
            "stale"
        } else if statuses.iter().any(|s| matches!(s.state, crate::knowledge_engine::index_state::IndexState::Missing)) {
            "missing"
        } else {
            "unknown"
        };

        Ok(serde_json::json!({
            "repos": statuses,
            "overall": overall,
        }))
    }
}

#[derive(Clone)]
pub struct DevkitIndexStreamTool;

impl McpTool for DevkitIndexStreamTool {
    fn name(&self) -> &'static str {
        "devkit_index_stream"
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "description": r#"Index a repository with real-time progress streaming.

Use this for long-running index operations where the caller wants to see
progress updates. Pass stream: true in the request params to receive
an array of ToolStreamEvent objects.

Parameters:
- repo_id: Optional repo ID or path. Omit to index all registered repos.

Returns: Final result with indexed count and duration. When stream: true,
returns a JSON array of progress events."#,
            "inputSchema": {
                "type": "object",
                "properties": {
                    "repo_id": {
                        "type": "string",
                        "description": "Repo ID or local path to index. Omit for all."
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
        // Non-streaming fallback: delegate to normal index
        let path = args.get("repo_id").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let pool = ctx.pool();
        let count = tokio::task::spawn_blocking(move || {
            let mut conn = pool.get()?;
            crate::knowledge_engine::run_index(&mut conn, &path)
        })
        .await
        .map_err(|e| anyhow::anyhow!("spawn_blocking failed: {}", e))??;
        Ok(serde_json::json!({ "success": true, "indexed": count }))
    }

    async fn invoke_stream(
        &self,
        args: serde_json::Value,
        ctx: &mut AppContext,
    ) -> anyhow::Result<Vec<ToolStreamEvent>> {
        use std::time::Instant;
        let start = Instant::now();
        let mut events = Vec::new();

        let path = args.get("repo_id").and_then(|v| v.as_str()).unwrap_or("").to_string();

        events.push(ToolStreamEvent {
            phase: StreamPhase::Progress,
            payload: serde_json::json!({ "step": "start", "path": &path }),
        });

        let (tx, rx) = crossbeam_channel::bounded(32);
        let pool = ctx.pool();
        let path_clone = path.clone();
        let handle = tokio::task::spawn_blocking(move || {
            let mut conn = pool.get()?;
            crate::knowledge_engine::run_index_with_progress(&mut conn, &path_clone, Some(tx))
        });

        // Collect progress events as they arrive from the blocking thread.
        loop {
            match rx.recv_timeout(std::time::Duration::from_millis(100)) {
                Ok(msg) => {
                    events.push(ToolStreamEvent {
                        phase: StreamPhase::Progress,
                        payload: serde_json::json!({ "step": msg }),
                    });
                }
                Err(crossbeam_channel::RecvTimeoutError::Timeout) => {
                    if handle.is_finished() {
                        while let Ok(msg) = rx.try_recv() {
                            events.push(ToolStreamEvent {
                                phase: StreamPhase::Progress,
                                payload: serde_json::json!({ "step": msg }),
                            });
                        }
                        break;
                    }
                }
                Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
                    while let Ok(msg) = rx.try_recv() {
                        events.push(ToolStreamEvent {
                            phase: StreamPhase::Progress,
                            payload: serde_json::json!({ "step": msg }),
                        });
                    }
                    break;
                }
            }
        }

        let count = handle
            .await
            .map_err(|e| anyhow::anyhow!("spawn_blocking failed: {}", e))??;

        let duration_ms = start.elapsed().as_millis() as u64;
        events.push(ToolStreamEvent {
            phase: StreamPhase::Done,
            payload: serde_json::json!({ "indexed": count, "duration_ms": duration_ms }),
        });

        Ok(events)
    }
}
