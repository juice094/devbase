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
        .env("DEVBASE_REGISTRY_PATH", registry_db_path()?)
        .env("DEVBASE_SKILL_ID", &skill.id)
        .env("DEVBASE_HOME", devbase_home()?);

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

/// Check for unresolved hard vetoes before skill execution.
/// Returns an optional warning string if unresolved hard vetoes exist.
/// Logs to oplog and gracefully handles registry unavailability.
pub(crate) fn check_hard_vetoes_for_skill(
    skill: &SkillRow,
    conn: &rusqlite::Connection,
) -> Option<String> {
    let vetoes = match crate::registry::WorkspaceRegistry::list_known_limits(
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
    let _ = crate::registry::WorkspaceRegistry::save_oplog(
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

#[cfg(windows)]
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

#[cfg(unix)]
fn wait_with_timeout(
    child: &mut std::process::Child,
    timeout: Duration,
) -> anyhow::Result<Option<std::process::ExitStatus>> {
    use std::os::unix::process::ExitStatusExt;
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
        let result = run_skill(&conn, &skill, &[], std::time::Duration::from_secs(5)).unwrap();
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
        crate::registry::WorkspaceRegistry::seed_hard_vetoes(&conn).unwrap();

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
}
