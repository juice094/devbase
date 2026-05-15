// SPDX-License-Identifier: MIT
// Copyright (c) 2026 juice094
use super::{ExecutionResult, ExecutionStatus, SkillRow};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Run a skill's entry script with the given arguments.
///
/// The skill directory is used as the working directory.
/// Environment variables `DEVBASE_REGISTRY_PATH`, `DEVBASE_SKILL_ID`, and `DEVBASE_HOME`
/// are injected automatically.
pub fn run_skill(
    conn: &rusqlite::Connection,
    skill: &SkillRow,
    args: &[String],
    timeout: Duration,
) -> anyhow::Result<ExecutionResult> {
    // L3 Hard Veto runtime awareness: check for unresolved hard vetoes before execution
    let veto_warning = check_hard_vetoes_for_skill(skill, conn);

    let skill_dir = std::path::PathBuf::from(&skill.local_path);
    let skill_dir = std::env::current_dir()
        .ok()
        .and_then(|cwd| cwd.join(&skill_dir).canonicalize().ok())
        .unwrap_or_else(|| skill_dir.clone());
    let entry = skill.entry_script.as_deref().unwrap_or("scripts/run.py");
    let script_path = skill_dir.join(entry);

    if !script_path.exists() {
        return Ok(ExecutionResult {
            skill_id: skill.id.clone(),
            status: ExecutionStatus::Failed,
            stdout: String::new(),
            stderr: format!("Entry script not found: {}", script_path.display()),
            exit_code: Some(127),
            duration_ms: 0,
        });
    }

    let (interpreter, arg0) = resolve_interpreter(&script_path);

    let mut cmd = if let Some(interp) = interpreter {
        let mut c = Command::new(interp);
        c.arg(&arg0);
        c
    } else {
        Command::new(&arg0)
    };

    cmd.current_dir(&skill_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env_clear();

    // Whitelist essential system environment variables for cross-platform execution
    let allowed_env = ["PATH", "PATHEXT", "TEMP", "TMP", "SystemRoot", "SYSTEMROOT", "windir"];
    for key in &allowed_env {
        if let Ok(val) = std::env::var(key) {
            cmd.env(key, val);
        }
    }

    // Inject devbase-specific variables explicitly
    cmd.env("DEVBASE_REGISTRY_PATH", registry_db_path()?);
    cmd.env("DEVBASE_SKILL_ID", &skill.id);
    cmd.env("DEVBASE_HOME", devbase_home()?);

    // P2-B: Inject active session context memories via semantic recall.
    // v0.17.0: devbase no longer generates embeddings. Semantic recall requires
    // either the `llm-backend` feature or an external endpoint configured in
    // config.toml. Falls back to keyword search when embedding unavailable.
    if let Some(ctx_id) = crate::registry::agent_context::resolve_active_context() {
        cmd.env("DEVBASE_ACTIVE_CONTEXT", &ctx_id);

        let recalled = recall_context_memories(conn, &ctx_id, &skill.id, args);
        if let Ok((memories, recall_method)) = recalled {
            let mem_json: Vec<serde_json::Value> = memories
                .iter()
                .map(|(m, score)| {
                    serde_json::json!({
                        "id": m.id,
                        "type": m.memory_type,
                        "content": m.content,
                        "score": score,
                        "model": m.embedding_model,
                    })
                })
                .collect();
            cmd.env(
                "DEVBASE_CONTEXT_MEMORIES",
                serde_json::to_string(&mem_json).unwrap_or_default(),
            );
            cmd.env("DEVBASE_CONTEXT_MEMORY_COUNT", memories.len().to_string());
            cmd.env("DEVBASE_CONTEXT_RECALL_METHOD", recall_method);
        }

        if let Ok(linked) = crate::registry::agent_context::list_linked_entities(conn, &ctx_id)
            && !linked.is_empty()
        {
            let links_json: Vec<serde_json::Value> = linked
                .iter()
                .map(|(eid, ltype, _)| {
                    serde_json::json!({
                        "entity_id": eid,
                        "link_type": ltype,
                    })
                })
                .collect();
            cmd.env(
                "DEVBASE_CONTEXT_LINKS",
                serde_json::to_string(&links_json).unwrap_or_default(),
            );
        }
    }

    // Build JSON input from key=value args and pass via stdin
    let mut json_args = serde_json::Map::new();
    for arg in args {
        if let Some((k, v)) = arg.split_once('=') {
            json_args.insert(k.to_string(), serde_json::Value::String(v.to_string()));
        } else {
            json_args.insert("command".to_string(), serde_json::Value::String(arg.to_string()));
        }
    }
    let json_input = serde_json::Value::Object(json_args).to_string();
    cmd.stdin(Stdio::piped());

    let start = Instant::now();
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            return Ok(ExecutionResult {
                skill_id: skill.id.clone(),
                status: ExecutionStatus::Failed,
                stdout: String::new(),
                stderr: format!("Failed to spawn skill process: {}", e),
                exit_code: Some(126),
                duration_ms: start.elapsed().as_millis() as u64,
            });
        }
    };

    // Write JSON input to stdin
    if let Some(stdin) = child.stdin.take() {
        let _ = std::io::Write::write_all(&mut { stdin }, json_input.as_bytes());
    }

    // Wait with timeout
    let status = match wait_with_timeout(&mut child, timeout) {
        Ok(Some(s)) => s,
        Ok(None) => {
            let _ = child.kill();
            return Ok(ExecutionResult {
                skill_id: skill.id.clone(),
                status: ExecutionStatus::Timeout,
                stdout: String::new(),
                stderr: format!("Skill timed out after {}s", timeout.as_secs()),
                exit_code: None,
                duration_ms: start.elapsed().as_millis() as u64,
            });
        }
        Err(e) => {
            return Ok(ExecutionResult {
                skill_id: skill.id.clone(),
                status: ExecutionStatus::Failed,
                stdout: String::new(),
                stderr: format!("Process wait error: {}", e),
                exit_code: Some(1),
                duration_ms: start.elapsed().as_millis() as u64,
            });
        }
    };

    let stdout = child
        .stdout
        .take()
        .and_then(|mut o| {
            let mut s = String::new();
            std::io::Read::read_to_string(&mut o, &mut s).ok()?;
            Some(s)
        })
        .unwrap_or_default();
    let stderr = child
        .stderr
        .take()
        .and_then(|mut o| {
            let mut s = String::new();
            std::io::Read::read_to_string(&mut o, &mut s).ok()?;
            Some(s)
        })
        .unwrap_or_default();

    let exit_code = status.code();
    let exec_status = if exit_code == Some(0) {
        ExecutionStatus::Success
    } else {
        ExecutionStatus::Failed
    };

    let stderr = if let Some(ref warning) = veto_warning {
        format!("[HARD-VETO-WARNING] {}\n{}", warning, stderr)
    } else {
        stderr
    };

    Ok(ExecutionResult {
        skill_id: skill.id.clone(),
        status: exec_status,
        stdout,
        stderr,
        exit_code,
        duration_ms: start.elapsed().as_millis() as u64,
    })
}

