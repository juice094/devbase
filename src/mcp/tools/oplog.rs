// SPDX-License-Identifier: MIT
// Copyright (c) 2026 juice094
use crate::mcp::McpTool;
use anyhow::Context;
use std::collections::HashMap;

/// A single entry from the MCP oplog NDJSON file.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct McpOplogEntry {
    pub timestamp: String,
    pub tool: String,
    pub duration_ms: u64,
    pub success: bool,
    pub error_type: Option<String>,
}

/// Per-tool statistics.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ToolStats {
    pub tool: String,
    pub calls: usize,
    pub success: usize,
    pub errors: usize,
    pub avg_latency_ms: f64,
    pub p50_latency_ms: u64,
    pub p95_latency_ms: u64,
    pub p99_latency_ms: u64,
}

/// Overall latency distribution.
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct LatencyStats {
    pub p50_ms: u64,
    pub p95_ms: u64,
    pub p99_ms: u64,
    pub min_ms: u64,
    pub max_ms: u64,
}

/// Error classification breakdown.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ErrorStats {
    pub error_type: String,
    pub count: usize,
    pub tools: Vec<String>,
}

/// Complete analytics report for an MCP oplog file.
#[derive(Debug, Clone, serde::Serialize)]
pub struct OplogAnalyticsReport {
    pub total_calls: usize,
    pub success_count: usize,
    pub error_count: usize,
    pub success_rate: f64,
    pub unique_tools: usize,
    pub tool_breakdown: Vec<ToolStats>,
    pub latency_ms: LatencyStats,
    pub error_breakdown: Vec<ErrorStats>,
    pub time_range_start: Option<String>,
    pub time_range_end: Option<String>,
}

/// Parse an NDJSON file of `McpOplogEntry` records.
fn parse_mcp_oplog(path: &std::path::Path) -> anyhow::Result<Vec<McpOplogEntry>> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read MCP oplog: {}", path.display()))?;
    let mut entries = Vec::new();
    for line in content.lines().filter(|l| !l.trim().is_empty()) {
        match serde_json::from_str::<McpOplogEntry>(line) {
            Ok(entry) => entries.push(entry),
            Err(e) => {
                tracing::warn!("Skipping malformed oplog line: {} — {}", e, line);
            }
        }
    }
    Ok(entries)
}

/// Compute latency percentiles from a sorted slice.
fn latency_percentiles(sorted: &[u64]) -> LatencyStats {
    if sorted.is_empty() {
        return LatencyStats::default();
    }
    let n = sorted.len();
    let idx = |p: f64| -> usize { ((n as f64 * p / 100.0).ceil() as usize).saturating_sub(1).min(n - 1) };
    LatencyStats {
        p50_ms: sorted[idx(50.0)],
        p95_ms: sorted[idx(95.0)],
        p99_ms: sorted[idx(99.0)],
        min_ms: sorted[0],
        max_ms: sorted[n - 1],
    }
}

