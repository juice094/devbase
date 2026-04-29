use super::interpolate::{InterpolationContext, interpolate, interpolate_value};
use super::model::{
    ErrorPolicy, ExecutionStatus, StepDefinition, StepResult, StepType, WorkflowDefinition,
};
use super::scheduler::build_schedule;
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Execute a workflow, returning the final step results.
pub fn execute_workflow(
    conn: &rusqlite::Connection,
    pool: &r2d2::Pool<r2d2_sqlite::SqliteConnectionManager>,
    wf: &WorkflowDefinition,
    inputs: HashMap<String, String>,
) -> anyhow::Result<HashMap<String, StepResult>> {
    let batches = build_schedule(wf)?;
    let mut ctx = InterpolationContext::with_inputs(inputs);
    let mut all_results: HashMap<String, StepResult> = HashMap::new();

    for batch in &batches {
        // Execute independent steps within each batch in parallel.
        // Error handling (retry/fallback) remains sequential to preserve ordering.
        let ctx_arc = std::sync::Arc::new(ctx);
        let batch_results: Vec<(StepDefinition, StepResult)> = std::thread::scope(|s| {
            let mut handles = Vec::with_capacity(batch.len());
            for step in batch {
                let step = step.clone();
                let ctx_ref = std::sync::Arc::clone(&ctx_arc);
                handles.push(s.spawn(move || {
                    let conn = pool.get().map_err(|e| {
                        anyhow::anyhow!("db open failed for step '{}': {}", step.id, e)
                    })?;
                    let result = execute_step(&conn, pool, &step, ctx_ref.as_ref())?;
                    Ok::<_, anyhow::Error>((step, result))
                }));
            }
            let mut results = Vec::with_capacity(handles.len());
            for handle in handles {
                match handle.join() {
                    Ok(Ok(r)) => results.push(r),
                    Ok(Err(e)) => return Err(e),
                    Err(_) => return Err(anyhow::anyhow!("step thread panicked")),
                }
            }
            Ok(results)
        })?;
        ctx = std::sync::Arc::try_unwrap(ctx_arc)
            .map_err(|_| anyhow::anyhow!("parallel step references leaked"))?;

        // Sequential post-processing: update context and handle errors
        for (step, result) in batch_results {
            let status = result.status.clone();
            let step_id = step.id.clone();

            ctx.add_step_output(&step_id, result.outputs.clone());
            all_results.insert(step_id.clone(), result);

            if status == ExecutionStatus::Failed {
                match &step.on_error {
                    ErrorPolicy::Fail => {
                        return Err(anyhow::anyhow!(
                            "workflow failed at step '{step_id}' (on_error: fail)"
                        ));
                    }
                    ErrorPolicy::Continue => {
                        // proceed
                    }
                    ErrorPolicy::Retry { count, backoff_ms } => {
                        let mut retry_ok = false;
                        for i in 0..*count {
                            std::thread::sleep(Duration::from_millis(*backoff_ms));
                            let retry_result = execute_step(conn, pool, &step, &ctx)?;
                            if retry_result.status != ExecutionStatus::Failed {
                                ctx.add_step_output(&step_id, retry_result.outputs.clone());
                                all_results.insert(step_id.clone(), retry_result);
                                retry_ok = true;
                                break;
                            }
                            if i == count - 1 {
                                all_results.insert(step_id.clone(), retry_result);
                            }
                        }
                        if !retry_ok && matches!(step.on_error, ErrorPolicy::Retry { .. }) {
                            return Err(anyhow::anyhow!(
                                "workflow failed at step '{step_id}' after retries"
                            ));
                        }
                    }
                    ErrorPolicy::Fallback { step_id: fallback_id } => {
                        if let Some(fb_step) = wf.steps.iter().find(|s| s.id == *fallback_id) {
                            let fb_result = execute_step(conn, pool, fb_step, &ctx)?;
                            ctx.add_step_output(&step_id, fb_result.outputs.clone());
                            all_results.insert(step_id.clone(), fb_result);
                        } else {
                            return Err(anyhow::anyhow!(
                                "fallback step '{fallback_id}' not found for step '{step_id}'"
                            ));
                        }
                    }
                }
            }
        }
    }

    Ok(all_results)
}

