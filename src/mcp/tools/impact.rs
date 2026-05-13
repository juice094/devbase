// SPDX-License-Identifier: MIT
// Copyright (c) 2026 juice094
use crate::mcp::McpTool;
use crate::storage::AppContext;
use anyhow::Context;

#[derive(Clone)]
pub struct DevkitImpactAnalysisTool;

impl McpTool for DevkitImpactAnalysisTool {
    fn name(&self) -> &'static str {
        "devkit_impact_analysis"
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "description": "Analyze the impact of modifying a specific code symbol. Returns callers, callees, related symbols, potentially affected tests, and recent change history. Use this before refactoring to understand blast radius.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "repo_id": { "type": "string" },
                    "symbol_name": { "type": "string", "description": "Function, struct, or trait name to analyze" },
                    "depth": { "type": "integer", "default": 2, "description": "Call graph traversal depth (1-3)" }
                },
                "required": ["repo_id", "symbol_name"]
            }
        })
    }

    async fn invoke(
        &self,
        args: serde_json::Value,
        ctx: &mut AppContext,
    ) -> anyhow::Result<serde_json::Value> {
        let repo_id = args.get("repo_id").and_then(|v| v.as_str()).context("repo_id required")?;
        let symbol_name = args.get("symbol_name").and_then(|v| v.as_str()).context("symbol_name required")?;
        let depth = args.get("depth").and_then(|v| v.as_u64()).unwrap_or(2).clamp(1, 3) as usize;

        let pool = ctx.pool();
        let repo_id_owned = repo_id.to_string();
        let symbol_name_owned = symbol_name.to_string();
        let result = tokio::task::spawn_blocking(move || {
            let conn = pool.get()?;
            analyze_impact(&conn, &repo_id_owned, &symbol_name_owned, depth)
        })
        .await
        .map_err(|e| anyhow::anyhow!("spawn_blocking failed: {}", e))??;

        Ok(serde_json::json!({
            "success": true,
            "repo_id": repo_id,
            "symbol_name": symbol_name,
            "depth": depth,
            "impact": result,
        }))
    }
}

fn analyze_impact(
    conn: &rusqlite::Connection,
    repo_id: &str,
    symbol_name: &str,
    depth: usize,
) -> anyhow::Result<serde_json::Value> {
    // 1. Symbol metadata
    let mut stmt = conn.prepare(
        "SELECT name, file_path, symbol_type, line_start, signature
         FROM code_symbols WHERE repo_id = ?1 AND name = ?2 LIMIT 1"
    )?;
    let symbol_meta = stmt.query_row([repo_id, symbol_name], |row| {
        Ok(serde_json::json!({
            "name": row.get::<_, String>(0)?,
            "file": row.get::<_, String>(1)?,
            "type": row.get::<_, String>(2)?,
            "line": row.get::<_, Option<i64>>(3)?,
            "signature": row.get::<_, Option<String>>(4)?,
        }))
    }).ok();

    // 2. Direct callers (up to 2 levels)
    let mut callers = Vec::new();
    let mut visited_callers: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut current = vec![symbol_name.to_string()];
    for _level in 0..depth {
        let mut next = Vec::new();
        for sym in &current {
            let mut stmt = conn.prepare(
                "SELECT DISTINCT caller_symbol, caller_file
                 FROM code_call_graph WHERE repo_id = ?1 AND callee_name = ?2"
            )?;
            let rows = stmt.query_map([repo_id, sym], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?;
            for (caller, file) in rows.flatten() {
                if visited_callers.insert(caller.clone()) {
                    callers.push(serde_json::json!({
                        "symbol": caller,
                        "file": file,
                        "level": _level + 1,
                    }));
                    next.push(caller);
                }
            }
        }
        current = next;
    }

    // 3. Direct callees (up to 2 levels)
    let mut callees = Vec::new();
    let mut visited_callees: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut current = vec![symbol_name.to_string()];
    for _level in 0..depth {
        let mut next = Vec::new();
        for sym in &current {
            let mut stmt = conn.prepare(
                "SELECT DISTINCT callee_name, caller_file
                 FROM code_call_graph WHERE repo_id = ?1 AND caller_symbol = ?2"
            )?;
            let rows = stmt.query_map([repo_id, sym], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?;
            for (callee, file) in rows.flatten() {
                if visited_callees.insert(callee.clone()) {
                    callees.push(serde_json::json!({
                        "symbol": callee,
                        "file": file,
                        "level": _level + 1,
                    }));
                    next.push(callee);
                }
            }
        }
        current = next;
    }

    // 4. Related symbols (conceptual)
    let related = crate::registry::WorkspaceRegistry::find_related_symbols(
        conn, repo_id, symbol_name, 10,
    )
    .unwrap_or_default()
    .into_iter()
    .map(|(_src_repo, _src_sym, tgt_repo, tgt_sym, link_type, strength)| {
        serde_json::json!({
            "symbol": tgt_sym,
            "repo": tgt_repo,
            "link_type": link_type,
            "strength": strength,
        })
    })
    .collect::<Vec<_>>();

    // 5. Tests: heuristic — symbols containing "test_" that call or are called by target
    let mut tests = Vec::new();
    let mut stmt = conn.prepare(
        "SELECT DISTINCT caller_symbol FROM code_call_graph
         WHERE repo_id = ?1 AND callee_name = ?2 AND caller_symbol LIKE 'test_%'"
    )?;
    let rows = stmt.query_map([repo_id, symbol_name], |row| {
        row.get::<_, String>(0)
    })?;
    for t in rows.flatten() {
        tests.push(t);
    }
    // Also test functions that are CALLED by the target (test helpers)
    let mut stmt = conn.prepare(
        "SELECT DISTINCT callee_name FROM code_call_graph
         WHERE repo_id = ?1 AND caller_symbol LIKE 'test_%' AND callee_name = ?2"
    )?;
    let rows = stmt.query_map([repo_id, symbol_name], |row| {
        row.get::<_, String>(0)
    })?;
    for t in rows.flatten() {
        tests.push(t);
    }
    tests.sort();
    tests.dedup();

    // 6. Recent history (oplog + recent commits mentioning symbol)
    let mut history = Vec::new();
    if let Ok(entries) = crate::registry::workspace::list_oplog_by_repo(conn, repo_id, 10) {
        for entry in entries {
            if entry.details.as_ref().map(|d| d.contains(symbol_name)).unwrap_or(false) {
                history.push(serde_json::json!({
                    "source": "oplog",
                    "timestamp": entry.timestamp.to_rfc3339(),
                    "event": entry.event_type.as_str(),
                    "details": entry.details,
                }));
            }
        }
    }

    Ok(serde_json::json!({
        "symbol": symbol_meta,
        "callers": callers,
        "callees": callees,
        "related": related,
        "tests": tests,
        "history": history,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::McpTool;

    #[test]
    fn test_name() {
        assert_eq!(DevkitImpactAnalysisTool.name(), "devkit_impact_analysis");
    }
}
