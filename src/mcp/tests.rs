use super::*;

fn test_ctx() -> (crate::storage::AppContext, tempfile::TempDir) {
    let tmp = tempfile::tempdir().unwrap();
    unsafe {
        std::env::set_var("DEVBASE_DATA_DIR", tmp.path());
    }
    let ctx = crate::storage::AppContext::with_defaults().unwrap();
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
    assert_eq!(tools.len(), 45);
    let names: Vec<&str> = tools.iter().map(|t| t.get("name").unwrap().as_str().unwrap()).collect();
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
    assert!(names.contains(&"devkit_project_context"));
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
    unsafe {
        std::env::set_var("DEVBASE_MCP_ENABLE_DESTRUCTIVE", "1");
    }
    let result = crate::mcp::check_destructive_enabled();
    assert!(result.is_ok());
    // Cleanup
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
fn test_nl_filter_repos_empty_query_returns_empty() {
    let _guard = NL_FILTER_TEST_LOCK.lock().unwrap();
    let conn = crate::registry::WorkspaceRegistry::init_in_memory().unwrap();
    let repos: Vec<crate::registry::RepoEntry> = vec![];
    let results = crate::mcp::tools::repo::nl_filter_repos("", &repos, &conn).unwrap();
    assert!(results.is_empty());
}

#[test]
fn test_nl_filter_repos_fallback_finds_by_language() {
    let _guard = NL_FILTER_TEST_LOCK.lock().unwrap();
    let conn = crate::registry::WorkspaceRegistry::init_in_memory().unwrap();
    let repos = vec![
        mock_repo("repo1", Some("rust"), vec!["cli"], Some(10)),
        mock_repo("repo2", Some("python"), vec!["web"], Some(5)),
    ];
    let results = crate::mcp::tools::repo::nl_filter_repos("rust cli tool", &repos, &conn).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, "repo1");
}

#[test]
fn test_nl_filter_repos_tantivy_finds_devbase() {
    let _guard = NL_FILTER_TEST_LOCK.lock().unwrap();
    let _search_guard = crate::search::SEARCH_TEST_LOCK.lock().unwrap();

    let tmp = tempfile::tempdir().unwrap();
    let old = std::env::var("DEVBASE_DATA_DIR").ok();
    unsafe {
        std::env::set_var("DEVBASE_DATA_DIR", tmp.path());
    }

    // Ensure DB schema exists in temp dir
    let conn = crate::registry::WorkspaceRegistry::init_in_memory().unwrap();

    // Populate Tantivy index with devbase doc
    let (index, _reader) = crate::search::init_index().unwrap();
    let mut writer = crate::search::get_writer(&index).unwrap();
    let schema = index.schema();
    crate::search::add_repo_doc(
        &mut writer,
        &schema,
        "devbase",
        "devbase developer workspace manager",
        "rust, cli, workspace, developer",
        &["rust".to_string(), "cli".to_string()],
    )
    .unwrap();
    crate::search::commit_writer(&mut writer).unwrap();

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

    let results =
        crate::mcp::tools::repo::nl_filter_repos("developer workspace", &repos, &conn).unwrap();
    assert!(!results.is_empty(), "tantivy path should find devbase");
    assert_eq!(results[0].id, "devbase");

    if let Some(v) = old {
        unsafe {
            std::env::set_var("DEVBASE_DATA_DIR", v);
        }
    } else {
        unsafe {
            std::env::remove_var("DEVBASE_DATA_DIR");
        }
    }
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
