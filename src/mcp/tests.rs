// SPDX-License-Identifier: MIT
// Copyright (c) 2026 juice094
use super::*;
use crate::storage::StorageBackend;

fn test_ctx() -> (crate::storage::AppContext, tempfile::TempDir) {
    let backend = std::sync::Arc::new(crate::storage::TempStorageBackend::new());
    let ctx = crate::storage::AppContext::with_storage(backend).unwrap();
    // Return a dummy TempDir to keep call-site destructuring compatible.
    let tmp = tempfile::tempdir().unwrap();
    (ctx, tmp)
}

#[tokio::test]
async fn test_initialize() {
    let server = build_server();
    let req = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize"
    });
    let (mut ctx, _tmp) = test_ctx();
    let resp = server.handle_request(req, &mut ctx).await.unwrap();
    assert_eq!(resp.get("jsonrpc").unwrap(), "2.0");
    let result = resp.get("result").unwrap();
    assert_eq!(result.get("protocolVersion").unwrap(), "2024-11-05");
    assert_eq!(result.get("serverInfo").unwrap().get("name").unwrap(), "devbase");
    assert!(result.get("capabilities").unwrap().get("tools").is_some());
}

#[tokio::test]
async fn test_tools_list() {
    let server = build_server();
    let req = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/list"
    });
    let (mut ctx, _tmp) = test_ctx();
    let resp = server.handle_request(req, &mut ctx).await.unwrap();
    let tools = resp.get("result").unwrap().get("tools").unwrap().as_array().unwrap();
    assert_eq!(tools.len(), 68);
    let names: Vec<&str> = tools.iter().map(|t| t.get("name").unwrap().as_str().unwrap()).collect();
    assert!(names.contains(&"devkit_index_health"));
    assert!(names.contains(&"devkit_vault_export"));
    assert!(names.contains(&"devkit_vault_history"));
    assert!(names.contains(&"devkit_search_quality"));
    assert!(names.contains(&"devkit_session_save"));
    assert!(names.contains(&"devkit_session_list"));
    assert!(names.contains(&"devkit_session_resume"));
    assert!(names.contains(&"devkit_session_recall"));
    assert!(names.contains(&"devkit_session_index"));
    assert!(names.contains(&"devkit_session_export"));
    assert!(names.contains(&"devkit_session_import"));
    assert!(names.contains(&"devkit_evaluate"));
    assert!(names.contains(&"devkit_scan"));
    assert!(names.contains(&"devkit_health"));
    assert!(names.contains(&"devkit_sync"));
    assert!(names.contains(&"devkit_query"));
    assert!(names.contains(&"devkit_query_repos"));
    assert!(names.contains(&"devkit_index"));
    assert!(names.contains(&"devkit_index_stream"));
    assert!(names.contains(&"devkit_status"));
    assert!(names.contains(&"devkit_note"));
    assert!(names.contains(&"devkit_digest"));
    assert!(names.contains(&"devkit_paper_index"));
    assert!(names.contains(&"devkit_experiment_log"));
    assert!(names.contains(&"devkit_github_info"));
    assert!(names.contains(&"devkit_code_metrics"));
    assert!(names.contains(&"devkit_module_graph"));
    assert!(names.contains(&"devkit_code_symbols"));
    assert!(names.contains(&"devkit_dependency_graph"));
    assert!(names.contains(&"devkit_call_graph"));
    assert!(names.contains(&"devkit_dead_code"));
    assert!(names.contains(&"devkit_semantic_search"));
    assert!(names.contains(&"devkit_embedding_store"));
    assert!(names.contains(&"devkit_embedding_search"));
    assert!(names.contains(&"devkit_natural_language_query"));
    assert!(names.contains(&"devkit_vault_search"));
    assert!(names.contains(&"devkit_vault_read"));
    assert!(names.contains(&"devkit_vault_write"));
    assert!(names.contains(&"devkit_vault_backlinks"));
    assert!(names.contains(&"devkit_vault_daily"));
    assert!(names.contains(&"devkit_vault_graph"));
    assert!(names.contains(&"devkit_project_context"));
    assert!(names.contains(&"devkit_project_brief"));
    assert!(names.contains(&"devkit_impact_analysis"));
    assert!(names.contains(&"devkit_cross_repo_search"));
    assert!(names.contains(&"devkit_knowledge_report"));
    assert!(names.contains(&"devkit_related_symbols"));
    assert!(names.contains(&"devkit_hybrid_search"));
    assert!(names.contains(&"devkit_skill_list"));
    assert!(names.contains(&"devkit_skill_search"));
    assert!(names.contains(&"devkit_skill_run"));
    assert!(names.contains(&"devkit_skill_discover"));
    assert!(names.contains(&"devkit_known_limit_store"));
    assert!(names.contains(&"devkit_known_limit_list"));
    assert!(names.contains(&"devkit_relation_store"));
    assert!(names.contains(&"devkit_relation_query"));
    assert!(names.contains(&"devkit_relation_delete"));
    assert!(names.contains(&"devkit_workflow_list"));
    assert!(names.contains(&"devkit_workflow_run"));
    assert!(names.contains(&"devkit_workflow_status"));
    for tool in tools {
        assert!(tool.get("name").is_some());
        assert!(tool.get("description").is_some());
        assert!(tool.get("inputSchema").is_some());
    }
}