// ---------------------------------------------------------------------------
// Context Memory Auto-Recall (v0.17.0)
// ---------------------------------------------------------------------------

/// Recall relevant memories for the active context using a tiered strategy:
/// 1. Semantic recall (cosine similarity) if embedding is available
/// 2. Keyword fallback (LIKE search) otherwise
///
/// Returns (memories with scores, recall_method label).
fn recall_context_memories(
    conn: &rusqlite::Connection,
    context_id: &str,
    skill_id: &str,
    args: &[String],
) -> anyhow::Result<(Vec<(crate::registry::agent_context::AgentMemory, f64)>, String)> {
    let query_text = build_recall_query(skill_id, args);

    // Tier 1: semantic recall
    if let Ok(embedding) = generate_query_embedding_external(&query_text)
        && let Ok(results) = try_semantic_recall(conn, context_id, &embedding)
        && !results.is_empty()
    {
        return Ok((results, "semantic".to_string()));
    }

    // Tier 2: keyword fallback
    let keywords =
        crate::registry::agent_context::search_memories(conn, Some(context_id), &query_text, 5)?;
    let scored = keywords.into_iter().map(|m| (m, 0.0)).collect();
    Ok((scored, "keyword".to_string()))
}

fn build_recall_query(skill_id: &str, args: &[String]) -> String {
    let mut parts = vec![skill_id.to_string()];
    for arg in args {
        if let Some((k, v)) = arg.split_once('=') {
            parts.push(format!("{}:{}", k, v));
        } else {
            parts.push(arg.to_string());
        }
    }
    parts.join(" ")
}

fn try_semantic_recall(
    conn: &rusqlite::Connection,
    context_id: &str,
    embedding: &[f32],
) -> anyhow::Result<Vec<(crate::registry::agent_context::AgentMemory, f64)>> {
    crate::registry::agent_context::register_vector_functions(conn)?;
    crate::registry::agent_context::search_memories_semantic(conn, context_id, embedding, 5)
}

