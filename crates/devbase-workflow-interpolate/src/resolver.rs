// SPDX-License-Identifier: MIT
// Copyright (c) 2026 juice094

use regex::Regex;
use serde_json::Value;
use std::sync::OnceLock;

use crate::InterpolationContext;

static VAR_RE: OnceLock<Regex> = OnceLock::new();

pub fn var_regex() -> &'static Regex {
    VAR_RE.get_or_init(|| Regex::new(r"\$\{([^}]+)\}").expect("static regex is valid"))
}

pub fn resolve(path: &str, ctx: &InterpolationContext) -> anyhow::Result<String> {
    let parts: Vec<&str> = path.split('.').collect();
    match parts.as_slice() {
        ["inputs", name] => ctx
            .inputs
            .get(*name)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("missing input: {name}")),
        ["steps", step_id, "outputs", out_name] => ctx
            .step_outputs
            .get(*step_id)
            .and_then(|m| m.get(*out_name))
            .map(json_to_string)
            .ok_or_else(|| anyhow::anyhow!("missing output {out_name} for step {step_id}")),
        ["env", name] => {
            std::env::var(*name).map_err(|_| anyhow::anyhow!("missing env var: {name}"))
        }
        ["loop", "item"] => ctx
            .loop_vars
            .get("item")
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("loop item not set")),
        ["loop", "index"] => ctx
            .loop_vars
            .get("index")
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("loop index not set")),
        _ => Err(anyhow::anyhow!("unsupported variable path: {path}")),
    }
}

pub fn json_to_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        _ => v.to_string(),
    }
}