#[tokio::test]
async fn test_tools_call_devkit_health() {
    let server = build_server();
    let req = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 3,
        "method": "tools/call",
        "params": {
            "name": "devkit_health",
            "arguments": { "detail": false }
        }
    });
    let (mut ctx, _tmp) = test_ctx();
    let resp = server.handle_request(req, &mut ctx).await.unwrap();
    let result = resp.get("result").unwrap();
    let content = result.get("content").unwrap().as_array().unwrap();
    let text = content[0].get("text").unwrap().as_str().unwrap();
    let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
    if parsed.get("success").unwrap() != &serde_json::Value::Bool(true) {
        eprintln!(
            "devkit_health returned error: {}",
            serde_json::to_string_pretty(&parsed).unwrap()
        );
    }
    assert_eq!(parsed.get("success").unwrap(), true);
    let summary = parsed.get("summary").unwrap();
    assert!(summary.get("total_repos").unwrap().as_i64().unwrap() >= 0);
}

#[tokio::test]
async fn test_tools_call_devkit_query() {
    let server = build_server();
    let req = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 4,
        "method": "tools/call",
        "params": {
            "name": "devkit_query",
            "arguments": { "expression": "lang:rust" }
        }
    });
    let (mut ctx, _tmp) = test_ctx();
    let resp = server.handle_request(req, &mut ctx).await.unwrap();
    let result = resp.get("result").unwrap();
    let content = result.get("content").unwrap().as_array().unwrap();
    let text = content[0].get("text").unwrap().as_str().unwrap();
    let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
    assert_eq!(parsed.get("success").unwrap(), true);
    assert!(parsed.get("count").unwrap().as_i64().unwrap() >= 0);
}

#[tokio::test]
async fn test_tools_call_unknown_tool() {
    let server = build_server();
    let req = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 5,
        "method": "tools/call",
        "params": {
            "name": "unknown_tool",
            "arguments": {}
        }
    });
    let (mut ctx, _tmp) = test_ctx();
    let resp = server.handle_request(req, &mut ctx).await.unwrap();
    assert!(resp.get("error").is_some());
    let error = resp.get("error").unwrap();
    assert_eq!(error.get("code").unwrap().as_i64().unwrap(), -32602);
}

#[tokio::test]
async fn test_unknown_method() {
    let server = build_server();
    let req = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 6,
        "method": "unknown/method"
    });
    let (mut ctx, _tmp) = test_ctx();
    let resp = server.handle_request(req, &mut ctx).await.unwrap();
    assert!(resp.get("error").is_some());
    let error = resp.get("error").unwrap();
    assert_eq!(error.get("code").unwrap().as_i64().unwrap(), -32601);
}

