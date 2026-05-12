// SPDX-License-Identifier: MIT
// Copyright (c) 2026 juice094
//! devkit_evaluate: AI self-evaluation tool inspired by Claude Computer Use.
//!
//! After the AI makes code changes, it can call this tool to automatically
//! verify correctness via cargo check / clippy / fmt / test (no-run).
//! This closes the loop: AI acts → AI evaluates → AI decides next step.

use super::super::McpTool;
use crate::storage::AppContext;
use serde_json::json;
use std::process::Command;
use std::time::Instant;

#[derive(Clone)]
pub struct DevkitEvaluateTool;

impl McpTool for DevkitEvaluateTool {
    fn name(&self) -> &'static str {
        "devkit_evaluate"
    }

    fn schema(&self) -> serde_json::Value {
        json!({
            "description": r#"Run automated quality checks and return a structured report.

Use this when the AI (or user) wants to:
- Verify that recent changes compile without errors
- Check for clippy warnings or formatting issues
- Get a quick quality assessment before committing or merging

Scopes:
- "check_only" (default, fastest): cargo check + clippy + fmt — ~10-30s
- "lib": cargo test --lib --no-run + clippy + fmt — verifies test compilation
- "full": cargo test --all-targets --no-run + clippy + fmt — verifies all targets

Returns a structured JSON report with success/failure per check and captured output snippets."#,
            "inputSchema": {
                "type": "object",
                "properties": {
                    "scope": {
                        "type": "string",
                        "enum": ["check_only", "lib", "full"],
                        "description": "Evaluation scope. Default: check_only"
                    }
                }
            }
        })
    }

    async fn invoke(
        &self,
        args: serde_json::Value,
        _ctx: &mut AppContext,
    ) -> anyhow::Result<serde_json::Value> {
        let scope = args.get("scope").and_then(|v| v.as_str()).unwrap_or("check_only");

        let start = Instant::now();

        let check = run_cargo_check()?;
        let clippy = run_cargo_clippy()?;
        let fmt = run_cargo_fmt_check()?;
        let test_compile = if scope == "lib" {
            Some(run_cargo_test_lib_no_run()?)
        } else if scope == "full" {
            Some(run_cargo_test_all_no_run()?)
        } else {
            None
        };

        let overall_success = check.success
            && clippy.success
            && fmt.success
            && test_compile.as_ref().is_none_or(|t| t.success);

        let total_ms = start.elapsed().as_millis() as i64;

        Ok(json!({
            "success": overall_success,
            "scope": scope,
            "check": check.into_json(),
            "clippy": clippy.into_json(),
            "fmt": fmt.into_json(),
            "test_compile": test_compile.map(|t| t.into_json()),
            "total_duration_ms": total_ms,
        }))
    }
}

struct CheckResult {
    success: bool,
    duration_ms: i64,
    output: String,
}

impl CheckResult {
    fn into_json(self) -> serde_json::Value {
        json!({
            "success": self.success,
            "duration_ms": self.duration_ms,
            "output_preview": truncate_output(&self.output, 2000),
        })
    }
}

fn run_cargo_check() -> anyhow::Result<CheckResult> {
    let start = Instant::now();
    let (success, output) = run_command(Command::new("cargo").args(["check", "--all-targets"]))?;
    Ok(CheckResult {
        success,
        duration_ms: start.elapsed().as_millis() as i64,
        output,
    })
}

fn run_cargo_clippy() -> anyhow::Result<CheckResult> {
    let start = Instant::now();
    let (success, output) = run_command(Command::new("cargo").args([
        "clippy",
        "--all-targets",
        "--",
        "-D",
        "warnings",
    ]))?;
    Ok(CheckResult {
        success,
        duration_ms: start.elapsed().as_millis() as i64,
        output,
    })
}

fn run_cargo_fmt_check() -> anyhow::Result<CheckResult> {
    let start = Instant::now();
    let (success, output) = run_command(Command::new("cargo").args(["fmt", "--check"]))?;
    Ok(CheckResult {
        success,
        duration_ms: start.elapsed().as_millis() as i64,
        output,
    })
}

fn run_cargo_test_lib_no_run() -> anyhow::Result<CheckResult> {
    let start = Instant::now();
    let (success, output) = run_command(Command::new("cargo").args(["test", "--lib", "--no-run"]))?;
    Ok(CheckResult {
        success,
        duration_ms: start.elapsed().as_millis() as i64,
        output,
    })
}

fn run_cargo_test_all_no_run() -> anyhow::Result<CheckResult> {
    let start = Instant::now();
    let (success, output) =
        run_command(Command::new("cargo").args(["test", "--all-targets", "--no-run"]))?;
    Ok(CheckResult {
        success,
        duration_ms: start.elapsed().as_millis() as i64,
        output,
    })
}

fn run_command(cmd: &mut Command) -> anyhow::Result<(bool, String)> {
    let output = cmd.output()?;
    let success = output.status.success();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let combined = if stdout.is_empty() {
        stderr
    } else if stderr.is_empty() {
        stdout
    } else {
        format!("{stdout}\n{stderr}")
    };
    Ok((success, combined))
}

fn truncate_output(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}\n...[truncated {} chars]", &s[..max_len], s.len() - max_len)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate_output() {
        assert_eq!(truncate_output("hello", 10), "hello");
        let long = "a".repeat(3000);
        let truncated = truncate_output(&long, 2000);
        assert!(truncated.contains("[truncated"));
        assert!(truncated.len() < 2100);
    }

    #[test]
    fn test_evaluate_tool_name() {
        let t = DevkitEvaluateTool;
        assert_eq!(t.name(), "devkit_evaluate");
    }
}
