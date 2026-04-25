use super::{ExecutionResult, ExecutionStatus, SkillRow};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Run a skill's entry script with the given arguments.
///
/// The skill directory is used as the working directory.
/// Environment variables `DEVBASE_REGISTRY_PATH`, `DEVBASE_SKILL_ID`, and `DEVBASE_HOME`
/// are injected automatically.
pub fn run_skill(
    skill: &SkillRow,
    args: &[String],
    timeout: Duration,
) -> anyhow::Result<ExecutionResult> {
    let skill_dir = std::path::PathBuf::from(&skill.local_path);
    let skill_dir = std::env::current_dir()
        .ok()
        .and_then(|cwd| cwd.join(&skill_dir).canonicalize().ok())
        .unwrap_or_else(|| skill_dir.clone());
    let entry = skill
        .entry_script
        .as_deref()
        .unwrap_or("scripts/run.py");
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

    // Parse key=value args and pass as --kebab-case value
    for arg in args {
        if let Some((k, v)) = arg.split_once('=') {
            let flag = k.replace('_', "-");
            cmd.arg(format!("--{}", flag));
            cmd.arg(v);
        } else {
            cmd.arg(arg);
        }
    }

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

    Ok(ExecutionResult {
        skill_id: skill.id.clone(),
        status: exec_status,
        stdout,
        stderr,
        exit_code,
        duration_ms: start.elapsed().as_millis() as u64,
    })
}

fn resolve_interpreter(path: &std::path::Path) -> (Option<String>, String) {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    let path_str = path.to_string_lossy().to_string();
    match ext {
        "py" => (Some("python".to_string()), path_str),
        "sh" => {
            // On Windows, try bash from Git Bash or WSL
            if cfg!(windows) {
                (Some("bash".to_string()), path_str)
            } else {
                (Some("bash".to_string()), path_str)
            }
        }
        "ps1" => (Some("powershell".to_string()), path_str),
        "js" => (Some("node".to_string()), path_str),
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
