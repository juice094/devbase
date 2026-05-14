// SPDX-License-Identifier: MIT
// Copyright (c) 2026 juice094
//! MCP tool: devkit_index_health — Tantivy + SQLite 索引健康度诊断。

use crate::mcp::McpTool;
use crate::registry::ENTITY_TYPE_REPO;
use crate::search::list_indexed_repo_ids_at;
use crate::storage::AppContext;
use std::collections::HashSet;
use tantivy::{Index, ReloadPolicy};

#[derive(Clone)]
pub struct DevkitIndexHealthTool;

impl McpTool for DevkitIndexHealthTool {
    fn name(&self) -> &'static str {
        "devkit_index_health"
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "description": r#"Diagnose the health of devbase search indexes (Tantivy + SQLite).

Returns an overall health score (0-100) and detailed metrics for:
- Tantivy repo index: document count, schema validity, orphan detection
- Tantivy symbol index: document count, schema validity
- SQLite registry: repo count, journal mode, orphan records

Use this when:
- Search results seem incomplete or stale
- Before/after running devkit_index to verify consistency
- Troubleshooting "missing repo" or "orphan document" issues

Parameters: none (inspects all registered indexes automatically)."#,
            "inputSchema": {
                "type": "object",
                "properties": {}
            }
        })
    }

    async fn invoke(
        &self,
        _args: serde_json::Value,
        ctx: &mut AppContext,
    ) -> anyhow::Result<serde_json::Value> {
        run_index_health(ctx)
    }
}

fn build_repo_schema() -> tantivy::schema::Schema {
    let mut schema_builder = tantivy::schema::Schema::builder();
    schema_builder.add_text_field("id", tantivy::schema::STRING | tantivy::schema::STORED);
    schema_builder.add_text_field("title", tantivy::schema::TEXT | tantivy::schema::STORED);
    schema_builder.add_text_field("content", tantivy::schema::TEXT);
    schema_builder.add_text_field("tags", tantivy::schema::TEXT);
    schema_builder.add_text_field("doc_type", tantivy::schema::TEXT | tantivy::schema::STORED);
    schema_builder.build()
}

fn build_symbol_schema() -> tantivy::schema::Schema {
    let mut sb = tantivy::schema::Schema::builder();
    sb.add_text_field("repo_id", tantivy::schema::TEXT | tantivy::schema::STORED);
    sb.add_text_field("name", tantivy::schema::TEXT | tantivy::schema::STORED);
    sb.add_text_field("signature", tantivy::schema::TEXT | tantivy::schema::STORED);
    sb.add_text_field("file_path", tantivy::schema::TEXT | tantivy::schema::STORED);
    sb.add_text_field("line_start", tantivy::schema::STORED);
    sb.build()
}

fn check_index_at(
    path: &std::path::Path,
    expected_schema: &tantivy::schema::Schema,
) -> anyhow::Result<(bool, usize)> {
    if !path.exists() {
        return Ok((true, 0));
    }
    let idx = match Index::open_in_dir(path) {
        Ok(i) => i,
        Err(_) => return Ok((false, 0)),
    };
    let schema_valid = idx.schema() == *expected_schema;
    let reader = idx.reader_builder().reload_policy(ReloadPolicy::Manual).try_into()?;
    let num_docs = reader.searcher().num_docs() as usize;
    Ok((schema_valid, num_docs))
}

pub fn run_index_health(ctx: &mut AppContext) -> anyhow::Result<serde_json::Value> {
    let index_path = ctx.storage.index_path()?;
    let symbol_index_path = ctx.storage.symbol_index_path()?;

    // 1. Tantivy repo index
    let repo_schema = build_repo_schema();
    let (repo_schema_valid, repo_docs) = check_index_at(&index_path, &repo_schema)?;

    // 2. Tantivy symbol index
    let sym_schema = build_symbol_schema();
    let (sym_schema_valid, sym_docs) = check_index_at(&symbol_index_path, &sym_schema)?;

    // 3. SQLite repo count + journal mode
    let conn = ctx.conn_mut()?;
    let sqlite_repo_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM entities WHERE entity_type = ?1",
            [ENTITY_TYPE_REPO],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let journal_mode: String = conn
        .query_row("PRAGMA journal_mode", [], |row| row.get(0))
        .unwrap_or_else(|_| "unknown".to_string());

    // 4. Orphans from orphan_tantivy_docs table
    let orphan_rows: Vec<String> = {
        let mut stmt = conn.prepare("SELECT repo_id FROM orphan_tantivy_docs")?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        rows.filter_map(Result::ok).collect()
    };
    let recorded_orphans = orphan_rows.len();

    // 5. Live consistency: Tantivy IDs vs SQLite IDs
    let (live_orphans, missing_from_index) = {
        let tantivy_ids: HashSet<String> = match list_indexed_repo_ids_at(&index_path) {
            Ok(ids) => ids.into_iter().collect(),
            Err(_) => HashSet::new(),
        };
        let sqlite_ids: HashSet<String> = {
            let mut stmt = conn.prepare("SELECT id FROM entities WHERE entity_type = ?1")?;
            let rows = stmt.query_map([ENTITY_TYPE_REPO], |row| row.get::<_, String>(0))?;
            rows.filter_map(Result::ok).collect()
        };
        let orphans = tantivy_ids.difference(&sqlite_ids).count();
        let missing = sqlite_ids.difference(&tantivy_ids).count();
        (orphans, missing)
    };

    drop(conn);

    // 6. Health score calculation
    let mut score = 100i64;
    if !repo_schema_valid {
        score = 0;
    } else {
        score -= (live_orphans as i64).min(6) * 5;
        score -= (missing_from_index as i64).min(10) * 3;
        if journal_mode != "wal" {
            score -= 10;
        }
    }
    let score = score.max(0) as u8;

    Ok(serde_json::json!({
        "overall_score": score,
        "journal_mode": journal_mode,
        "tantivy_repo_index": {
            "path": index_path.to_string_lossy(),
            "schema_valid": repo_schema_valid,
            "num_docs": repo_docs,
        },
        "tantivy_symbol_index": {
            "path": symbol_index_path.to_string_lossy(),
            "schema_valid": sym_schema_valid,
            "num_docs": sym_docs,
        },
        "sqlite_registry": {
            "num_repos": sqlite_repo_count,
            "recorded_orphans": recorded_orphans,
        },
        "consistency": {
            "live_orphans": live_orphans,
            "missing_from_index": missing_from_index,
            "orphan_repo_ids": orphan_rows,
        }
    }))
}