#[tokio::test]
async fn test_tools_call_devkit_project_context() {
    let server = build_server();
    let req = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 7,
        "method": "tools/call",
        "params": {
            "name": "devkit_project_context",
            "arguments": { "project": "nonexistent-project-xyz" }
        }
    });
    let (mut ctx, _tmp) = test_ctx();
    let resp = server.handle_request(req, &mut ctx).await.unwrap();
    let result = resp.get("result").unwrap();
    assert_eq!(result.get("content").unwrap().as_array().unwrap().len(), 1);
    let text = result["content"][0]["text"].as_str().unwrap();
    let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
    assert_eq!(parsed.get("success").unwrap(), true);
    assert!(parsed.get("repo").unwrap().is_null());
    assert!(parsed.get("vault_notes").unwrap().as_array().unwrap().is_empty());
    assert!(parsed.get("assets").unwrap().as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_tools_call_devkit_arxiv_fetch() {
    let server = build_server();
    let req = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 8,
        "method": "tools/call",
        "params": {
            "name": "devkit_arxiv_fetch",
            "arguments": { "arxiv_id": "" }
        }
    });
    let (mut ctx, _tmp) = test_ctx();
    let resp = server.handle_request(req, &mut ctx).await.unwrap();
    let result = resp.get("result").unwrap();
    let content = result.get("content").unwrap().as_array().unwrap();
    let text = content[0].get("text").unwrap().as_str().unwrap();
    let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
    // Empty arxiv_id should result in an error from the arXiv API or parser
    assert_eq!(parsed.get("success").unwrap(), false);
    assert!(!parsed.get("error").unwrap().as_str().unwrap().is_empty());
}

#[tokio::test]
async fn test_tools_call_devkit_skill_list() {
    let server = build_server();
    let req = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 9,
        "method": "tools/call",
        "params": {
            "name": "devkit_skill_list",
            "arguments": {}
        }
    });
    let (mut ctx, _tmp) = test_ctx();
    let resp = server.handle_request(req, &mut ctx).await.unwrap();
    let result = resp.get("result").unwrap();
    let content = result.get("content").unwrap().as_array().unwrap();
    let text = content[0].get("text").unwrap().as_str().unwrap();
    let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
    assert_eq!(parsed.get("success").unwrap(), true);
    assert!(parsed.get("skills").unwrap().is_array());
    assert!(parsed.get("count").unwrap().as_i64().unwrap() >= 0);
}

#[tokio::test]
async fn test_tools_call_devkit_skill_search() {
    let server = build_server();
    let req = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 10,
        "method": "tools/call",
        "params": {
            "name": "devkit_skill_search",
            "arguments": { "query": "report" }
        }
    });
    let (mut ctx, _tmp) = test_ctx();
    let resp = server.handle_request(req, &mut ctx).await.unwrap();
    let result = resp.get("result").unwrap();
    let content = result.get("content").unwrap().as_array().unwrap();
    let text = content[0].get("text").unwrap().as_str().unwrap();
    let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
    assert_eq!(parsed.get("success").unwrap(), true);
    assert!(parsed.get("skills").unwrap().is_array());
    assert!(parsed.get("count").unwrap().as_i64().unwrap() >= 0);
}

#[tokio::test]
async fn test_tools_call_devkit_skill_discover() {
    let server = build_server();
    let req = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 11,
        "method": "tools/call",
        "params": {
            "name": "devkit_skill_discover",
            "arguments": {
                "path": ".",
                "skill_id": "mcp-test-discover",
                "dry_run": true
            }
        }
    });
    // SAFETY: test-only env var mutation; test runner guarantees no concurrent
    // reads of DEVBASE_MCP_ENABLE_DESTRUCTIVE in this process.
    unsafe {
        std::env::set_var("DEVBASE_MCP_ENABLE_DESTRUCTIVE", "1");
    }
    let (mut ctx, _tmp) = test_ctx();
    let resp = server.handle_request(req, &mut ctx).await.unwrap();
    let result = resp.get("result").unwrap();
    let content = result.get("content").unwrap().as_array().unwrap();
    let text = content[0].get("text").unwrap().as_str().unwrap();
    let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
    assert_eq!(parsed.get("success").unwrap(), true);
    assert!(!parsed.get("id").unwrap().as_str().unwrap().is_empty());
    assert!(!parsed.get("name").unwrap().as_str().unwrap().is_empty());
    assert!(parsed.get("version").unwrap().as_str().is_some());
    assert!(parsed.get("category").is_some());
}

