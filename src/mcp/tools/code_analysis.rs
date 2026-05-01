use crate::mcp::McpTool;
use crate::mcp::clients::RegistryClient;
use crate::repository::dependency::DependencyRepository;
use crate::repository::knowledge::KnowledgeRepository;
use crate::repository::repo::RepoRepository;
use crate::repository::symbol::SymbolRepository;
use crate::storage::AppContext;
use anyhow::Context;

#[derive(Clone)]
pub struct DevkitCodeMetricsTool;

impl McpTool for DevkitCodeMetricsTool {
    fn name(&self) -> &'static str {
        "devkit_code_metrics"
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "description": r#"Compute code metrics for registered repositories: total lines of code, file count, language breakdown, and rough complexity indicators (via tokei).

Use this when the user wants to:
- Compare the size of different projects
- Identify the primary language of a repo
- Find the largest or most complex codebase in the workspace

Do NOT use this for:
- Module-level structure analysis (use devkit_module_graph instead)
- Git status or health checks (use devkit_health instead)
- Searching code content (use devkit_natural_language_query instead)

Parameters:
- repo_id: Specific repo ID. If omitted, returns metrics for all registered repos.

Returns: JSON array of metric objects per repo: repo_id, total_lines, code_lines, comment_lines, blank_lines, file_count, and language_breakdown."#,
            "inputSchema": {
                "type": "object",
                "properties": {
                    "repo_id": { "type": "string", "description": "Specific repo ID; if omitted, returns all repos", "default": "" }
                }
            }
        })
    }

    async fn invoke(
        &self,
        args: serde_json::Value,
        ctx: &mut AppContext,
    ) -> anyhow::Result<serde_json::Value> {
        let repo_id = args.get("repo_id").and_then(|v| v.as_str()).unwrap_or("").to_string();

        if repo_id.is_empty() {
            RegistryClient::list_code_metrics(ctx)
        } else {
            RegistryClient::get_code_metrics(ctx, &repo_id)
        }
    }
}
#[derive(Clone)]
pub struct DevkitModuleGraphTool;

impl McpTool for DevkitModuleGraphTool {
    fn name(&self) -> &'static str {
        "devkit_module_graph"
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "description": r#"Extract the module and binary target structure from a Rust repository using cargo metadata. Returns crates, binaries, libraries, and their interdependencies.

Use this when the user wants to:
- Understand the architecture of a Rust workspace
- Find all binary targets (executables) in a project
- Map crate dependencies within a workspace

Do NOT use this for:
- Non-Rust repositories (returns empty or error)
- General code metrics like line counts (use devkit_code_metrics instead)
- Git operations (use devkit_health or devkit_sync instead)

Parameters:
- repo_id: Repository ID. If omitted, analyzes the current directory.

Returns: JSON with workspace_members, packages (name, version, targets), and dependency graph."#,
            "inputSchema": {
                "type": "object",
                "properties": {
                    "repo_id": { "type": "string", "description": "Repository ID", "default": "" }
                }
            }
        })
    }

    async fn invoke(
        &self,
        args: serde_json::Value,
        ctx: &mut AppContext,
    ) -> anyhow::Result<serde_json::Value> {
        let repo_id = args.get("repo_id").and_then(|v| v.as_str()).unwrap_or("").to_string();

        let pool = ctx.pool();
        tokio::task::spawn_blocking(move || {

            let conn = pool.get()?;
            if repo_id.is_empty() {
                let repos = RepoRepository::new(&conn).list_repos(None)?;
                let mut all_modules = vec![];
                for repo in repos {
                    if repo.language.as_deref() == Some("Rust") {
                        let modules = KnowledgeRepository::new(&conn).list_modules(&repo.id)?;
                        if !modules.is_empty() {
                            all_modules.push(serde_json::json!({
                                "repo_id": repo.id,
                                "modules": modules.iter().map(|(n, t, p)| serde_json::json!({
                                    "name": n, "type": t, "path": p
                                })).collect::<Vec<_>>()
                            }));
                        }
                    }
                }
                Ok::<_, anyhow::Error>(serde_json::json!({ "success": true, "count": all_modules.len(), "repos": all_modules }))
            } else {
                let modules = KnowledgeRepository::new(&conn).list_modules(&repo_id)?;
                Ok(serde_json::json!({
                    "success": true,
                    "repo_id": repo_id,
                    "modules": modules.iter().map(|(n, t, p)| serde_json::json!({
                        "name": n, "type": t, "path": p
                    })).collect::<Vec<_>>()
                }))
            }
        })
        .await
        .map_err(|e| anyhow::anyhow!("spawn_blocking failed: {}", e))?
    }
}
#[derive(Clone)]
pub struct DevkitCodeSymbolsTool;

