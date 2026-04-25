use super::interpolate::{interpolate_value, InterpolationContext};
use super::model::{
    ErrorPolicy, ExecutionStatus, StepDefinition, StepResult, StepType, WorkflowDefinition,
};
use super::scheduler::build_schedule;
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Execute a workflow, returning the final step results.
pub fn execute_workflow(
    conn: &rusqlite::Connection,
    wf: &WorkflowDefinition,
    inputs: HashMap<String, String>,
) -> anyhow::Result<HashMap<String, StepResult>> {
    let batches = build_schedule(wf)?;
    let mut ctx = InterpolationContext::with_inputs(inputs);
    let mut all_results: HashMap<String, StepResult> = HashMap::new();

    for batch in &batches {
        // Simple sequential execution within batch for MVP;
        // parallel execution can be added with rayon/tokio later.
        for step in batch {
            let result = execute_step(conn, step, &ctx)?;
            let status = result.status.clone();
            let step_id = step.id.clone();

            // Collect outputs for downstream interpolation
            ctx.add_step_output(&step_id, result.outputs.clone());
            all_results.insert(step_id.clone(), result);

            match status {
                ExecutionStatus::Failed => match &step.on_error {
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
                            let retry_result = execute_step(conn, step, &ctx)?;
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
                        // Find fallback step in workflow and execute it
                        if let Some(fb_step) = wf.steps.iter().find(|s| s.id == *fallback_id) {
                            let fb_result = execute_step(conn, fb_step, &ctx)?;
                            ctx.add_step_output(&step_id, fb_result.outputs.clone());
                            all_results.insert(step_id.clone(), fb_result);
                        } else {
                            return Err(anyhow::anyhow!(
                                "fallback step '{fallback_id}' not found for step '{step_id}'"
                            ));
                        }
                    }
                },
                _ => {}
            }
        }
    }

    Ok(all_results)
}

fn execute_step(
    conn: &rusqlite::Connection,
    step: &StepDefinition,
    ctx: &InterpolationContext,
) -> anyhow::Result<StepResult> {
    let started = chrono::Local::now().to_rfc3339();
    let start = Instant::now();

    let result = match &step.step_type {
        StepType::Skill { skill } => execute_skill_step(conn, step, skill, ctx),
        StepType::Subworkflow { workflow: _ } => {
            Err(anyhow::anyhow!("subworkflow execution not yet implemented"))
        }
        StepType::Parallel { parallel: _ } => {
            Err(anyhow::anyhow!("parallel step execution not yet implemented"))
        }
        StepType::Condition { r#if: _ } => {
            Err(anyhow::anyhow!("condition step execution not yet implemented"))
        }
        StepType::Loop { for_each: _ } => {
            Err(anyhow::anyhow!("loop step execution not yet implemented"))
        }
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
        .map(|s| Duration::from_secs(s))
        .unwrap_or(Duration::from_secs(300));

    let exec_result = crate::skill_runtime::executor::run_skill(&skill, &args, timeout)?;

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
