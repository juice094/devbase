use crate::mcp::McpTool;
use crate::skill_runtime::registry;
use anyhow::Context;
use std::time::Duration;

#[derive(Clone)]
pub struct DevkitSkillListTool;

impl McpTool for DevkitSkillListTool {
    fn name(&self) -> &'static str {
        "devkit_skill_list"
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "description": r#"List installed devbase skills. Optionally filter by skill type (builtin, custom, system).

Use this when the user wants to:
- See what AI skills are available in the devbase workspace
- Discover builtin capabilities like embed-repo, search-workspace, knowledge-report
- Check installed custom skills

Parameters:
- skill_type: Optional filter — "builtin", "custom", or "system". Omit for all.

Returns: JSON array of skills with id, name, version, type, description, tags, and path."#,
            "inputSchema": {
                "type": "object",
                "properties": {
                    "skill_type": {
                        "type": "string",
                        "description": "Filter by skill type: builtin, custom, or system"
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
        let skill_type =
            args.get("skill_type").and_then(|v| v.as_str()).and_then(|s| s.parse().ok());
        let conn = ctx.conn()?;
        let skills = registry::list_skills(&conn, skill_type, None)?;
        let results: Vec<serde_json::Value> = skills
            .into_iter()
            .map(|s| {
                serde_json::json!({
                    "id": s.id,
                    "name": s.name,
                    "version": s.version,
                    "skill_type": s.skill_type.as_str(),
                    "description": s.description,
                    "tags": s.tags,
                    "path": s.local_path,
                })
            })
            .collect();
        Ok(serde_json::json!({ "success": true, "skills": results, "count": results.len() }))
    }
}

#[derive(Clone)]
pub struct DevkitSkillSearchTool;

impl McpTool for DevkitSkillSearchTool {
    fn name(&self) -> &'static str {
        "devkit_skill_search"
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "description": r#"Search installed devbase skills by name or description (text search).

Use this when the user wants to:
- Find a skill matching a keyword like "audit" or "embed"
- Discover the right skill for a task without knowing its exact ID

Parameters:
- query: Search string matched against skill name and description.
- limit: Maximum results. Default 10.

Returns: JSON array of matching skills."#,
            "inputSchema": {
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Search query"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum results",
                        "default": 10
                    }
                },
                "required": ["query"]
            }
        })
    }

    async fn invoke(
        &self,
        args: serde_json::Value,
        ctx: &mut crate::storage::AppContext,
    ) -> anyhow::Result<serde_json::Value> {
        let query = args
            .get("query")
            .and_then(|v| v.as_str())
            .context("Missing required argument: query")?;
        let limit = args.get("limit").and_then(|v| v.as_i64()).unwrap_or(10) as usize;
        let conn = ctx.conn()?;
        let skills = registry::search_skills_text(&conn, query, limit, None)?;
        let results: Vec<serde_json::Value> = skills
            .into_iter()
            .map(|s| {
                serde_json::json!({
                    "id": s.id,
                    "name": s.name,
                    "version": s.version,
                    "skill_type": s.skill_type.as_str(),
                    "description": s.description,
                    "tags": s.tags,
                })
            })
            .collect();
        Ok(serde_json::json!({ "success": true, "skills": results, "count": results.len() }))
    }
}

#[derive(Clone)]
pub struct DevkitSkillDiscoverTool;