/// Analyze a collection of MCP oplog entries and produce a report.
pub fn analyze_mcp_oplog(entries: &[McpOplogEntry]) -> OplogAnalyticsReport {
    let total_calls = entries.len();
    let success_count = entries.iter().filter(|e| e.success).count();
    let error_count = total_calls.saturating_sub(success_count);
    let success_rate = if total_calls > 0 {
        (success_count as f64 / total_calls as f64) * 100.0
    } else {
        0.0
    };

    // Tool-level grouping
    let mut tool_map: HashMap<String, Vec<&McpOplogEntry>> = HashMap::new();
    for e in entries {
        tool_map.entry(e.tool.clone()).or_default().push(e);
    }

    let mut tool_breakdown: Vec<ToolStats> = tool_map
        .into_iter()
        .map(|(tool, recs)| {
            let calls = recs.len();
            let success = recs.iter().filter(|e| e.success).count();
            let errors = calls.saturating_sub(success);
            let mut latencies: Vec<u64> = recs.iter().map(|e| e.duration_ms).collect();
            latencies.sort_unstable();
            let lat = latency_percentiles(&latencies);
            let avg = if !latencies.is_empty() {
                latencies.iter().sum::<u64>() as f64 / latencies.len() as f64
            } else {
                0.0
            };
            ToolStats {
                tool,
                calls,
                success,
                errors,
                avg_latency_ms: avg,
                p50_latency_ms: lat.p50_ms,
                p95_latency_ms: lat.p95_ms,
                p99_latency_ms: lat.p99_ms,
            }
        })
        .collect();
    tool_breakdown.sort_by_key(|b| std::cmp::Reverse(b.calls));

    // Overall latency
    let mut all_latencies: Vec<u64> = entries.iter().map(|e| e.duration_ms).collect();
    all_latencies.sort_unstable();
    let latency_ms = latency_percentiles(&all_latencies);

    // Error classification
    let mut error_map: HashMap<String, (usize, Vec<String>)> = HashMap::new();
    for e in entries.iter().filter(|e| !e.success) {
        let key = e.error_type.clone().unwrap_or_else(|| "unknown".to_string());
        let entry = error_map.entry(key).or_insert_with(|| (0, Vec::new()));
        entry.0 += 1;
        if !entry.1.contains(&e.tool) {
            entry.1.push(e.tool.clone());
        }
    }
    let mut error_breakdown: Vec<ErrorStats> = error_map
        .into_iter()
        .map(|(error_type, (count, tools))| ErrorStats { error_type, count, tools })
        .collect();
    error_breakdown.sort_by_key(|b| std::cmp::Reverse(b.count));

    let time_range_start = entries.iter().map(|e| e.timestamp.clone()).min();
    let time_range_end = entries.iter().map(|e| e.timestamp.clone()).max();

    OplogAnalyticsReport {
        total_calls,
        success_count,
        error_count,
        success_rate,
        unique_tools: tool_breakdown.len(),
        tool_breakdown,
        latency_ms,
        error_breakdown,
        time_range_start,
        time_range_end,
    }
}

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
- Analyze MCP tool call patterns, error rates, and latency distributions

Parameters:
- limit: Maximum number of events to return (default: 20, max: 100)
- repo_id: Optional filter by repository ID. If omitted, returns workspace-wide activity.
- analytics: If true, returns a statistical summary of MCP oplog data instead of raw events.