/// Generate a query embedding using the best available provider.
#[cfg(feature = "embedding")]
fn generate_query_embedding_external(text: &str) -> anyhow::Result<Vec<f32>> {
    crate::embedding::generate_query_embedding(text)
}

#[cfg(not(feature = "embedding"))]
fn generate_query_embedding_external(text: &str) -> anyhow::Result<Vec<f32>> {
    let cfg = crate::config::Config::load()?;
    if !cfg.embedding.enabled {
        anyhow::bail!("embedding provider not enabled in config.toml");
    }
    call_external_embedding_endpoint(text, &cfg.embedding)
}

#[cfg(not(feature = "embedding"))]
pub(crate) fn call_external_embedding_endpoint(
    text: &str,
    cfg: &crate::config::EmbeddingConfig,
) -> anyhow::Result<Vec<f32>> {
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(cfg.timeout_seconds))
        .build()?;

    let url = format!("{}/api/embeddings", cfg.base_url.trim_end_matches('/'));
    let body = serde_json::json!({
        "model": cfg.model,
        "prompt": text,
    });

    let resp = client.post(&url).json(&body).send()?;
    let status = resp.status();
    let resp_json: serde_json::Value = resp.json()?;

    if !status.is_success() {
        anyhow::bail!(
            "Embedding endpoint returned {}: {}",
            status,
            resp_json.get("error").and_then(|v| v.as_str()).unwrap_or("unknown error")
        );
    }

    let embedding = resp_json
        .get("embedding")
        .and_then(|v| v.as_array())
        .ok_or_else(|| anyhow::anyhow!("embedding array missing in response"))?
        .iter()
        .filter_map(|v| v.as_f64().map(|f| f as f32))
        .collect::<Vec<f32>>();

    if embedding.is_empty() {
        anyhow::bail!("embedding array empty");
    }
    Ok(embedding)
}

/// Check for unresolved hard vetoes before skill execution.
/// Returns an optional warning string if unresolved hard vetoes exist.
/// Logs to oplog and gracefully handles registry unavailability.
pub(crate) fn check_hard_vetoes_for_skill(
    skill: &SkillRow,
    conn: &rusqlite::Connection,
) -> Option<String> {
    let vetoes = match crate::registry::known_limits::list_known_limits(
        conn,
        Some("hard-veto"),
        Some(false),
    ) {
        Ok(v) => v,
        Err(_) => return None,
    };
    if vetoes.is_empty() {
        return None;
    }

    let ids: Vec<String> = vetoes.iter().map(|v| v.id.clone()).collect();
    let details = serde_json::json!({
        "action": "skill_guard",
        "skill_id": &skill.id,
        "unresolved_vetoes": ids,
        "veto_count": vetoes.len(),
    });
    let _ = crate::registry::workspace::save_oplog(
        conn,
        &crate::registry::OplogEntry {
            id: None,
            event_type: crate::registry::OplogEventType::KnownLimit,
            repo_id: None,
            details: Some(details.to_string()),
            status: "warning".to_string(),
            timestamp: chrono::Utc::now(),
            duration_ms: None,
            event_version: 1,
        },
    );

    let descriptions: Vec<String> =
        vetoes.iter().map(|v| format!("- [{}] {}", v.id, v.description)).collect();
    Some(format!(
        "Skill '{}' executed with {} unresolved hard veto(s):\n{}",
        skill.id,
        vetoes.len(),
        descriptions.join("\n")
    ))
}

fn resolve_interpreter(path: &std::path::Path) -> (Option<String>, String) {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    let path_str = path.to_string_lossy().to_string();
    match ext {
        "py" => {
            let candidates = if cfg!(windows) {
                vec!["python", "python3", "py"]
            } else {
                vec!["python3", "python"]
            };
            let found = candidates.into_iter().find(|c| which::which(c).is_ok());
            (found.map(|c| c.to_string()), path_str)
        }
        "sh" => {
            let candidates = if cfg!(windows) {
                vec!["bash", "sh", "cmd"]
            } else {
                vec!["bash", "sh"]
            };
            let found = candidates.into_iter().find(|c| which::which(c).is_ok());
            (found.map(|c| c.to_string()), path_str)
        }
        "ps1" => (Some("powershell".to_string()), path_str),
        "js" => {
            let found = which::which("node").ok().map(|_| "node".to_string());
            (found, path_str)
        }
        _ => (None, path_str),
    }
}

