use regex::Regex;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::OnceLock;

static VAR_RE: OnceLock<Regex> = OnceLock::new();

fn var_regex() -> &'static Regex {
    // TODO(veto-audit-2026-04-26): RF-6 expect — 静态正则在编译时即可验证，风险极低。可保留或改为 unwrap_or_else。
    VAR_RE.get_or_init(|| Regex::new(r"\$\{([^}]+)\}").expect("static regex is valid"))
}

/// Interpolate variables in a string using the provided context.
///
/// Supported patterns:
///   ${inputs.<name>}       → workflow inputs
///   ${steps.<id>.outputs.<name>} → step outputs
///   ${env.<NAME>}          → environment variables
///   ${config.<key>}        → devbase config (not implemented yet)
pub fn interpolate(template: &str, ctx: &InterpolationContext) -> anyhow::Result<String> {
    let re = var_regex();
    let mut result = template.to_string();
    for cap in re.captures_iter(template) {
        // TODO(veto-audit-2026-04-26): RF-6 expect — Regex capture group 0 在 matched 时必然存在，风险低。
        let full = cap.get(0).expect("capture group 0 always exists").as_str();
        // TODO(veto-audit-2026-04-26): RF-6 expect — group 1 在匹配此正则时必然存在，风险低。
        let path = cap.get(1).expect("capture group 1 exists for matched pattern").as_str();
        let value = resolve(path, ctx)?;
        result = result.replace(full, &value);
    }
    Ok(result)
}

fn resolve(path: &str, ctx: &InterpolationContext) -> anyhow::Result<String> {
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

fn json_to_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        _ => v.to_string(),
    }
}

/// Interpolate a serde_yaml::Value recursively.
pub fn interpolate_value(
    value: &serde_yaml::Value,
    ctx: &InterpolationContext,
) -> anyhow::Result<serde_yaml::Value> {
    match value {
        serde_yaml::Value::String(s) => {
            let interpolated = interpolate(s, ctx)?;
            Ok(serde_yaml::Value::String(interpolated))
        }
        serde_yaml::Value::Sequence(seq) => {
            let mut out = Vec::with_capacity(seq.len());
            for item in seq {
                out.push(interpolate_value(item, ctx)?);
            }
            Ok(serde_yaml::Value::Sequence(out))
        }
        serde_yaml::Value::Mapping(map) => {
            let mut out = serde_yaml::Mapping::with_capacity(map.len());
            for (k, v) in map {
                let k2 = interpolate_value(k, ctx)?;
                let v2 = interpolate_value(v, ctx)?;
                out.insert(k2, v2);
            }
            Ok(serde_yaml::Value::Mapping(out))
        }
        other => Ok(other.clone()),
    }
}

#[derive(Debug, Default, Clone)]
pub struct InterpolationContext {
    pub inputs: HashMap<String, String>,
    pub step_outputs: HashMap<String, HashMap<String, Value>>,
    pub loop_vars: HashMap<String, String>,
}

impl InterpolationContext {
    pub fn with_inputs(inputs: HashMap<String, String>) -> Self {
        Self {
            inputs,
            step_outputs: HashMap::new(),
            loop_vars: HashMap::new(),
        }
    }

    pub fn add_step_output(&mut self, step_id: &str, outputs: HashMap<String, Value>) {
        self.step_outputs.insert(step_id.to_string(), outputs);
    }

    pub fn set_loop_var(&mut self, key: &str, value: String) {
        self.loop_vars.insert(key.to_string(), value);
    }

    pub fn clear_loop_vars(&mut self) {
        self.loop_vars.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_interpolate_inputs() {
        let mut ctx = InterpolationContext::default();
        ctx.inputs.insert("repo_path".to_string(), "/home/dev".to_string());
        assert_eq!(interpolate("path: ${inputs.repo_path}", &ctx).unwrap(), "path: /home/dev");
    }

    #[test]
    fn test_interpolate_step_outputs() {
        let mut ctx = InterpolationContext::default();
        let mut outputs = HashMap::new();
        outputs.insert("stdout".to_string(), Value::String("ok".to_string()));
        ctx.add_step_output("lint", outputs);
        assert_eq!(interpolate("${steps.lint.outputs.stdout}", &ctx).unwrap(), "ok");
    }

    struct EnvGuard {
        key: &'static str,
        old: Option<String>,
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            match &self.old {
                // SAFETY: set_var/remove_var in tests only. Tests run single-threaded,
                // and the key is scoped to this test via EnvGuard.
                Some(v) => unsafe { std::env::set_var(self.key, v) },
                None => unsafe { std::env::remove_var(self.key) },
            }
        }
    }

    #[test]
    fn test_interpolate_env() {
        let key = "DEVBASE_TEST_VAR";
        let old = std::env::var(key).ok();
        let _guard = EnvGuard { key, old };
        // SAFETY: Test-only env mutation. Single-threaded test scope.
        unsafe {
            std::env::set_var(key, "test_value");
        }
        let ctx = InterpolationContext::default();
        assert_eq!(interpolate("${env.DEVBASE_TEST_VAR}", &ctx).unwrap(), "test_value");
    }

    #[test]
    fn test_interpolate_loop_item() {
        let mut ctx = InterpolationContext::default();
        ctx.set_loop_var("item", "repo-a".to_string());
        assert_eq!(interpolate("${loop.item}", &ctx).unwrap(), "repo-a");
    }

    #[test]
    fn test_interpolate_loop_index() {
        let mut ctx = InterpolationContext::default();
        ctx.set_loop_var("index", "2".to_string());
        assert_eq!(interpolate("${loop.index}", &ctx).unwrap(), "2");
    }

    #[test]
    fn test_interpolate_loop_missing() {
        let ctx = InterpolationContext::default();
        assert!(interpolate("${loop.item}", &ctx).is_err());
    }

    #[test]
    fn test_interpolate_value_string() {
        let mut ctx = InterpolationContext::default();
        ctx.inputs.insert("name".to_string(), "world".to_string());
        let input = serde_yaml::Value::String("hello ${inputs.name}".to_string());
        let result = interpolate_value(&input, &ctx).unwrap();
        assert_eq!(result, serde_yaml::Value::String("hello world".to_string()));
    }

    #[test]
    fn test_interpolate_value_sequence() {
        let mut ctx = InterpolationContext::default();
        ctx.inputs.insert("a".to_string(), "1".to_string());
        let input = serde_yaml::Value::Sequence(vec![
            serde_yaml::Value::String("${inputs.a}".to_string()),
            serde_yaml::Value::String("static".to_string()),
        ]);
        let result = interpolate_value(&input, &ctx).unwrap();
        assert_eq!(result.as_sequence().unwrap().len(), 2);
        assert_eq!(result.as_sequence().unwrap()[0], serde_yaml::Value::String("1".to_string()));
    }

    #[test]
    fn test_interpolate_value_mapping() {
        let mut ctx = InterpolationContext::default();
        ctx.inputs.insert("key".to_string(), "replaced".to_string());
        let mut map = serde_yaml::Mapping::new();
        map.insert(
            serde_yaml::Value::String("k".to_string()),
            serde_yaml::Value::String("${inputs.key}".to_string()),
        );
        let input = serde_yaml::Value::Mapping(map);
        let result = interpolate_value(&input, &ctx).unwrap();
        let out_map = result.as_mapping().unwrap();
        assert_eq!(
            out_map.get(serde_yaml::Value::String("k".to_string())),
            Some(&serde_yaml::Value::String("replaced".to_string()))
        );
    }
}