fn execute_step(
    conn: &rusqlite::Connection,
    pool: &r2d2::Pool<r2d2_sqlite::SqliteConnectionManager>,
    step: &StepDefinition,
    ctx: &InterpolationContext,
) -> anyhow::Result<StepResult> {
    let started = chrono::Local::now().to_rfc3339();
    let start = Instant::now();

    let result = match &step.step_type {
        StepType::Skill { skill } => execute_skill_step(conn, step, skill, ctx),
        StepType::Subworkflow { workflow } => execute_subworkflow_step(conn, pool, step, workflow, ctx),
        StepType::Parallel { parallel } => execute_parallel_step(conn, pool, step, parallel, ctx),
        StepType::Condition { r#if } => execute_condition_step(step, r#if, ctx),
        StepType::Loop { for_each, body } => execute_loop_step(conn, pool, step, for_each, body, ctx),
    };

    let _duration = start.elapsed();
    let finished = chrono::Local::now().to_rfc3339();

    match result {
        Ok(mut r) => {
            r.started_at = Some(started);
            r.finished_at = Some(finished);
            Ok(r)
        }
        Err(e) => Ok(StepResult {
            step_id: step.id.clone(),
            status: ExecutionStatus::Failed,
            outputs: HashMap::new(),
            stdout: None,
            stderr: Some(e.to_string()),
            started_at: Some(started),
            finished_at: Some(finished),
            error: Some(e.to_string()),
        }),
    }
}

fn execute_skill_step(
    conn: &rusqlite::Connection,
    step: &StepDefinition,
    skill_id: &str,
    ctx: &InterpolationContext,
) -> anyhow::Result<StepResult> {
    let skill = crate::skill_runtime::registry::get_skill(conn, skill_id)?
        .ok_or_else(|| anyhow::anyhow!("skill '{skill_id}' not found in registry"))?;

    // Interpolate inputs
    let mut args: Vec<String> = Vec::new();
    for (key, raw_val) in &step.inputs {
        let interpolated = interpolate_value(raw_val, ctx)?;
        let val_str = match interpolated {
            serde_yaml::Value::String(s) => s,
            other => serde_yaml::to_string(&other)?.trim().to_string(),
        };
        args.push(format!("{key}={val_str}"));
    }

    let timeout = step
        .timeout_seconds
        .map(Duration::from_secs)
        .unwrap_or(Duration::from_secs(300));

    let exec_result = crate::skill_runtime::executor::run_skill(conn, &skill, &args, timeout)?;

    // Parse stdout as JSON if possible to extract structured outputs
    let mut outputs: HashMap<String, serde_json::Value> = HashMap::new();
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&exec_result.stdout) {
        if let serde_json::Value::Object(map) = json {
            for (k, v) in map {
                outputs.insert(k, v);
            }
        } else {
            outputs.insert("result".to_string(), json);
        }
    } else {
        outputs.insert("stdout".to_string(), serde_json::Value::String(exec_result.stdout.clone()));
    }

    let status = match exec_result.status {
        crate::skill_runtime::ExecutionStatus::Success => ExecutionStatus::Completed,
        _ => ExecutionStatus::Failed,
    };
    let error = if status == ExecutionStatus::Failed {
        Some(format!("exit_code={:?}", exec_result.exit_code))
    } else {
        None
    };

    Ok(StepResult {
        step_id: step.id.clone(),
        status,
        outputs,
        stdout: Some(exec_result.stdout),
        stderr: Some(exec_result.stderr),
        started_at: None,
        finished_at: None,
        error,
    })
}

