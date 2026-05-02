use crate::tui::{App, SkillItem};

impl App {
    pub(crate) fn load_skills(&mut self) {
        let Ok(conn) = self.ctx.conn() else {
            return;
        };
        let rows = match crate::skill_runtime::registry::list_skills(&conn, None, None) {
            Ok(r) => r,
            Err(e) => {
                self.log_warn(format!("无法列出 Skills: {}", e));
                self.skill_panel.items.clear();
                self.skill_panel.selected = 0;
                self.skill_panel.list_state.select(Some(0));
                return;
            }
        };
        self.skill_panel.items = rows.into_iter().map(SkillItem::from).collect();
        self.skill_panel.selected = 0;
        self.skill_panel.list_state.select(Some(0));
        self.log_info(self.ctx.i18n.log.loaded_skills(self.skill_panel.items.len()));
    }

    pub(crate) fn next_skill(&mut self) {
        if !self.skill_panel.items.is_empty() {
            self.skill_panel.selected =
                (self.skill_panel.selected + 1) % self.skill_panel.items.len();
            self.skill_panel.list_state.select(Some(self.skill_panel.selected));
        }
    }

    pub(crate) fn previous_skill(&mut self) {
        if !self.skill_panel.items.is_empty() {
            self.skill_panel.selected = (self.skill_panel.selected + self.skill_panel.items.len()
                - 1)
                % self.skill_panel.items.len();
            self.skill_panel.list_state.select(Some(self.skill_panel.selected));
        }
    }

    pub(crate) fn jump_to_top_skill(&mut self) {
        if !self.skill_panel.items.is_empty() {
            self.skill_panel.selected = 0;
            self.skill_panel.list_state.select(Some(self.skill_panel.selected));
        }
    }

    pub(crate) fn jump_to_bottom_skill(&mut self) {
        if !self.skill_panel.items.is_empty() {
            self.skill_panel.selected = self.skill_panel.items.len() - 1;
            self.skill_panel.list_state.select(Some(self.skill_panel.selected));
        }
    }

    pub(crate) fn current_skill(&self) -> Option<&SkillItem> {
        self.skill_panel.items.get(self.skill_panel.selected)
    }

    pub(crate) fn load_workflows(&mut self) {
        let Ok(conn) = self.ctx.conn() else {
            return;
        };
        match crate::workflow::list_workflows(&conn) {
            Ok(rows) => {
                self.workflows = rows
                    .into_iter()
                    .filter_map(|(id, _, _)| {
                        crate::workflow::get_workflow(&conn, &id).ok().flatten()
                    })
                    .collect();
            }
            Err(e) => {
                self.log_warn(format!("无法列出 Workflow: {}", e));
                self.workflows.clear();
            }
        }
        self.workflow_selected = 0;
        self.workflow_list_state.select(Some(0));
        self.log_info(format!("已加载 {} 个 Workflow", self.workflows.len()));
    }

    pub(crate) fn next_workflow(&mut self) {
        if !self.workflows.is_empty() {
            self.workflow_selected = (self.workflow_selected + 1) % self.workflows.len();
            self.workflow_list_state.select(Some(self.workflow_selected));
        }
    }

    pub(crate) fn previous_workflow(&mut self) {
        if !self.workflows.is_empty() {
            self.workflow_selected =
                (self.workflow_selected + self.workflows.len() - 1) % self.workflows.len();
            self.workflow_list_state.select(Some(self.workflow_selected));
        }
    }

    pub(crate) fn current_workflow(&self) -> Option<&crate::workflow::WorkflowDefinition> {
        self.workflows.get(self.workflow_selected)
    }

    pub(crate) fn run_selected_workflow(&mut self) {
        let wf = match self.selected_workflow.clone() {
            Some(w) => w,
            None => {
                self.log_warn("未选择 Workflow".to_string());
                return;
            }
        };

        let mut inputs = std::collections::HashMap::new();
        for inp in &wf.inputs {
            if inp.required && inp.default.is_none() {
                self.log_warn(format!("Workflow '{}' 缺少必要输入: {}", wf.id, inp.name));
                return;
            }
            if let Some(default) = &inp.default {
                let val = match default {
                    serde_yaml::Value::String(s) => s.clone(),
                    other => serde_yaml::to_string(other).unwrap_or_default().trim().to_string(),
                };
                inputs.insert(inp.name.clone(), val);
            }
        }

        let tx = self.async_tx.clone();
        let pool = self.ctx.pool();
        std::thread::spawn(move || {
            let conn = match pool.get() {
                Ok(c) => c,
                Err(e) => {
                    let _ = tx.send(crate::asyncgit::AsyncNotification::WorkflowRunFinished {
                        workflow_id: wf.id.clone(),
                        results: std::collections::HashMap::new(),
                        error: Some(e.to_string()),
                    });
                    return;
                }
            };
            let result = crate::workflow::execute_workflow(&conn, &pool, &wf, inputs);
            match result {
                Ok(results) => {
                    let _ = tx.send(crate::asyncgit::AsyncNotification::WorkflowRunFinished {
                        workflow_id: wf.id,
                        results,
                        error: None,
                    });
                }
                Err(e) => {
                    let _ = tx.send(crate::asyncgit::AsyncNotification::WorkflowRunFinished {
                        workflow_id: wf.id,
                        results: std::collections::HashMap::new(),
                        error: Some(e.to_string()),
                    });
                }
            }
        });
    }

    pub(crate) fn run_selected_skill(&mut self) {
        let skill_item = match self.current_skill() {
            Some(s) => s.clone(),
            None => {
                self.log_warn("未选择 Skill".to_string());
                return;
            }
        };
        self.run_skill_item(
            skill_item,
            self.skill_panel
                .param_buffer
                .split_whitespace()
                .map(|s| s.to_string())
                .collect(),
        );
    }

    pub(crate) fn run_nlp_selected_skill(&mut self) {
        let skill_item = match self.nlp_results.get(self.nlp_selected) {
            Some(s) => s.clone(),
            None => {
                self.log_warn("未选择 NLQ 结果".to_string());
                return;
            }
        };
        self.run_skill_item(skill_item, vec![]);
    }

    fn run_skill_item(&mut self, skill_item: SkillItem, args: Vec<String>) {
        let tx = self.async_tx.clone();
        let pool = self.ctx.pool();
        std::thread::spawn(move || {
            let conn = match pool.get() {
                Ok(c) => c,
                Err(e) => {
                    let _ = tx.send(crate::asyncgit::AsyncNotification::SkillRunFinished(
                        crate::skill_runtime::ExecutionResult {
                            skill_id: skill_item.row.id.clone(),
                            status: crate::skill_runtime::ExecutionStatus::Failed,
                            stdout: String::new(),
                            stderr: e.to_string(),
                            exit_code: Some(1),
                            duration_ms: 0,
                        },
                    ));
                    return;
                }
            };
            let result = crate::skill_runtime::executor::run_skill(
                &conn,
                &skill_item.row,
                &args,
                std::time::Duration::from_secs(30),
            );
            let execution_result = match result {
                Ok(r) => r,
                Err(e) => crate::skill_runtime::ExecutionResult {
                    skill_id: skill_item.row.id.clone(),
                    status: crate::skill_runtime::ExecutionStatus::Failed,
                    stdout: String::new(),
                    stderr: e.to_string(),
                    exit_code: Some(1),
                    duration_ms: 0,
                },
            };
            let _ = tx.send(crate::asyncgit::AsyncNotification::SkillRunFinished(execution_result));
        });
    }
}