#[test]
fn test_destructive_gate_disabled_by_default() {
    // Ensure the variable is unset
    // SAFETY: test-only env var mutation; no concurrent reads of this var.
    unsafe {
        std::env::remove_var("DEVBASE_MCP_ENABLE_DESTRUCTIVE");
    }
    let result = crate::mcp::check_destructive_enabled();
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(msg.contains("DEVBASE_MCP_ENABLE_DESTRUCTIVE"));
}

#[test]
fn test_destructive_gate_enabled() {
    // SAFETY: test-only env var mutation; no concurrent reads of this var.
    unsafe {
        std::env::set_var("DEVBASE_MCP_ENABLE_DESTRUCTIVE", "1");
    }
    let result = crate::mcp::check_destructive_enabled();
    assert!(result.is_ok());
    // Cleanup
    // SAFETY: test-only env var mutation; no concurrent reads of this var.
    unsafe {
        std::env::remove_var("DEVBASE_MCP_ENABLE_DESTRUCTIVE");
    }
}

#[tokio::test]
#[ignore = "requires knowledge-report skill installed and may run external Python process"]
async fn test_tools_call_devkit_skill_run() {
    let server = build_server();
    let req = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 11,
        "method": "tools/call",
        "params": {
            "name": "devkit_skill_run",
            "arguments": {
                "skill_id": "knowledge-report",
                "args": { "repo_id": "devbase" }
            }
        }
    });
    let (mut ctx, _tmp) = test_ctx();
    let resp = server.handle_request(req, &mut ctx).await.unwrap();
    let result = resp.get("result").unwrap();
    let content = result.get("content").unwrap().as_array().unwrap();
    let text = content[0].get("text").unwrap().as_str().unwrap();
    let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
    assert_eq!(parsed.get("success").unwrap(), true);
    assert!(parsed.get("status").is_some());
    assert!(parsed.get("stdout").is_some());
}

#[tokio::test]
async fn test_stdio_content_length_format() {
    let body = serde_json::json!({ "jsonrpc": "2.0", "id": 1, "result": {} });
    let msg = format_mcp_message(&body);
    assert!(msg.starts_with("Content-Length: "));
    let parts: Vec<&str> = msg.split("\r\n\r\n").collect();
    assert_eq!(parts.len(), 2);
    let body_part = parts[1];
    // No trailing newline — Content-Length must match exact body bytes
    assert!(!body_part.ends_with("\n"));
    let parsed: serde_json::Value = serde_json::from_str(body_part).unwrap();
    assert_eq!(parsed, body);
    // Verify Content-Length header matches actual body byte count
    let header = parts[0];
    let cl_str = header.strip_prefix("Content-Length: ").unwrap();
    let cl: usize = cl_str.parse().unwrap();
    assert_eq!(cl, body_part.len());
}

static NL_FILTER_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn mock_repo(
    id: &str,
    language: Option<&str>,
    tags: Vec<&str>,
    stars: Option<u64>,
) -> crate::registry::RepoEntry {
    crate::registry::RepoEntry {
        id: id.to_string(),
        local_path: std::path::PathBuf::from(format!("/tmp/{}", id)),
        tags: tags.into_iter().map(String::from).collect(),
        discovered_at: chrono::Utc::now(),
        language: language.map(String::from),
        workspace_type: "git".to_string(),
        data_tier: "private".to_string(),
        last_synced_at: None,
        stars,
        remotes: vec![],
    }
}