fn execute_subworkflow_step(
    conn: &rusqlite::Connection,
    pool: &r2d2::Pool<r2d2_sqlite::SqliteConnectionManager>,
    step: &StepDefinition,
    workflow_id: &str,
    ctx: &InterpolationContext,
) -> anyhow::Result<StepResult> {
    let wf = crate::workflow::get_workflow(conn, workflow_id)?
        .ok_or_else(|| anyhow::anyhow!("subworkflow '{}' not found", workflow_id))?;

    // Build inputs from parent context via interpolation
    let mut inputs = std::collections::HashMap::new();
    for (key, raw_val) in &step.inputs {
        let interpolated = interpolate_value(raw_val, ctx)?;
        let val_str = match interpolated {
            serde_yaml::Value::String(s) => s,
            other => serde_yaml::to_string(&other)?.trim().to_string(),
        };
        inputs.insert(key.clone(), val_str);
    }

    let results = execute_workflow(conn, pool, &wf, inputs)?;

    let mut outputs = std::collections::HashMap::new();
    let mut stdout_lines = Vec::new();
    for (sub_step_id, result) in &results {
        stdout_lines.push(format!("[{}] {:?}", sub_step_id, result.status));
        if let Some(out) = result.stdout.as_ref() {
            outputs
                .insert(format!("{}.stdout", sub_step_id), serde_json::Value::String(out.clone()));
        }
    }

    let status = if results.values().any(|r| r.status == ExecutionStatus::Failed) {
        ExecutionStatus::Failed
    } else {
        ExecutionStatus::Completed
    };

    Ok(StepResult {
        step_id: step.id.clone(),
        status,
        outputs,
        stdout: Some(stdout_lines.join("\n")),
        stderr: None,
        started_at: None,
        finished_at: None,
        error: None,
    })
}

fn execute_parallel_step(
    conn: &rusqlite::Connection,
    pool: &r2d2::Pool<r2d2_sqlite::SqliteConnectionManager>,
    step: &StepDefinition,
    sub_steps: &[super::model::StepDefinition],
    ctx: &InterpolationContext,
) -> anyhow::Result<StepResult> {
    let mut sub_results = Vec::new();
    for sub_step in sub_steps {
        let result = execute_step(conn, pool, sub_step, ctx)?;
        sub_results.push((sub_step.id.clone(), result));
    }

    let mut outputs = std::collections::HashMap::new();
    let mut stdout_lines = Vec::new();
    for (id, result) in &sub_results {
        stdout_lines.push(format!("[{}] {:?}", id, result.status));
        if let Some(out) = result.stdout.as_ref() {
            outputs.insert(format!("{}.stdout", id), serde_json::Value::String(out.clone()));
        }
    }

    let status = if sub_results.iter().any(|(_, r)| r.status == ExecutionStatus::Failed) {
        ExecutionStatus::Failed
    } else {
        ExecutionStatus::Completed
    };

    Ok(StepResult {
        step_id: step.id.clone(),
        status,
        outputs,
        stdout: Some(stdout_lines.join("\n")),
        stderr: None,
        started_at: None,
        finished_at: None,
        error: None,
    })
}

fn execute_condition_step(
    step: &StepDefinition,
    expr: &str,
    ctx: &InterpolationContext,
) -> anyhow::Result<StepResult> {
    let evaluated = interpolate(expr, ctx)?;
    let is_true = !evaluated.is_empty()
        && evaluated != "false"
        && evaluated != "0"
        && evaluated != "no"
        && evaluated != "null";

    let mut outputs = std::collections::HashMap::new();
    outputs.insert("condition".to_string(), serde_json::Value::Bool(is_true));

    Ok(StepResult {
        step_id: step.id.clone(),
        status: ExecutionStatus::Completed,
        outputs,
        stdout: Some(format!("condition evaluated to: {}", is_true)),
        stderr: None,
        started_at: None,
        finished_at: None,
        error: None,
    })
}