fn registry_db_path() -> anyhow::Result<String> {
    let path = crate::registry::WorkspaceRegistry::db_path()?;
    Ok(path.to_string_lossy().to_string())
}

fn devbase_home() -> anyhow::Result<String> {
    let path = crate::registry::WorkspaceRegistry::workspace_dir()?;
    Ok(path.to_string_lossy().to_string())
}

fn wait_with_timeout(
    child: &mut std::process::Child,
    timeout: Duration,
) -> anyhow::Result<Option<std::process::ExitStatus>> {
    let start = Instant::now();
    loop {
        match child.try_wait()? {
            Some(status) => return Ok(Some(status)),
            None => {
                if start.elapsed() >= timeout {
                    return Ok(None);
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_interpreter_python() {
        let path = std::path::PathBuf::from("scripts/run.py");
        let (interp, arg0) = super::resolve_interpreter(&path);
        assert_eq!(interp, Some("python".to_string()));
        assert_eq!(arg0, "scripts/run.py");
    }

    #[test]
    fn test_resolve_interpreter_shell() {
        let path = std::path::PathBuf::from("scripts/run.sh");
        let (interp, arg0) = super::resolve_interpreter(&path);
        assert_eq!(interp, Some("bash".to_string()));
        assert_eq!(arg0, "scripts/run.sh");
    }

    #[test]
    fn test_resolve_interpreter_powershell() {
        let path = std::path::PathBuf::from("scripts/run.ps1");
        let (interp, arg0) = super::resolve_interpreter(&path);
        assert_eq!(interp, Some("powershell".to_string()));
        assert_eq!(arg0, "scripts/run.ps1");
    }

    #[test]
    fn test_resolve_interpreter_binary() {
        let path = std::path::PathBuf::from("bin/my-tool");
        let (interp, arg0) = super::resolve_interpreter(&path);
        assert_eq!(interp, None);
        assert_eq!(arg0, "bin/my-tool");
    }

    #[test]
    fn test_run_skill_success() {
        let dir = std::env::temp_dir().join("test-skill-run");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("scripts")).unwrap();

        // Create a simple Python script
        #[cfg(windows)]
        let script = "scripts/run.py";
        #[cfg(unix)]
        let script = "scripts/run.py";

        std::fs::write(
            dir.join(script),
            r#"import sys
print("hello")
print("stderr msg", file=sys.stderr)
sys.exit(0)
"#,
        )
        .unwrap();

        let skill = SkillRow {
            id: "test-run".to_string(),
            name: "Test Run".to_string(),
            version: "1.0.0".to_string(),
            description: "test".to_string(),
            author: None,
            tags: vec![],
            entry_script: Some(script.to_string()),
            category: None,
            skill_type: crate::skill_runtime::SkillType::Builtin,
            local_path: dir.to_string_lossy().to_string(),
            installed_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            last_used_at: None,
            dependencies: vec![],
        };

        let conn = crate::registry::WorkspaceRegistry::init_in_memory().unwrap();
        let result = run_skill(&conn, &skill, &[], std::time::Duration::from_secs(30)).unwrap();
        assert_eq!(result.status, ExecutionStatus::Success);
        assert_eq!(result.exit_code, Some(0));
        assert!(result.stdout.contains("hello"));
        assert!(result.stderr.contains("stderr msg"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_run_skill_not_found() {
        let skill = SkillRow {
            id: "missing".to_string(),
            name: "Missing".to_string(),
            version: "1.0.0".to_string(),
            description: "test".to_string(),
            author: None,
            tags: vec![],
            entry_script: Some("scripts/nonexistent.py".to_string()),
            category: None,
            skill_type: crate::skill_runtime::SkillType::Builtin,
            local_path: std::env::temp_dir().to_string_lossy().to_string(),
            installed_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            last_used_at: None,
            dependencies: vec![],
        };

        let conn = crate::registry::WorkspaceRegistry::init_in_memory().unwrap();
        let result = run_skill(&conn, &skill, &[], std::time::Duration::from_secs(5)).unwrap();
        assert_eq!(result.status, ExecutionStatus::Failed);
        assert_eq!(result.exit_code, Some(127));
    }

    #[test]
    fn test_hard_veto_guard_with_unresolved_vetoes() {
        let conn = crate::registry::WorkspaceRegistry::init_in_memory().unwrap();
        crate::registry::known_limits::seed_hard_vetoes(&conn).unwrap();

        let skill = SkillRow {
            id: "test-guard".to_string(),
            name: "Test Guard".to_string(),
            version: "1.0.0".to_string(),
            description: "test".to_string(),
            author: None,
            tags: vec![],
            entry_script: None,
            category: None,
            skill_type: crate::skill_runtime::SkillType::Builtin,
            local_path: std::env::temp_dir().to_string_lossy().to_string(),
            installed_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            last_used_at: None,
            dependencies: vec![],
        };

        let warning = check_hard_vetoes_for_skill(&skill, &conn);
        assert!(warning.is_some(), "should detect unresolved hard vetoes");
        let msg = warning.unwrap();
        assert!(msg.contains("hard veto"), "warning should mention hard veto");
        assert!(msg.contains("test-guard"), "warning should mention skill id");
    }

    #[test]
    fn test_hard_veto_guard_empty_registry() {
        let conn = crate::registry::WorkspaceRegistry::init_in_memory().unwrap();

        let skill = SkillRow {
            id: "test-no-veto".to_string(),
            name: "Test No Veto".to_string(),
            version: "1.0.0".to_string(),
            description: "test".to_string(),
            author: None,
            tags: vec![],
            entry_script: None,
            category: None,
            skill_type: crate::skill_runtime::SkillType::Builtin,
            local_path: std::env::temp_dir().to_string_lossy().to_string(),
            installed_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            last_used_at: None,
            dependencies: vec![],
        };

        let warning = check_hard_vetoes_for_skill(&skill, &conn);
        assert!(warning.is_none(), "should return None when no vetoes exist");
    }

    /// End-to-end test: mock Ollama /api/embeddings endpoint and verify
    /// call_external_embedding_endpoint parses the response correctly.
    #[test]
    #[cfg(not(feature = "embedding"))]
    fn test_external_embedding_endpoint_ollama_parsing() {
        use std::io::{BufRead, BufReader, Write};
        use std::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();

        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(&stream);
            let mut headers = String::new();
            loop {
                let mut line = String::new();
                if reader.read_line(&mut line).unwrap() == 0 {
                    break;
                }
                if line == "\r\n" {
                    break;
                }
                headers.push_str(&line);
            }
            // Verify it hit the correct path
            assert!(
                headers.contains("POST /api/embeddings"),
                "expected POST /api/embeddings, got: {}",
                headers
            );

            let body = r#"{"embedding":[0.1,0.2,0.3]}"#;
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/json\r\n\r\n{}",
                body.len(),
                body
            );
            stream.write_all(response.as_bytes()).unwrap();
            stream.flush().unwrap();
        });

        let cfg = crate::config::EmbeddingConfig {
            enabled: true,
            provider: "ollama".to_string(),
            model: "test-model".to_string(),
            base_url: format!("http://127.0.0.1:{}", port),
            timeout_seconds: 5,
        };

        let result = call_external_embedding_endpoint("test prompt", &cfg);
        assert!(result.is_ok(), "should parse ollama response: {:?}", result.err());
        let emb = result.unwrap();
        assert_eq!(emb.len(), 3);
        assert!((emb[0] - 0.1f32).abs() < 0.001);
        assert!((emb[1] - 0.2f32).abs() < 0.001);
        assert!((emb[2] - 0.3f32).abs() < 0.001);
    }

    /// Verify error handling when external endpoint returns non-2xx.
    #[test]
    #[cfg(not(feature = "embedding"))]
    fn test_external_embedding_endpoint_error_response() {
        use std::io::{BufRead, BufReader, Write};
        use std::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();

        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(&stream);
            loop {
                let mut line = String::new();
                if reader.read_line(&mut line).unwrap() == 0 || line == "\r\n" {
                    break;
                }
            }
            let body = r#"{"error":"model not found"}"#;
            let response = format!(
                "HTTP/1.1 404 Not Found\r\nContent-Length: {}\r\nContent-Type: application/json\r\n\r\n{}",
                body.len(),
                body
            );
            stream.write_all(response.as_bytes()).unwrap();
            stream.flush().unwrap();
        });

        let cfg = crate::config::EmbeddingConfig {
            enabled: true,
            provider: "ollama".to_string(),
            model: "missing-model".to_string(),
            base_url: format!("http://127.0.0.1:{}", port),
            timeout_seconds: 5,
        };

        let result = call_external_embedding_endpoint("test", &cfg);
        assert!(result.is_err(), "should fail on 404");
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("404"), "error should mention status code: {}", msg);
    }
}