#[test]
fn test_nl_filter_repos_empty_query_returns_empty() -> anyhow::Result<()> {
    let _guard = NL_FILTER_TEST_LOCK.lock().unwrap();
    let conn = crate::registry::WorkspaceRegistry::init_in_memory()?;
    let repos: Vec<crate::registry::RepoEntry> = vec![];
    let backend = crate::storage::TempStorageBackend::new();
    let index_path = backend.index_path()?;
    let searcher = crate::search::SearchClientImpl;
    let analyzer = crate::health::RepoAnalyzerImpl;
    let results = crate::mcp::tools::repo::nl_filter_repos_at(
        &index_path,
        "",
        &repos,
        &conn,
        &searcher,
        &analyzer,
    )?;
    assert!(results.is_empty());
    Ok(())
}

#[test]
fn test_nl_filter_repos_fallback_finds_by_language() -> anyhow::Result<()> {
    let _guard = NL_FILTER_TEST_LOCK.lock().unwrap();
    let conn = crate::registry::WorkspaceRegistry::init_in_memory()?;
    let repos = vec![
        mock_repo("repo1", Some("rust"), vec!["cli"], Some(10)),
        mock_repo("repo2", Some("python"), vec!["web"], Some(5)),
    ];
    let backend = crate::storage::TempStorageBackend::new();
    let index_path = backend.index_path()?;
    let searcher = crate::search::SearchClientImpl;
    let analyzer = crate::health::RepoAnalyzerImpl;
    let results = crate::mcp::tools::repo::nl_filter_repos_at(
        &index_path,
        "rust cli tool",
        &repos,
        &conn,
        &searcher,
        &analyzer,
    )?;
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, "repo1");
    Ok(())
}

#[test]
fn test_nl_filter_repos_tantivy_finds_devbase() -> anyhow::Result<()> {
    let backend = std::sync::Arc::new(crate::storage::TempStorageBackend::new());
    let index_path = backend.index_path()?;

    // Ensure DB schema exists
    let conn = crate::registry::WorkspaceRegistry::init_db_with(&*backend)?;

    // Populate Tantivy index with devbase doc
    let (index, _reader) = crate::search::init_index_at(&index_path)?;
    let mut writer = crate::search::get_writer(&index)?;
    let schema = index.schema();
    crate::search::add_repo_doc(
        &mut writer,
        &schema,
        "devbase",
        "devbase developer workspace manager",
        "rust, cli, workspace, developer",
        &["rust".to_string(), "cli".to_string()],
    )?;
    crate::search::commit_writer(&mut writer)?;

    let repos = vec![crate::registry::RepoEntry {
        id: "devbase".to_string(),
        local_path: std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")),
        tags: vec!["rust".to_string(), "cli".to_string()],
        discovered_at: chrono::Utc::now(),
        language: Some("rust".to_string()),
        workspace_type: "git".to_string(),
        data_tier: "private".to_string(),
        last_synced_at: None,
        stars: Some(10),
        remotes: vec![],
    }];

    let searcher = crate::search::SearchClientImpl;
    let analyzer = crate::health::RepoAnalyzerImpl;
    let results = crate::mcp::tools::repo::nl_filter_repos_at(
        &index_path,
        "developer workspace",
        &repos,
        &conn,
        &searcher,
        &analyzer,
    )?;
    assert!(!results.is_empty(), "tantivy path should find devbase");
    assert_eq!(results[0].id, "devbase");
    Ok(())
}

#[test]
fn test_format_mcp_message() {
    let body = serde_json::json!({"jsonrpc": "2.0", "id": 1});
    let msg = format_mcp_message(&body);
    assert!(msg.starts_with("Content-Length:"));
    assert!(msg.contains("\r\n\r\n"));
    // No trailing newline — spec-compliant MCP message ends after JSON body
    assert!(!msg.ends_with("\n"));
}

#[test]
fn test_parse_tool_tiers() {
    let tiers = parse_tool_tiers("stable,beta");
    assert!(tiers.contains(&ToolTier::Stable));
    assert!(tiers.contains(&ToolTier::Beta));
    assert!(!tiers.contains(&ToolTier::Experimental));
}