fn execute_loop_step(
    conn: &rusqlite::Connection,
    pool: &r2d2::Pool<r2d2_sqlite::SqliteConnectionManager>,
    step: &StepDefinition,
    for_each: &str,
    body: &[StepDefinition],
    ctx: &InterpolationContext,
) -> anyhow::Result<StepResult> {
    let collection_str = interpolate(for_each, ctx)?;
    let items = parse_collection(&collection_str)?;

    let mut all_outputs: HashMap<String, serde_json::Value> = HashMap::new();
    let mut stdout_lines: Vec<String> = Vec::new();

    for (index, item) in items.iter().enumerate() {
        let mut iter_ctx = ctx.clone();
        iter_ctx.set_loop_var("item", item.clone());
        iter_ctx.set_loop_var("index", index.to_string());

        for body_step in body {
            let mut body_result = execute_step(conn, pool, body_step, &iter_ctx)?;

            if body_result.status == ExecutionStatus::Failed {
                match &body_step.on_error {
                    ErrorPolicy::Continue => {
                        // Record failure but continue to next body step
                    }
                    ErrorPolicy::Retry { count, backoff_ms } => {
                        let mut retry_ok = false;
                        for i in 0..*count {
                            std::thread::sleep(Duration::from_millis(*backoff_ms));
                            body_result = execute_step(conn, pool, body_step, &iter_ctx)?;
                            if body_result.status != ExecutionStatus::Failed {
                                retry_ok = true;
                                break;
                            }
                            if i == count - 1 {
                                // Final retry also failed: record it but do not return yet;
                                // let the outer loop handle termination based on overall policy
                            }
                        }
                        if !retry_ok {
                            return Ok(loop_failure_result(
                                step,
                                &all_outputs,
                                &stdout_lines,
                                &body_result,
                                index,
                                body_step,
                            ));
                        }
                    }
                    ErrorPolicy::Fallback { step_id: fallback_id } => {
                        if let Some(fb_step) = body.iter().find(|s| s.id == *fallback_id) {
                            body_result = execute_step(conn, pool, fb_step, &iter_ctx)?;
                            if body_result.status == ExecutionStatus::Failed {
                                return Ok(loop_failure_result(
                                    step,
                                    &all_outputs,
                                    &stdout_lines,
                                    &body_result,
                                    index,
                                    fb_step,
                                ));
                            }
                        } else {
                            return Ok(loop_failure_result(
                                step,
                                &all_outputs,
                                &stdout_lines,
                                &body_result,
                                index,
                                body_step,
                            ));
                        }
                    }
                    ErrorPolicy::Fail => {
                        return Ok(loop_failure_result(
                            step,
                            &all_outputs,
                            &stdout_lines,
                            &body_result,
                            index,
                            body_step,
                        ));
                    }
                }
            }

            // Aggregate outputs: later iterations overwrite earlier ones by default
            for (k, v) in &body_result.outputs {
                all_outputs.insert(k.clone(), v.clone());
            }

            if let Some(out) = body_result.stdout.as_ref() {
                stdout_lines.push(format!("[{index}] {out}"));
            }

            // Make body step outputs available to subsequent body steps in the same iteration
            iter_ctx.add_step_output(&body_step.id, body_result.outputs.clone());
        }
    }

    Ok(StepResult {
        step_id: step.id.clone(),
        status: ExecutionStatus::Completed,
        outputs: all_outputs,
        stdout: Some(stdout_lines.join("\n")),
        stderr: None,
        started_at: None,
        finished_at: None,
        error: None,
    })
}

fn loop_failure_result(
    step: &StepDefinition,
    all_outputs: &HashMap<String, serde_json::Value>,
    stdout_lines: &[String],
    body_result: &StepResult,
    index: usize,
    failed_step: &StepDefinition,
) -> StepResult {
    StepResult {
        step_id: step.id.clone(),
        status: ExecutionStatus::Failed,
        outputs: all_outputs.clone(),
        stdout: Some(stdout_lines.join("\n")),
        stderr: body_result.stderr.clone(),
        started_at: None,
        finished_at: None,
        error: Some(format!("loop iteration {index} failed at body step '{}'", failed_step.id)),
    }
}