impl McpTool for DevkitCodeSymbolsTool {
    fn name(&self) -> &'static str {
        "devkit_code_symbols"
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "description": r#"Query the semantic code symbol index for a repository. Returns functions, structs, enums, traits, impls, and modules extracted via tree-sitter AST parsing.

Use this when the user wants to:
- Find the definition of a specific function or struct
- Explore the API surface of a repository
- Answer questions like "what functions are in file X?" or "where is struct Y defined?"
- Understand the module structure at the symbol level

Do NOT use this for:
- Full-text search across code contents (use devkit_natural_language_query instead)
- Getting repo-level summaries (use devkit_query_repos instead)
- Code metrics like line counts (use devkit_code_metrics instead)

Parameters:
- repo_id: Registered repository ID to query.
- name_filter: Optional symbol name substring to filter results (case-insensitive).
- symbol_type: Optional filter by symbol type: "function", "struct", "enum", "trait", "impl", "module", "type_alias", "constant", "static".
- file_path: Optional file path substring to filter by source file.
- limit: Maximum results to return (default: 50, max: 200).

Returns: JSON array of symbols with file_path, name, symbol_type, line_start, line_end, and optional signature."#,
            "inputSchema": {
                "type": "object",
                "properties": {
                    "repo_id": { "type": "string" },
                    "name_filter": { "type": "string", "default": "" },
                    "symbol_type": { "type": "string", "default": "" },
                    "file_path": { "type": "string", "default": "" },
                    "limit": { "type": "integer", "default": 50 }
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
        let name_filter = args.get("name_filter").and_then(|v| v.as_str()).unwrap_or("");
        let symbol_type = args.get("symbol_type").and_then(|v| v.as_str()).unwrap_or("");
        let file_path = args.get("file_path").and_then(|v| v.as_str()).unwrap_or("");
        let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(50).min(200) as usize;

        let repo_id = repo_id.to_string();
        let name_filter = name_filter.to_string();
        let symbol_type = symbol_type.to_string();
        let file_path = file_path.to_string();

        let pool = ctx.pool();
        tokio::task::spawn_blocking(move || {
            let conn = pool.get()?;
            let symbols = SymbolRepository::new(&conn).query_code_symbols(
                &repo_id,
                Some(name_filter.as_str()).filter(|s| !s.is_empty()),
                Some(symbol_type.as_str()).filter(|s| !s.is_empty()),
                Some(file_path.as_str()).filter(|s| !s.is_empty()),
                limit,
            )?;
            let symbols: Vec<serde_json::Value> = symbols
                .into_iter()
                .map(|s| {
                    serde_json::json!({
                        "file_path": s.file_path,
                        "symbol_type": s.symbol_type,
                        "name": s.name,
                        "line_start": s.line_start,
                        "line_end": s.line_end,
                        "signature": s.signature,
                    })
                })
                .collect();

            Ok::<_, anyhow::Error>(serde_json::json!({
                "success": true,
                "repo_id": repo_id,
                "count": symbols.len(),
                "symbols": symbols,
            }))
        })
        .await
        .map_err(|e| anyhow::anyhow!("spawn_blocking failed: {}", e))?
    }
}
#[derive(Clone)]
pub struct DevkitDependencyGraphTool;

impl McpTool for DevkitDependencyGraphTool {
    fn name(&self) -> &'static str {
        "devkit_dependency_graph"
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "description": r#"Query the cross-repository dependency graph. Returns which local repos a given repo depends on, or which repos depend on it (reverse dependencies). Edges are discovered by parsing Cargo.toml, package.json, and go.mod manifest files.

Use this when the user wants to:
- Understand the impact of changing a shared library ("who depends on X?")
- Explore the architecture of a monorepo or multi-repo workspace
- Find all repos that use a specific local crate/package/module
- Plan refactoring or breaking changes across repo boundaries

Do NOT use this for:
- Code-level "who calls this function" (use devkit_code_symbols instead)
- Full-text search (use devkit_natural_language_query instead)
- Remote/external dependency analysis (this only tracks local repos)

Parameters:
- repo_id: Registered repository ID to query.
- direction: "outgoing" (repos this repo depends on) or "incoming" (repos that depend on this repo). Default: "outgoing".
- relation_type: Optional filter by relation type (default "depends_on").

Returns: JSON array of dependency edges with target_repo_id, relation_type, and confidence score (1.0 = verified local path dependency, 0.7-0.9 = name heuristic match)."#,
            "inputSchema": {
                "type": "object",
                "properties": {
                    "repo_id": { "type": "string" },
                    "direction": { "type": "string", "default": "outgoing" },
                    "relation_type": { "type": "string", "default": "" }
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
        let direction = args.get("direction").and_then(|v| v.as_str()).unwrap_or("outgoing");
        let relation_type = args.get("relation_type").and_then(|v| v.as_str()).unwrap_or("");

        let repo_id = repo_id.to_string();
        let direction = direction.to_string();
        let relation_type = relation_type.to_string();

        let pool = ctx.pool();
        tokio::task::spawn_blocking(move || {
            let conn = pool.get()?;

            let mut results = Vec::new();
            if direction == "incoming" || direction == "reverse" {
                let rows = DependencyRepository::new(&conn).list_reverse_dependencies(&repo_id)?;
                for (from_id, rel, conf) in rows {
                    if !relation_type.is_empty() && rel != relation_type {
                        continue;
                    }
                    results.push(serde_json::json!({
                        "source_repo_id": from_id,
                        "target_repo_id": repo_id,
                        "relation_type": rel,
                        "confidence": conf,
                    }));
                }
            } else {
                let rows = DependencyRepository::new(&conn).list_dependencies(&repo_id)?;
                for (to_id, rel, conf) in rows {
                    if !relation_type.is_empty() && rel != relation_type {
                        continue;
                    }
                    results.push(serde_json::json!({
                        "source_repo_id": repo_id,
                        "target_repo_id": to_id,
                        "relation_type": rel,
                        "confidence": conf,
                    }));
                }
            }

            Ok::<_, anyhow::Error>(serde_json::json!({
                "success": true,
                "repo_id": repo_id,
                "direction": direction,
                "count": results.len(),
                "dependencies": results,
            }))
        })
        .await
        .map_err(|e| anyhow::anyhow!("spawn_blocking failed: {}", e))?
    }
}
#[derive(Clone)]
pub struct DevkitCallGraphTool;

impl McpTool for DevkitCallGraphTool {
    fn name(&self) -> &'static str {
        "devkit_call_graph"
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "description": r#"Query the intra-repository call graph extracted by tree-sitter AST parsing. Answer "which functions call X" or "what does function Y call" within a single repo.

Use this when the user wants to:
- Find all call sites of a specific function inside a repo
- Understand the control flow impact of changing a function
- Discover unused functions (no incoming call edges)
- Trace how data flows through the codebase

Do NOT use this for:
- Cross-repo dependency questions (use devkit_dependency_graph instead)
- Finding symbol definitions (use devkit_code_symbols instead)
- Full-text search (use devkit_natural_language_query instead)

Parameters:
- repo_id: Registered repository ID to query.
- callee_name: Name of the called function to search for (required for "who calls X").
- caller_name: Name of the calling function to search for (required for "what does Y call").
- file_path: Optional file path substring to narrow scope.
- limit: Maximum results (default: 50, max: 200).

At least one of callee_name or caller_name must be provided.

Returns: JSON array of call edges with caller_file, caller_symbol, caller_line, callee_name."#,
            "inputSchema": {
                "type": "object",
                "properties": {
                    "repo_id": { "type": "string" },
                    "callee_name": { "type": "string", "default": "" },
                    "caller_name": { "type": "string", "default": "" },
                    "file_path": { "type": "string", "default": "" },
                    "limit": { "type": "integer", "default": 50 }
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
        let callee_name = args.get("callee_name").and_then(|v| v.as_str()).unwrap_or("");
        let caller_name = args.get("caller_name").and_then(|v| v.as_str()).unwrap_or("");
        let file_path = args.get("file_path").and_then(|v| v.as_str()).unwrap_or("");
        let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(50).min(200) as usize;

        if callee_name.is_empty() && caller_name.is_empty() {
            anyhow::bail!("At least one of callee_name or caller_name must be provided");
        }

        let repo_id = repo_id.to_string();
        let callee_name = callee_name.to_string();
        let caller_name = caller_name.to_string();
        let file_path = file_path.to_string();

        let pool = ctx.pool();
        tokio::task::spawn_blocking(move || {
            let conn = pool.get()?;
            let edges = SymbolRepository::new(&conn).query_call_graph(
                &repo_id,
                Some(callee_name.as_str()).filter(|s| !s.is_empty()),
                Some(caller_name.as_str()).filter(|s| !s.is_empty()),
                Some(file_path.as_str()).filter(|s| !s.is_empty()),
                limit,
            )?;
            let calls: Vec<serde_json::Value> = edges
                .into_iter()
                .map(|e| {
                    serde_json::json!({
                        "caller_file": e.caller_file,
                        "caller_symbol": e.caller_symbol,
                        "caller_line": e.caller_line,
                        "callee_name": e.callee_name,
                    })
                })
                .collect();
            Ok::<_, anyhow::Error>(serde_json::json!({
                "success": true,
                "repo_id": repo_id,
                "count": calls.len(),
                "calls": calls,
            }))
        })
        .await
        .map_err(|e| anyhow::anyhow!("spawn_blocking failed: {}", e))?
    }
}
#[derive(Clone)]
pub struct DevkitDeadCodeTool;

impl McpTool for DevkitDeadCodeTool {
    fn name(&self) -> &'static str {
        "devkit_dead_code"
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "description": r#"Identify potentially dead (unused) functions in a repository by comparing the code symbol index against the call graph. Returns functions that are defined but never called within the same repo.

Use this when the user wants to:
- Clean up unused code in a repository
- Identify functions that may be safe to remove or deprecate
- Audit API surface for internal-only dead functions
- Reduce maintenance burden by eliminating unnecessary code

Do NOT use this for:
- Public API methods that are called by external consumers (devbase only sees intra-repo calls)
- Functions referenced by trait bounds or dynamic dispatch (may have false positives)
- Cross-repo usage analysis (use devkit_dependency_graph + devkit_call_graph instead)

Parameters:
- repo_id: Registered repository ID to analyze.
- limit: Maximum results (default: 50, max: 200).
- include_pub: Also report `pub fn` items (default: false; public functions may be called externally).

Returns: JSON array of potentially dead functions with file_path, name, and line_start."#,
            "inputSchema": {
                "type": "object",
                "properties": {
                    "repo_id": { "type": "string" },
                    "limit": { "type": "integer", "default": 50 },
                    "include_pub": { "type": "boolean", "default": false }
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
        let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(50).min(200) as usize;
        let include_pub = args.get("include_pub").and_then(|v| v.as_bool()).unwrap_or(false);

        let repo_id = repo_id.to_string();

        let pool = ctx.pool();
        tokio::task::spawn_blocking(move || {

            let conn = pool.get()?;
            let dead = SymbolRepository::new(&conn).query_dead_code(&repo_id, include_pub, limit)?;
            let dead: Vec<serde_json::Value> = dead
                .into_iter()
                .map(|d| {
                    serde_json::json!({
                        "file_path": d.file_path,
                        "name": d.name,
                        "line_start": d.line_start,
                    })
                })
                .collect();

            Ok::<_, anyhow::Error>(serde_json::json!({
                "success": true,
                "repo_id": repo_id,
                "count": dead.len(),
                "note": "Results may include false positives: public APIs, trait methods, callback registrations, and dynamically dispatched functions are not visible in the intra-repo call graph.",
                "dead_functions": dead,
            }))
        })
        .await
        .map_err(|e| anyhow::anyhow!("spawn_blocking failed: {}", e))?
    }
}
