pub fn run_workflow(
    ctx: &mut crate::storage::AppContext,
    cmd: crate::WorkflowCommands,
) -> anyhow::Result<()> {
    let conn = ctx.conn_mut();
    match cmd {
        crate::WorkflowCommands::List => {
            let workflows = crate::workflow::list_workflows(&conn)?;
            if workflows.is_empty() {
                println!("No workflows registered.");
            } else {
                println!("Registered workflows:");
                for (id, name, version) in workflows {
                    println!("  [{}] {} (v{})", id, name, version);
                }
            }
        }
        crate::WorkflowCommands::Show { workflow_id } => {
            match crate::workflow::get_workflow(&conn, &workflow_id)? {
                Some(wf) => {
                    println!("Workflow: {} ({})", wf.name, wf.id);
                    println!("Version: {}", wf.version);
                    if let Some(desc) = &wf.description {
                        println!("Description: {}", desc);
                    }
                    println!("\nSteps:");
                    for step in &wf.steps {
                        let deps = if step.depends_on.is_empty() {
                            "".to_string()
                        } else {
                            format!(" [depends_on: {}]", step.depends_on.join(", "))
                        };
                        println!("  - {}{}", step.id, deps);
                    }
                }
                None => {
                    return Err(anyhow::anyhow!("Workflow '{}' not found", workflow_id));
                }
            }
        }
        crate::WorkflowCommands::Register { path } => {
            let p = std::path::PathBuf::from(&path);
            let wf = crate::workflow::parse_workflow_yaml(&p)?;
            crate::workflow::validate_workflow(&wf)?;
            crate::workflow::save_workflow(&conn, &wf)?;
            println!("Registered workflow '{}' ({} steps).", wf.id, wf.steps.len());
        }
        crate::WorkflowCommands::Run { workflow_id, inputs } => {
            let wf = crate::workflow::get_workflow(&conn, &workflow_id)?
                .ok_or_else(|| anyhow::anyhow!("Workflow '{}' not found", workflow_id))?;
            let mut input_map = std::collections::HashMap::new();
            for inp in inputs {
                if let Some((k, v)) = inp.split_once('=') {
                    input_map.insert(k.to_string(), v.to_string());
                } else {
                    return Err(anyhow::anyhow!(
                        "Invalid input format: '{}'. Expected key=value",
                        inp
                    ));
                }
            }
            let exec_id = crate::workflow::create_execution(
                &conn,
                &workflow_id,
                &serde_json::to_string(&input_map)?,
            )?;
            crate::workflow::update_execution(
                &conn,
                exec_id,
                &crate::workflow::ExecutionStatus::Running,
                None,
                None,
            )?;
            println!("Running workflow '{}' (execution #{})...", workflow_id, exec_id);
            match crate::workflow::execute_workflow(&conn, &wf, input_map) {
                Ok(results) => {
                    crate::workflow::update_execution(
                        &conn,
                        exec_id,
                        &crate::workflow::ExecutionStatus::Completed,
                        None,
                        None,
                    )?;
                    println!("\nWorkflow completed successfully.");
                    for (step_id, result) in &results {
                        println!("  [{}] {:?}", step_id, result.status);
                    }
                }
                Err(e) => {
                    crate::workflow::update_execution(
                        &conn,
                        exec_id,
                        &crate::workflow::ExecutionStatus::Failed,
                        None,
                        None,
                    )?;
                    return Err(anyhow::anyhow!("Workflow failed: {}", e));
                }
            }
        }
        crate::WorkflowCommands::Delete { workflow_id } => {
            if crate::workflow::delete_workflow(&conn, &workflow_id)? {
                println!("Deleted workflow '{}'.", workflow_id);
            } else {
                println!("Workflow '{}' not found.", workflow_id);
            }
        }
    }
    Ok(())
}