fn parse_collection(s: &str) -> anyhow::Result<Vec<String>> {
    let trimmed = s.trim();
    if trimmed.starts_with('[') {
        let arr: Vec<serde_json::Value> = serde_json::from_str(trimmed)
            .map_err(|e| anyhow::anyhow!("invalid JSON array in for_each: {e}"))?;
        Ok(arr
            .into_iter()
            .map(|v| match v {
                serde_json::Value::String(st) => st,
                other => other.to_string(),
            })
            .collect())
    } else {
        Ok(trimmed
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skill_runtime::registry::install_skill;
    use crate::skill_runtime::{SkillMeta, SkillType};
    use crate::workflow::model::{ErrorPolicy, StepDefinition, StepType, WorkflowDefinition};
    use std::collections::HashMap;

    fn test_pool() -> (tempfile::TempDir, r2d2::Pool<r2d2_sqlite::SqliteConnectionManager>, rusqlite::Connection) {
        let tmp = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("DEVBASE_DATA_DIR", tmp.path()); }
        let conn = crate::registry::WorkspaceRegistry::init_db().unwrap();
        let path = crate::registry::WorkspaceRegistry::db_path().unwrap();
        let manager = r2d2_sqlite::SqliteConnectionManager::file(&path).with_init(|c| {
            c.execute("PRAGMA foreign_keys = ON", [])?;
            Ok(())
        });
        let pool = r2d2::Pool::builder().max_size(5).build(manager).unwrap();
        (tmp, pool, conn)
    }

    fn dummy_skill_meta(id: &str) -> SkillMeta {
        SkillMeta {
            id: id.to_string(),
            name: id.to_string(),
            version: "1.0.0".to_string(),
            description: "test".to_string(),
            author: None,
            tags: vec![],
            entry_script: None,
            category: None,
            skill_type: SkillType::Custom,
            local_path: std::path::PathBuf::from(format!("/tmp/{}", id)),
            inputs: vec![],
            outputs: vec![],
            dependencies: vec![],
            embedding: None,
            installed_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            last_used_at: None,
            body: "".to_string(),
        }
    }

    fn dummy_wf(id: &str, steps: Vec<StepDefinition>) -> WorkflowDefinition {
        WorkflowDefinition {
            id: id.to_string(),
            name: id.to_string(),
            version: "0.1.0".to_string(),
            description: None,
            inputs: vec![],
            outputs: vec![],
            steps,
            output_mapping: HashMap::new(),
        }
    }

    #[test]
    fn test_condition_step_true() {
        let (_tmp, pool, conn) = test_pool();
        let wf = dummy_wf(
            "cond-test",
            vec![StepDefinition {
                id: "check".to_string(),
                step_type: StepType::Condition { r#if: "true".to_string() },
                inputs: HashMap::new(),
                depends_on: vec![],
                on_error: ErrorPolicy::Fail,
                timeout_seconds: None,
            }],
        );
        let results = execute_workflow(&conn, &pool, &wf, HashMap::new()).unwrap();
        assert_eq!(results["check"].status, ExecutionStatus::Completed);
        assert_eq!(results["check"].outputs.get("condition"), Some(&serde_json::Value::Bool(true)));
    }

    #[test]
    fn test_condition_step_false() {
        let (_tmp, pool, conn) = test_pool();
        let wf = dummy_wf(
            "cond-test",
            vec![StepDefinition {
                id: "check".to_string(),
                step_type: StepType::Condition { r#if: "false".to_string() },
                inputs: HashMap::new(),
                depends_on: vec![],
                on_error: ErrorPolicy::Fail,
                timeout_seconds: None,
            }],
        );
        let results = execute_workflow(&conn, &pool, &wf, HashMap::new()).unwrap();
        assert_eq!(results["check"].status, ExecutionStatus::Completed);
        assert_eq!(
            results["check"].outputs.get("condition"),
            Some(&serde_json::Value::Bool(false))
        );
    }

    #[test]
    fn test_parallel_step() {
        let (_tmp, pool, conn) = test_pool();
        install_skill(&conn, &dummy_skill_meta("echo-a")).unwrap();
        install_skill(&conn, &dummy_skill_meta("echo-b")).unwrap();
        let wf = dummy_wf(
            "par-test",
            vec![StepDefinition {
                id: "parallel".to_string(),
                step_type: StepType::Parallel {
                    parallel: vec![
                        StepDefinition {
                            id: "a".to_string(),
                            step_type: StepType::Skill { skill: "echo-a".to_string() },
                            inputs: HashMap::new(),
                            depends_on: vec![],
                            on_error: ErrorPolicy::Continue,
                            timeout_seconds: None,
                        },
                        StepDefinition {
                            id: "b".to_string(),
                            step_type: StepType::Skill { skill: "echo-b".to_string() },
                            inputs: HashMap::new(),
                            depends_on: vec![],
                            on_error: ErrorPolicy::Continue,
                            timeout_seconds: None,
                        },
                    ],
                },
                inputs: HashMap::new(),
                depends_on: vec![],
                on_error: ErrorPolicy::Continue,
                timeout_seconds: None,
            }],
        );
        let results = execute_workflow(&conn, &pool, &wf, HashMap::new()).unwrap();
        // Skill fails because entry script does not exist, but parallel step itself is valid
        assert!(results.contains_key("parallel"));
    }

    #[test]
    fn test_subworkflow_step() {
        let (_tmp, pool, conn) = test_pool();
        install_skill(&conn, &dummy_skill_meta("echo-sub")).unwrap();

        // Register child workflow
        let child = dummy_wf(
            "child",
            vec![StepDefinition {
                id: "sub".to_string(),
                step_type: StepType::Skill { skill: "echo-sub".to_string() },
                inputs: HashMap::new(),
                depends_on: vec![],
                on_error: ErrorPolicy::Fail,
                timeout_seconds: None,
            }],
        );
        crate::workflow::save_workflow(&conn, &child).unwrap();

        // Parent workflow references child
        let parent = dummy_wf(
            "parent",
            vec![StepDefinition {
                id: "run_child".to_string(),
                step_type: StepType::Subworkflow { workflow: "child".to_string() },
                inputs: HashMap::new(),
                depends_on: vec![],
                on_error: ErrorPolicy::Continue,
                timeout_seconds: None,
            }],
        );
        let results = execute_workflow(&conn, &pool, &parent, HashMap::new()).unwrap();
        // Child skill fails (no entry script), but subworkflow step is valid
        assert!(results.contains_key("run_child"));
    }

    #[test]
    fn test_loop_empty_collection() {
        let (_tmp, pool, conn) = test_pool();
        let wf = dummy_wf(
            "loop-empty",
            vec![StepDefinition {
                id: "loop1".to_string(),
                step_type: StepType::Loop {
                    for_each: "".to_string(),
                    body: vec![],
                },
                inputs: HashMap::new(),
                depends_on: vec![],
                on_error: ErrorPolicy::Fail,
                timeout_seconds: None,
            }],
        );
        let results = execute_workflow(&conn, &pool, &wf, HashMap::new()).unwrap();
        assert_eq!(results["loop1"].status, ExecutionStatus::Completed);
        assert!(results["loop1"].stdout.as_ref().unwrap().is_empty());
    }

    #[test]
    fn test_loop_single_iteration() {
        let (_tmp, pool, conn) = test_pool();
        let wf = dummy_wf(
            "loop-single",
            vec![StepDefinition {
                id: "loop1".to_string(),
                step_type: StepType::Loop {
                    for_each: "x".to_string(),
                    body: vec![StepDefinition {
                        id: "check".to_string(),
                        step_type: StepType::Condition { r#if: "true".to_string() },
                        inputs: HashMap::new(),
                        depends_on: vec![],
                        on_error: ErrorPolicy::Fail,
                        timeout_seconds: None,
                    }],
                },
                inputs: HashMap::new(),
                depends_on: vec![],
                on_error: ErrorPolicy::Fail,
                timeout_seconds: None,
            }],
        );
        let results = execute_workflow(&conn, &pool, &wf, HashMap::new()).unwrap();
        assert_eq!(results["loop1"].status, ExecutionStatus::Completed);
        let stdout = results["loop1"].stdout.as_ref().unwrap();
        assert!(stdout.contains("[0] condition evaluated to: true"));
    }

    #[test]
    fn test_loop_multi_iteration() {
        let (_tmp, pool, conn) = test_pool();
        let wf = dummy_wf(
            "loop-multi",
            vec![StepDefinition {
                id: "loop1".to_string(),
                step_type: StepType::Loop {
                    for_each: "a,b,c".to_string(),
                    body: vec![StepDefinition {
                        id: "check".to_string(),
                        step_type: StepType::Condition { r#if: "true".to_string() },
                        inputs: HashMap::new(),
                        depends_on: vec![],
                        on_error: ErrorPolicy::Fail,
                        timeout_seconds: None,
                    }],
                },
                inputs: HashMap::new(),
                depends_on: vec![],
                on_error: ErrorPolicy::Fail,
                timeout_seconds: None,
            }],
        );
        let results = execute_workflow(&conn, &pool, &wf, HashMap::new()).unwrap();
        assert_eq!(results["loop1"].status, ExecutionStatus::Completed);
        let stdout = results["loop1"].stdout.as_ref().unwrap();
        assert!(stdout.contains("[0] condition evaluated to: true"));
        assert!(stdout.contains("[1] condition evaluated to: true"));
        assert!(stdout.contains("[2] condition evaluated to: true"));
    }

    #[test]
    fn test_loop_failure() {
        let (_tmp, pool, conn) = test_pool();
        let wf = dummy_wf(
            "loop-fail",
            vec![StepDefinition {
                id: "loop1".to_string(),
                step_type: StepType::Loop {
                    for_each: "x".to_string(),
                    body: vec![StepDefinition {
                        id: "bad_skill".to_string(),
                        step_type: StepType::Skill {
                            skill: "nonexistent-skill".to_string(),
                        },
                        inputs: HashMap::new(),
                        depends_on: vec![],
                        on_error: ErrorPolicy::Fail,
                        timeout_seconds: None,
                    }],
                },
                inputs: HashMap::new(),
                depends_on: vec![],
                on_error: ErrorPolicy::Fail,
                timeout_seconds: None,
            }],
        );
        // When a loop body step fails and the loop step's on_error is Fail,
        // execute_workflow returns an Err (consistent with all other step types).
        let err = execute_workflow(&conn, &pool, &wf, HashMap::new()).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("workflow failed at step 'loop1'"), "unexpected error: {msg}");
    }

    #[test]
    fn test_loop_body_continue() {
        let (_tmp, pool, conn) = test_pool();
        let wf = dummy_wf(
            "loop-continue",
            vec![StepDefinition {
                id: "loop1".to_string(),
                step_type: StepType::Loop {
                    for_each: "a,b".to_string(),
                    body: vec![
                        StepDefinition {
                            id: "always_fail".to_string(),
                            step_type: StepType::Skill {
                                skill: "nonexistent-skill".to_string(),
                            },
                            inputs: HashMap::new(),
                            depends_on: vec![],
                            on_error: ErrorPolicy::Continue,
                            timeout_seconds: None,
                        },
                        StepDefinition {
                            id: "after_fail".to_string(),
                            step_type: StepType::Condition { r#if: "true".to_string() },
                            inputs: HashMap::new(),
                            depends_on: vec![],
                            on_error: ErrorPolicy::Fail,
                            timeout_seconds: None,
                        },
                    ],
                },
                inputs: HashMap::new(),
                depends_on: vec![],
                on_error: ErrorPolicy::Fail,
                timeout_seconds: None,
            }],
        );
        let results = execute_workflow(&conn, &pool, &wf, HashMap::new()).unwrap();
        assert_eq!(results["loop1"].status, ExecutionStatus::Completed);
    }

    #[test]
    fn test_loop_body_fallback() {
        let (_tmp, pool, conn) = test_pool();
        let wf = dummy_wf(
            "loop-fallback",
            vec![StepDefinition {
                id: "loop1".to_string(),
                step_type: StepType::Loop {
                    for_each: "x".to_string(),
                    body: vec![
                        StepDefinition {
                            id: "primary".to_string(),
                            step_type: StepType::Skill {
                                skill: "nonexistent-skill".to_string(),
                            },
                            inputs: HashMap::new(),
                            depends_on: vec![],
                            on_error: ErrorPolicy::Fallback {
                                step_id: "fallback".to_string(),
                            },
                            timeout_seconds: None,
                        },
                        StepDefinition {
                            id: "fallback".to_string(),
                            step_type: StepType::Condition { r#if: "true".to_string() },
                            inputs: HashMap::new(),
                            depends_on: vec![],
                            on_error: ErrorPolicy::Fail,
                            timeout_seconds: None,
                        },
                    ],
                },
                inputs: HashMap::new(),
                depends_on: vec![],
                on_error: ErrorPolicy::Fail,
                timeout_seconds: None,
            }],
        );
        let results = execute_workflow(&conn, &pool, &wf, HashMap::new()).unwrap();
        assert_eq!(results["loop1"].status, ExecutionStatus::Completed);
    }
}
