// SPDX-License-Identifier: MIT
// Copyright (c) 2026 juice094
use crate::mcp::McpTool;
use crate::storage::AppContext;
use anyhow::Context;

#[derive(Clone)]
pub struct DevkitProjectBriefTool;

impl McpTool for DevkitProjectBriefTool {
    fn name(&self) -> &'static str {
        "devkit_project_brief"
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "description": "Generate a Markdown project brief optimized for LLM context injection.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "repo_id": { "type": "string" },
                    "max_tokens": { "type": "integer", "default": 2000 }
                },
                "required": ["repo_id"]
            }
        })
    }

    async fn invoke(
        &self,
        args: serde_json::Value,
        ctx: &mut AppContext,
    ) -> anyhow::Result<serde_json::Value> {
        let repo_id = args.get("repo_id").and_then(|v| v.as_str()).context("repo_id required")?;
        let max_tokens = args.get("max_tokens").and_then(|v| v.as_u64()).unwrap_or(2000) as usize;

        let pool = ctx.pool();
        let repo_id_owned = repo_id.to_string();
        let brief = tokio::task::spawn_blocking(move || {
            let conn = pool.get()?;
            generate_brief(&conn, &repo_id_owned, max_tokens)
        })
        .await
        .map_err(|e| anyhow::anyhow!("spawn_blocking failed: {}", e))??;

        Ok(serde_json::json!({
            "success": true,
            "repo_id": repo_id,
            "brief": brief,
        }))
    }
}

fn generate_brief(
    conn: &rusqlite::Connection,
    repo_id: &str,
    max_tokens: usize,
) -> anyhow::Result<String> {
    let repos = crate::registry::repo::list_repos(conn)?;
    let repo = repos.into_iter().find(|r| r.id == repo_id);

    let mut sections: Vec<String> = Vec::new();
    sections.push(format!("# Project Brief: {}\n", repo_id));

    if let Some(ref r) = repo {
        let tags = r.tags.join(", ");
        let lang = r.language.as_deref().unwrap_or("unknown");
        let path = r.local_path.to_string_lossy();
        sections.push(format!(
            "## Overview\n- **Language**: {}\n- **Tags**: {}\n- **Path**: `{}`\n",
            lang, tags, path
        ));
    }

    let mut arch_lines = vec!["## Architecture\n".to_string()];
    if let Ok(ms) = crate::registry::knowledge::list_modules(conn, repo_id) {
        for (name, kind, _path) in ms.into_iter().take(20) {
            arch_lines.push(format!("- `{}` ({})\n", name, kind));
        }
    }
    let mut stmt = conn.prepare(
        "SELECT name, symbol_type, file_path, line_start FROM code_symbols WHERE repo_id = ?1 LIMIT 15"
    )?;
    let rows = stmt.query_map([repo_id], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, Option<i64>>(3)?,
        ))
    })?;
    let symbols: Vec<_> = rows.collect::<Result<Vec<_>, _>>()?;
    if !symbols.is_empty() {
        arch_lines.push("\n**Key Symbols:**\n".to_string());
        for (name, sym_type, file, line) in symbols {
            let loc = format!("{}:{}", file, line.map(|l| l.to_string()).unwrap_or_else(|| "?".to_string()));
            arch_lines.push(format!("- `{}` ({}) at `{}`\n", name, sym_type, loc));
        }
    }
    sections.push(arch_lines.join(""));

    if let Some(ref r) = repo {
        let commits = super::context::collect_recent_commits(&r.local_path, 7);
        let hot_files = super::context::collect_hot_files(&r.local_path, 14);
        let mut activity = vec!["## Recent Activity\n".to_string()];
        if !commits.is_empty() {
            activity.push("**Commits (last 7):**\n".to_string());
            for c in commits.iter().take(7) {
                activity.push(format!("- {}\n", c.lines().next().unwrap_or(c)));
            }
        }
        if !hot_files.is_empty() {
            activity.push("\n**Hot Files (14d):**\n".to_string());
            for f in hot_files.into_iter().take(10) {
                let path = f.get("path").and_then(|v| v.as_str()).unwrap_or("?");
                let count = f.get("change_count").and_then(|v| v.as_u64()).unwrap_or(0);
                activity.push(format!("- `{}` ({} changes)\n", path, count));
            }
        }
        sections.push(activity.join(""));
    }

    let limits = crate::registry::known_limits::list_known_limits(conn, None, Some(false))?;
    let repo_limits: Vec<_> = limits.into_iter().filter(|l| l.category != "resolved").take(10).collect();
    if !repo_limits.is_empty() {
        let mut limit_lines = vec!["## Known Limits & Tech Debt\n".to_string()];
        for l in repo_limits {
            let sev = l.severity.map(|s| s.to_string()).unwrap_or_else(|| "?".to_string());
            limit_lines.push(format!("- **[{}]** {} ({}): {}\n", l.id, l.category, sev, l.description));
        }
        sections.push(limit_lines.join(""));
    }

    let contexts: Vec<String> = conn
        .prepare("SELECT context_id FROM context_entity_links WHERE entity_id = ?1")?
        .query_map([repo_id], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    if !contexts.is_empty() {
        let mut ctx_lines = vec!["## Active Contexts\n".to_string()];
        for cid in contexts {
            if let Ok(Some(ctx)) = crate::registry::agent_context::get_context(conn, &cid) {
                let intent = ctx.intent.as_deref().unwrap_or("no intent");
                ctx_lines.push(format!("- **{}** — {}\n", ctx.name, intent));
                if let Ok(mems) = crate::registry::agent_context::list_memories(conn, &cid) {
                    for mem in mems.into_iter().take(3) {
                        let content = mem.content.lines().next().unwrap_or(&mem.content);
                        ctx_lines.push(format!("  - [{}] {}\n", mem.memory_type, content));
                    }
                }
            }
        }
        sections.push(ctx_lines.join(""));
    }

    let mut brief = sections.join("\n");
    let approx_chars = max_tokens * 4;
    if brief.len() > approx_chars {
        let trunc = &brief[..approx_chars];
        if let Some(pos) = trunc.rfind("\n## ") {
            brief = format!("{}\n\n_... (truncated to ~{} tokens)_\n", &brief[..pos], max_tokens);
        }
    }
    Ok(brief)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::McpTool;

    #[test]
    fn test_name() {
        assert_eq!(DevkitProjectBriefTool.name(), "devkit_project_brief");
    }
}