Returns:
  - Normal mode: JSON array of OpLog entries
  - Analytics mode: JSON object with total_calls, success_rate, latency percentiles, tool breakdown, and error classification"#,
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
                    },
                    "analytics": {
                        "type": "boolean",
                        "description": "Return MCP oplog analytics summary instead of raw events",
                        "default": false
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
        let analytics = args.get("analytics").and_then(|v| v.as_bool()).unwrap_or(false);

        if analytics {
            // MCP oplog analytics path
            let data_dir = dirs::data_local_dir()
                .context("Failed to determine local data directory")?;
            let log_path = data_dir.join("devbase").join("mcp-oplog.ndjson");

            if !log_path.exists() {
                return Ok(serde_json::json!({
                    "success": true,
                    "message": "No MCP oplog data found",
                    "total_calls": 0,
                }));
            }

            let entries = parse_mcp_oplog(&log_path)?;
            let report = analyze_mcp_oplog(&entries);
            return Ok(serde_json::json!({
                "success": true,
                "total_calls": report.total_calls,
                "success_count": report.success_count,
                "error_count": report.error_count,
                "success_rate": report.success_rate,
                "unique_tools": report.unique_tools,
                "latency_ms": report.latency_ms,
                "tool_breakdown": report.tool_breakdown,
                "error_breakdown": report.error_breakdown,
                "time_range_start": report.time_range_start,
                "time_range_end": report.time_range_end,
            }));
        }

        // Original DB oplog query path
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
                    Some(r) => crate::registry::workspace::list_oplog_by_repo(&conn, &r, limit),
                    None => crate::registry::workspace::list_oplog(&conn, limit),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::McpTool;

    #[test]
    fn test_name() {
        let t = DevkitOplogQueryTool;
        assert_eq!(t.name(), "devkit_oplog_query");
    }

    #[test]
    fn test_schema_is_object() {
        let t = DevkitOplogQueryTool;
        let s = t.schema();
        assert!(s.is_object());
    }

    #[test]
    fn test_latency_percentiles_basic() {
        let data = vec![10, 20, 30, 40, 50, 60, 70, 80, 90, 100];
        let lat = latency_percentiles(&data);
        assert_eq!(lat.min_ms, 10);
        assert_eq!(lat.max_ms, 100);
        assert_eq!(lat.p50_ms, 50);
        assert_eq!(lat.p95_ms, 100);
        assert_eq!(lat.p99_ms, 100);
    }

    #[test]
    fn test_latency_percentiles_empty() {
        let lat = latency_percentiles(&[]);
        assert_eq!(lat.p50_ms, 0);
        assert_eq!(lat.p95_ms, 0);
    }

    #[test]
    fn test_analyze_mcp_oplog_smoke() {
        let entries = vec![
            McpOplogEntry {
                timestamp: "2026-01-01T00:00:00Z".to_string(),
                tool: "devkit_health".to_string(),
                duration_ms: 100,
                success: true,
                error_type: None,
            },
            McpOplogEntry {
                timestamp: "2026-01-01T00:01:00Z".to_string(),
                tool: "devkit_health".to_string(),
                duration_ms: 200,
                success: true,
                error_type: None,
            },
            McpOplogEntry {
                timestamp: "2026-01-01T00:02:00Z".to_string(),
                tool: "devkit_sync".to_string(),
                duration_ms: 500,
                success: false,
                error_type: Some("timeout".to_string()),
            },
        ];
        let report = analyze_mcp_oplog(&entries);
        assert_eq!(report.total_calls, 3);
        assert_eq!(report.success_count, 2);
        assert_eq!(report.error_count, 1);
        assert!((report.success_rate - 66.6667).abs() < 0.01, "success_rate = {}", report.success_rate);
        assert_eq!(report.unique_tools, 2);

        let health = report
            .tool_breakdown
            .iter()
            .find(|t| t.tool == "devkit_health")
            .unwrap();
        assert_eq!(health.calls, 2);
        assert_eq!(health.success, 2);
        // Nearest-rank ceil-based: n=2, p=50 -> idx=0 -> sorted[0]=100
        assert_eq!(health.p50_latency_ms, 100);

        let sync = report
            .tool_breakdown
            .iter()
            .find(|t| t.tool == "devkit_sync")
            .unwrap();
        assert_eq!(sync.calls, 1);
        assert_eq!(sync.errors, 1);
        assert_eq!(sync.p95_latency_ms, 500);

        assert_eq!(report.error_breakdown.len(), 1);
        assert_eq!(report.error_breakdown[0].error_type, "timeout");
        assert_eq!(report.error_breakdown[0].count, 1);
        assert!(report.error_breakdown[0].tools.contains(&"devkit_sync".to_string()));
    }

    #[test]
    fn test_analyze_mcp_oplog_empty() {
        let report = analyze_mcp_oplog(&[]);
        assert_eq!(report.total_calls, 0);
        assert_eq!(report.success_rate, 0.0);
        assert!(report.tool_breakdown.is_empty());
        assert!(report.error_breakdown.is_empty());
    }

    #[test]
    fn test_parse_mcp_oplog_roundtrip() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("test-oplog.ndjson");
        let lines = [
            r#"{"timestamp":"2026-01-01T00:00:00Z","tool":"t1","duration_ms":10,"success":true,"error_type":null}"#,
            r#"{"timestamp":"2026-01-01T00:00:01Z","tool":"t2","duration_ms":20,"success":false,"error_type":"err"}"#,
            "not-json-line",
            r#"{"timestamp":"2026-01-01T00:00:02Z","tool":"t3","duration_ms":30,"success":true}"#,
        ];
        std::fs::write(&path, lines.join("\n")).unwrap();

        let entries = parse_mcp_oplog(&path).unwrap();
        assert_eq!(entries.len(), 3); // malformed line skipped
        assert_eq!(entries[0].tool, "t1");
        assert_eq!(entries[1].tool, "t2");
        assert_eq!(entries[2].tool, "t3");
        assert!(entries[2].error_type.is_none());
    }
}