impl McpTool for DevkitSkillDiscoverTool {
    fn name(&self) -> &'static str {
        "devkit_skill_discover"
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "description": r#"Auto-discover a project directory or Git URL and package it as a devbase Skill.

Use this when the user wants to:
- Convert a GitHub project into an executable AI Skill
- Analyze a local project's CLI/API surface and generate a SKILL.md draft
- Package a Rust/Node/Python/Go/Docker project for AI agent consumption

Parameters:
- path: Local directory path or Git URL (http/https/git@) to analyze.
- skill_id: Optional explicit skill ID. Defaults to project name.
- dry_run: If true, print generated files without installing. Default false.

Returns: JSON with discovered skill id, name, version, description, category, and entry_script path."#,
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Local directory path or Git URL to discover"
                    },
                    "skill_id": {
                        "type": "string",
                        "description": "Optional explicit skill ID"
                    },
                    "dry_run": {
                        "type": "boolean",
                        "description": "Preview without installing",
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
        ctx: &mut crate::storage::AppContext,
    ) -> anyhow::Result<serde_json::Value> {
        let path = args
            .get("path")
            .and_then(|v| v.as_str())
            .context("Missing required argument: path")?;
        let skill_id = args.get("skill_id").and_then(|v| v.as_str()).map(|s| s.to_string());
        let dry_run = args.get("dry_run").and_then(|v| v.as_bool()).unwrap_or(false);

        let conn = ctx.conn()?;
        let project_path = std::path::PathBuf::from(path);
        let skill = crate::skill_runtime::discover::discover_and_install(
            &conn,
            &project_path,
            skill_id.as_deref(),
            dry_run,
        )?;

        Ok(serde_json::json!({
            "success": true,
            "id": skill.id,
            "name": skill.name,
            "version": skill.version,
            "description": skill.description,
            "category": skill.category,
            "skill_type": skill.skill_type.as_str(),
            "entry_script": skill.entry_script,
            "local_path": skill.local_path.to_string_lossy().to_string(),
            "dry_run": dry_run,
        }))
    }
}

#[derive(Clone)]
pub struct DevkitSkillRunTool;

impl McpTool for DevkitSkillRunTool {
    fn name(&self) -> &'static str {
        "devkit_skill_run"
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "description": r#"Execute a devbase skill by ID with arguments.

Use this when the user wants to:
- Run a specific skill like embed-repo, search-workspace, or knowledge-report
- Pass arguments to a skill (e.g. repo_id, query, limit)
- Trigger an AI capability from a conversation

Parameters:
- skill_id: ID of the skill to run (e.g. "embed-repo", "knowledge-report").
- args: Object mapping argument names to values. Example: {"repo_id": "devbase", "device": "cuda"}.
- timeout: Maximum execution time in seconds. Default 30.

Returns: JSON with status, stdout, stderr, exit_code, and duration_ms."#,
            "inputSchema": {
                "type": "object",
                "properties": {
                    "skill_id": {
                        "type": "string",
                        "description": "Skill ID to execute"
                    },
                    "args": {
                        "type": "object",
                        "description": "Arguments as key-value pairs",
                        "default": {}
                    },
                    "timeout": {
                        "type": "integer",
                        "description": "Timeout in seconds",
                        "default": 30
                    }
                },
                "required": ["skill_id"]
            }
        })
    }

    async fn invoke(
        &self,
        args: serde_json::Value,
        ctx: &mut crate::storage::AppContext,
    ) -> anyhow::Result<serde_json::Value> {
        let skill_id = args
            .get("skill_id")
            .and_then(|v| v.as_str())
            .context("Missing required argument: skill_id")?;
        let timeout = args.get("timeout").and_then(|v| v.as_u64()).unwrap_or(30);
        let skill_args: Vec<String> = args
            .get("args")
            .and_then(|v| v.as_object())
            .map(|obj| {
                obj.iter()
                    .map(|(k, v)| format!("{}={}", k, v.as_str().unwrap_or(&v.to_string())))
                    .collect()
            })
            .unwrap_or_default();

        let conn = ctx.conn()?;
        let skill = registry::get_skill(&conn, skill_id)?
            .context(format!("Skill '{}' not found", skill_id))?;

        let exec_id = registry::record_execution_start(
            &conn,
            skill_id,
            &serde_json::to_string(&skill_args).unwrap_or_default(),
        )?;

        let pool = ctx.pool();
        let result = tokio::task::spawn_blocking(move || {
            let conn = pool.get()?;
            crate::skill_runtime::executor::run_skill(
                &conn,
                &skill,
                &skill_args,
                Duration::from_secs(timeout),
            )
        })
        .await
        .map_err(|e| anyhow::anyhow!("spawn_blocking failed: {}", e))??;

        registry::record_execution_finish(&conn, exec_id, &result)?;

        Ok(serde_json::json!({
            "success": result.status == crate::skill_runtime::ExecutionStatus::Success,
            "status": result.status.as_str(),
            "stdout": result.stdout,
            "stderr": result.stderr,
            "exit_code": result.exit_code,
            "duration_ms": result.duration_ms,
        }))
    }
}