#[test]
fn test_parse_tool_tiers_empty() {
    let tiers = parse_tool_tiers("");
    assert!(tiers.is_empty());
}

// --- Claude Scenario Validation Tests ---

fn seed_scenario_data(ctx: &crate::storage::AppContext) {
    let mut conn = ctx.conn().unwrap();
    let now = chrono::Utc::now().to_rfc3339();

    // 1. Register a repo in the entities table (single source of truth)
    conn.execute(
        "INSERT INTO entities (id, entity_type, name, local_path, metadata, created_at, updated_at, language, discovered_at, workspace_type, data_tier, stars)
         VALUES (?1, 'repo', ?2, ?3, ?4, ?5, ?5, ?6, ?5, ?7, ?8, ?9)",
        rusqlite::params!["scenario-repo", "scenario-repo", "/tmp/scenario-repo", "{}", &now, "rust", "git", "private", 42i64],
    ).unwrap();

    // 2. Tags (including "managed" for is_managed coverage)
    for tag in &["rust", "cli", "managed"] {
        conn.execute(
            "INSERT INTO repo_tags (repo_id, tag) VALUES (?1, ?2)",
            rusqlite::params!["scenario-repo", tag],
        )
        .unwrap();
    }

    // 3. Code symbols: 10 entries, mix of functions and structs.
    //    Include auth-related signatures so "authentication flow" keyword search hits.
    let symbols: [(&str, &str, &str, i64, Option<&str>); 10] = [
        (
            "src/auth.rs",
            "function",
            "authenticate_user",
            10,
            Some("pub fn authenticate_user(token: &str) // authentication flow handler"),
        ),
        (
            "src/auth.rs",
            "function",
            "validate_token",
            20,
            Some("fn validate_token(t: &str) -> bool"),
        ),
        (
            "src/lib.rs",
            "function",
            "handle_error",
            30,
            Some("pub fn handle_error(e: Error)"),
        ),
        (
            "src/lib.rs",
            "function",
            "parse_config",
            40,
            Some("fn parse_config() -> Config"),
        ),
        ("src/main.rs", "function", "main", 1, Some("fn main()")),
        ("src/lib.rs", "struct", "Config", 5, None),
        ("src/models.rs", "struct", "User", 10, None),
        ("src/models.rs", "function", "new_user", 15, Some("fn new_user() -> User")),
        ("src/db.rs", "function", "connect_pool", 5, Some("fn connect_pool() -> Pool")),
        ("src/api.rs", "function", "serve", 1, Some("pub async fn serve(addr: &str)")),
    ];
    for (path, ty, name, line, sig) in &symbols {
        conn.execute(
            "INSERT INTO code_symbols (repo_id, file_path, symbol_type, name, line_start, signature)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params!["scenario-repo", path, ty, name, line, *sig],
        ).unwrap();
    }

    // 4. Vault notes: create filesystem files then scan into registry
    let ws = ctx.storage.workspace_dir().unwrap();
    let vault_dir = ws.join("vault");
    std::fs::create_dir_all(&vault_dir).unwrap();
    std::fs::write(
        vault_dir.join("auth-design.md"),
        "---\ntitle: Authentication Flow Design\nrepo: scenario-repo\ntags: [auth, design]\n---\n\nThis document describes the authentication flow for the scenario repo.\nThe authenticate_user function handles token validation.\n",
    ).unwrap();
    crate::vault::scanner::scan_vault(&mut conn, Some(&vault_dir)).unwrap();
}

