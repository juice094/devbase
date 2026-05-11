// SPDX-License-Identifier: MIT
// Copyright (c) 2026 juice094

use crate::i18n::I18n;

/// Refresh environment version cache by spawning all tool subprocesses in parallel.
pub async fn refresh_env_cache() -> crate::storage::EnvVersionCache {
    let (rustc, cargo, node, go, cmake, python, bun, zig, java) = tokio::join!(
        get_tool_version("rustc", &["--version"]),
        get_tool_version("cargo", &["--version"]),
        get_tool_version("node", &["--version"]),
        get_tool_version("go", &["version"]),
        get_tool_version("cmake", &["--version"]),
        get_tool_version("python", &["--version"]),
        get_tool_version("bun", &["--version"]),
        get_tool_version("zig", &["version"]),
        get_tool_version("java", &["-version"]),
    );
    crate::storage::EnvVersionCache {
        rustc,
        cargo,
        node,
        go,
        cmake,
        python,
        bun,
        zig,
        java,
        fetched_at: Some(std::time::Instant::now()),
    }
}

async fn get_tool_version(cmd: &str, args: &[&str]) -> Option<String> {
    let output = tokio::process::Command::new(cmd).args(args).output().await.ok()?;

    if !output.status.success() {
        return None;
    }

    let raw = if !output.stdout.is_empty() {
        String::from_utf8_lossy(&output.stdout)
    } else {
        String::from_utf8_lossy(&output.stderr)
    };
    let line = raw.lines().next()?.trim();
    if line.is_empty() {
        return None;
    }
    Some(line.to_string())
}

pub fn fmt_version(raw: Option<String>, i18n: &I18n) -> String {
    match raw {
        Some(s) => {
            let s = s.trim();
            if let Some(start) = s.find('"')
                && let Some(end) = s[start + 1..].find('"')
            {
                return s[start + 1..start + 1 + end].to_string();
            }
            let parts: Vec<&str> = s.split_whitespace().collect();
            if parts.len() >= 2 {
                match parts[0] {
                    "rustc" | "cargo" | "bun" | "zig" | "Python" => {
                        parts.get(1).unwrap_or(&"unknown").to_string()
                    }
                    "cmake" | "version" => parts.get(2).unwrap_or(&"unknown").to_string(),
                    "go" if parts.len() >= 3 => parts[2].to_string(),
                    "Docker" if parts.len() >= 3 && parts[1] == "version" => parts[2..].join(" "),
                    _ => {
                        if parts.len() >= 3 && parts[1] == "version" {
                            parts[2..].join(" ")
                        } else {
                            s.to_string()
                        }
                    }
                }
            } else {
                s.to_string()
            }
        }
        None => i18n.log.not_installed.to_string(),
    }
}
