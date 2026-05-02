use std::path::Path;
use std::process::Command;

/// 从 Rust 项目中提取模块结构
/// - 调用 `cargo metadata --format-version 1 --manifest-path <path>/Cargo.toml --no-deps`
/// - 解析 JSON，提取每个 package 的 targets[].name 和 targets[].kind[0]
/// - 非 Rust 项目返回空 Vec
pub fn extract_module_structure(path: &Path) -> Vec<super::ModuleInfo> {
    let manifest = path.join("Cargo.toml");
    if !manifest.exists() {
        return Vec::new();
    }

    let output = Command::new("cargo")
        .args([
            "metadata",
            "--format-version",
            "1",
            "--manifest-path",
            manifest.to_string_lossy().as_ref(),
            "--no-deps",
        ])
        .output();

    let output = match output {
        Ok(o) if o.status.success() => o,
        _ => return Vec::new(),
    };

    let json: serde_json::Value = match serde_json::from_slice(&output.stdout) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };

    let mut modules = Vec::new();
    let packages = json.get("packages").and_then(|v| v.as_array()).cloned().unwrap_or_default();
    for pkg in packages {
        let targets = pkg.get("targets").and_then(|v| v.as_array()).cloned().unwrap_or_default();
        for target in targets {
            let name = target.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let kind = target
                .get("kind")
                .and_then(|v| v.as_array())
                .and_then(|arr| arr.first())
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();
            modules.push(super::ModuleInfo { name, kind });
        }
    }

    modules
}