#[tokio::test]
async fn test_scenario_one_project_onboarding() {
    let backend = std::sync::Arc::new(crate::storage::TempStorageBackend::new());
    let mut ctx = crate::storage::AppContext::with_storage(backend).unwrap();
    seed_scenario_data(&ctx);

    // Tool 1: devkit_health
    let health_tool = DevkitHealthTool;
    let health_result = health_tool
        .invoke(serde_json::json!({ "detail": true }), &mut ctx)
        .await
        .unwrap();
    assert_eq!(health_result.get("success").unwrap(), true);
    let summary = health_result.get("summary").unwrap();
    assert!(summary.get("total_repos").unwrap().as_i64().unwrap() >= 1);

    // Tool 2: devkit_project_brief
    let brief_tool = DevkitProjectBriefTool;
    let brief_result = brief_tool
        .invoke(serde_json::json!({ "repo_id": "scenario-repo" }), &mut ctx)
        .await
        .unwrap();
    assert_eq!(brief_result.get("success").unwrap(), true);
    let brief = brief_result.get("brief").unwrap().as_str().unwrap();
    // Acceptance: brief contains >= 5 key modules/symbols
    let symbol_count = brief.matches("- `").count();
    assert!(
        symbol_count >= 5,
        "Expected >= 5 symbols in brief, found {}. Brief:\n{}",
        symbol_count,
        brief
    );
    assert!(brief.contains("## Architecture"));
    assert!(brief.contains("Key Symbols:"));

    // Tool 3: devkit_query_repos
    let query_tool = DevkitQueryReposTool;
    let query_result = query_tool.invoke(serde_json::json!({}), &mut ctx).await.unwrap();
    assert_eq!(query_result.get("success").unwrap(), true);
    let repos = query_result.get("repos").unwrap().as_array().unwrap();
    assert!(
        repos
            .iter()
            .any(|r| r.get("id").and_then(|v| v.as_str()) == Some("scenario-repo")),
        "scenario-repo should be listed in query_repos"
    );
}

#[tokio::test]
async fn test_scenario_two_semantic_exploration() {
    let backend = std::sync::Arc::new(crate::storage::TempStorageBackend::new());
    let mut ctx = crate::storage::AppContext::with_storage(backend).unwrap();
    seed_scenario_data(&ctx);

    // Tool 1: devkit_hybrid_search — keyword fallback path (no embeddings seeded)
    let search_tool = DevkitHybridSearchTool;
    let search_result = search_tool
        .invoke(
            serde_json::json!({ "repo_id": "scenario-repo", "query_text": "authentication flow", "limit": 10 }),
            &mut ctx,
        )
        .await
        .unwrap();
    assert_eq!(search_result.get("success").unwrap(), true);
    let symbols = search_result.get("symbols").unwrap().as_array().unwrap();
    assert!(
        !symbols.is_empty(),
        "hybrid_search should return at least 1 auth-related symbol via keyword fallback"
    );
    let names: Vec<&str> =
        symbols.iter().filter_map(|s| s.get("name").and_then(|v| v.as_str())).collect();
    assert!(
        names.contains(&"authenticate_user"),
        "authenticate_user should appear in hybrid_search results for 'authentication flow'. Got: {:?}",
        names
    );

    // Tool 2: devkit_project_context
    let context_tool = DevkitProjectContextTool;
    let ctx_result = context_tool
        .invoke(serde_json::json!({ "project": "scenario-repo" }), &mut ctx)
        .await
        .unwrap();
    assert_eq!(ctx_result.get("success").unwrap(), true);
    let ctx_symbols = ctx_result.get("symbols").unwrap().as_array().unwrap();
    assert!(
        ctx_symbols.len() >= 3,
        "project_context should return >= 3 symbols for understanding. Got: {}",
        ctx_symbols.len()
    );

    // Tool 3: devkit_vault_search
    let vault_tool = DevkitVaultSearchTool;
    let vault_result = vault_tool
        .invoke(serde_json::json!({ "query": "authentication" }), &mut ctx)
        .await
        .unwrap();
    assert_eq!(vault_result.get("success").unwrap(), true);
    let notes = vault_result.get("notes").unwrap().as_array().unwrap();
    assert!(!notes.is_empty(), "vault_search should find the auth-design note");
    assert!(
        notes
            .iter()
            .any(|n| n.get("title").and_then(|v| v.as_str()) == Some("Authentication Flow Design")),
        "vault_search should return auth-design note"
    );
}
