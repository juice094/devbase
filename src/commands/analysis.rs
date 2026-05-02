use anyhow::Context;
use devbase::*;
use devbase::mcp::clients::RegistryClient;
use rusqlite::OptionalExtension;
use tracing::{info, warn};

pub fn run_metrics(
    ctx: &mut crate::storage::AppContext,
    repo_id: &str,
    json: bool,
) -> anyhow::Result<()> {
    if repo_id.is_empty() {
        let val = ctx.list_code_metrics()?;
        let repos = val.get("repos").and_then(|v| v.as_array()).cloned().unwrap_or_default();
        if json {
            let output: Vec<serde_json::Value> = repos
                .into_iter()
                .map(|r| {
                    serde_json::json!({
                        "repo_id": r.get("repo_id").cloned().unwrap_or(serde_json::Value::Null),
                        "total_lines": r.get("total_lines").cloned().unwrap_or(serde_json::Value::Null),
                        "source_lines": r.get("source_lines").cloned().unwrap_or(serde_json::Value::Null),
                        "test_lines": r.get("test_lines").cloned().unwrap_or(serde_json::Value::Null),
                        "comment_lines": r.get("comment_lines").cloned().unwrap_or(serde_json::Value::Null),
                        "file_count": r.get("file_count").cloned().unwrap_or(serde_json::Value::Null),
                        "language_breakdown": r.get("language_breakdown").cloned().unwrap_or(serde_json::Value::Null),
                    })
                })
                .collect();
            println!("{}", serde_json::to_string_pretty(&output)?);
        } else {
            println!("Code metrics for {} repo(s):", repos.len());
            for r in repos {
                let id = r.get("repo_id").and_then(|v| v.as_str()).unwrap_or("");
                let total = r.get("total_lines").and_then(|v| v.as_u64()).unwrap_or(0);
                let source = r.get("source_lines").and_then(|v| v.as_u64()).unwrap_or(0);
                let test = r.get("test_lines").and_then(|v| v.as_u64()).unwrap_or(0);
                let comment = r.get("comment_lines").and_then(|v| v.as_u64()).unwrap_or(0);
                let files = r.get("file_count").and_then(|v| v.as_u64()).unwrap_or(0);
                println!(
                    "  [{}] total={} source={} test={} comment={} files={}",
                    id, total, source, test, comment, files
                );
            }
        }
    } else {
        let val = ctx.get_code_metrics(repo_id)?;
        let success = val.get("success").and_then(|v| v.as_bool()).unwrap_or(false);
        if !success {
            println!("No metrics found for '{}'.", repo_id);
        } else if json {
            let output = serde_json::json!({
                "repo_id": repo_id,
                "total_lines": val.get("total_lines").cloned().unwrap_or(serde_json::Value::Null),
                "source_lines": val.get("source_lines").cloned().unwrap_or(serde_json::Value::Null),
                "test_lines": val.get("test_lines").cloned().unwrap_or(serde_json::Value::Null),
                "comment_lines": val.get("comment_lines").cloned().unwrap_or(serde_json::Value::Null),
                "file_count": val.get("file_count").cloned().unwrap_or(serde_json::Value::Null),
                "language_breakdown": val.get("language_breakdown").cloned().unwrap_or(serde_json::Value::Null),
            });
            println!("{}", serde_json::to_string_pretty(&output)?);
        } else {
            let total = val.get("total_lines").and_then(|v| v.as_u64()).unwrap_or(0);
            let source = val.get("source_lines").and_then(|v| v.as_u64()).unwrap_or(0);
            let test = val.get("test_lines").and_then(|v| v.as_u64()).unwrap_or(0);
            let comment = val.get("comment_lines").and_then(|v| v.as_u64()).unwrap_or(0);
            let files = val.get("file_count").and_then(|v| v.as_u64()).unwrap_or(0);
            println!(
                "[{}] total={} source={} test={} comment={} files={}",
                repo_id, total, source, test, comment, files
            );
        }
    }
    Ok(())
}

pub fn run_module_graph(
    ctx: &mut crate::storage::AppContext,
    repo_id: &str,
    json: bool,
) -> anyhow::Result<()> {
    if repo_id.is_empty() {
        let repos_val = ctx.list_repos(None)?;
        let repos = repos_val.get("repos").and_then(|v| v.as_array()).cloned().unwrap_or_default();
        let mut all = Vec::new();
        for repo in repos {
            let id = repo.get("id").and_then(|v| v.as_str()).unwrap_or("");
            let language = repo.get("language").and_then(|v| v.as_str()).unwrap_or("");
            if language == "Rust" {
                let mod_val = ctx.list_modules(id)?;
                let modules = mod_val.get("modules").and_then(|v| v.as_array()).cloned().unwrap_or_default();
                if !modules.is_empty() {
                    all.push((id.to_string(), modules));
                }
            }
        }
        if json {
            let out: Vec<serde_json::Value> = all
                .into_iter()
                .map(|(id, mods)| {
                    serde_json::json!({
                        "repo_id": id,
                        "modules": mods
                    })
                })
                .collect();
            println!("{}", serde_json::to_string_pretty(&out)?);
        } else {
            println!("Module graph for {} Rust repo(s):", all.len());
            for (id, mods) in all {
                println!("  [{}] {} module(s)", id, mods.len());
                for m in mods {
                    let name = m.get("name").and_then(|v| v.as_str()).unwrap_or("");
                    let ty = m.get("type").and_then(|v| v.as_str()).unwrap_or("");
                    let path = m.get("path").and_then(|v| v.as_str()).unwrap_or("");
                    println!("    {} ({})  {}", name, ty, path);
                }
            }
        }
    } else {
        let mod_val = ctx.list_modules(repo_id)?;
        let modules = mod_val.get("modules").and_then(|v| v.as_array()).cloned().unwrap_or_default();
        if json {
            println!("{}", serde_json::to_string_pretty(&serde_json::json!({
                "repo_id": repo_id,
                "modules": modules
            }))?);
        } else {
            println!("Module graph for [{}]:", repo_id);
            for m in modules {
                let name = m.get("name").and_then(|v| v.as_str()).unwrap_or("");
                let ty = m.get("type").and_then(|v| v.as_str()).unwrap_or("");
                let path = m.get("path").and_then(|v| v.as_str()).unwrap_or("");
                println!("  {} ({})  {}", name, ty, path);
            }
        }
    }
    Ok(())
}

pub fn run_call_graph(
    ctx: &mut crate::storage::AppContext,
    repo_id: &str,
    callee: Option<String>,
    caller: Option<String>,
    file: Option<String>,
    limit: usize,
    json: bool,
) -> anyhow::Result<()> {
    let callee_s = callee.as_deref().unwrap_or("");
    let caller_s = caller.as_deref().unwrap_or("");
    if callee_s.is_empty() && caller_s.is_empty() {
        anyhow::bail!("At least one of --callee or --caller must be provided");
    }
    let val = ctx.query_call_graph(
        repo_id,
        Some(callee_s).filter(|s| !s.is_empty()),
        Some(caller_s).filter(|s| !s.is_empty()),
        file.as_deref().filter(|s| !s.is_empty()),
        limit,
    )?;
    let edges = val.get("calls").and_then(|v| v.as_array()).cloned().unwrap_or_default();
    if json {
        println!("{}", serde_json::to_string_pretty(&serde_json::json!({
            "repo_id": repo_id,
            "count": edges.len(),
            "calls": edges
        }))?);
    } else {
        println!("Call graph for [{}]: {} edge(s)", repo_id, edges.len());
        for e in edges {
            let caller_file = e.get("caller_file").and_then(|v| v.as_str()).unwrap_or("");
            let caller_symbol = e.get("caller_symbol").and_then(|v| v.as_str()).unwrap_or("");
            let caller_line = e.get("caller_line").and_then(|v| v.as_i64()).unwrap_or(0);
            let callee_name = e.get("callee_name").and_then(|v| v.as_str()).unwrap_or("");
            println!(
                "  {}:{}  {} -> {}",
                caller_file, caller_line, caller_symbol, callee_name
            );
        }
    }
    Ok(())
}

pub fn run_dependency_graph(
    ctx: &mut crate::storage::AppContext,
    repo_id: &str,
    direction: &str,
    relation_type: Option<String>,
    json: bool,
) -> anyhow::Result<()> {
    let rel_filter = relation_type.as_deref().filter(|s| !s.is_empty());
    let val = ctx.query_dependencies(repo_id, direction, rel_filter)?;
    let label = val.get("label").and_then(|v| v.as_str()).unwrap_or("dependencies");
    let deps = val.get("dependencies").and_then(|v| v.as_array()).cloned().unwrap_or_default();
    if json {
        println!("{}", serde_json::to_string_pretty(&serde_json::json!({
            "repo_id": repo_id,
            "direction": direction,
            "count": deps.len(),
            "dependencies": deps
        }))?);
    } else {
        println!("{} for [{}]: {} edge(s)", label, repo_id, deps.len());
        for d in deps {
            let id = d.get("repo_id").and_then(|v| v.as_str()).unwrap_or("");
            let rel = d.get("relation_type").and_then(|v| v.as_str()).unwrap_or("");
            let conf = d.get("confidence").and_then(|v| v.as_f64()).unwrap_or(0.0);
            println!("  -> {} ({} conf={:.2})", id, rel, conf);
        }
    }
    Ok(())
}

pub fn run_code_symbols(
    ctx: &mut crate::storage::AppContext,
    repo_id: &str,
    name: Option<String>,
    symbol_type: Option<String>,
    file: Option<String>,
    limit: usize,
    json: bool,
) -> anyhow::Result<()> {
    let val = ctx.query_code_symbols(
        repo_id,
        name.as_deref().filter(|s| !s.is_empty()),
        symbol_type.as_deref().filter(|s| !s.is_empty()),
        file.as_deref().filter(|s| !s.is_empty()),
        limit,
    )?;
    let symbols = val.get("symbols").and_then(|v| v.as_array()).cloned().unwrap_or_default();
    if json {
        println!("{}", serde_json::to_string_pretty(&serde_json::json!({
            "repo_id": repo_id,
            "count": symbols.len(),
            "symbols": symbols
        }))?);
    } else {
        println!("Code symbols for [{}]: {} result(s)", repo_id, symbols.len());
        for s in symbols {
            let fp = s.get("file_path").and_then(|v| v.as_str()).unwrap_or("");
            let st = s.get("symbol_type").and_then(|v| v.as_str()).unwrap_or("");
            let n = s.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let ls = s.get("line_start").and_then(|v| v.as_i64()).unwrap_or(0);
            let sig = s.get("signature").and_then(|v| v.as_str());
            let sig_str = sig.map(|s| format!("  {}", s)).unwrap_or_default();
            println!("  {}:{} {} {} {}", fp, ls, st, n, sig_str);
        }
    }
    Ok(())
}

pub fn run_dead_code(
    ctx: &mut crate::storage::AppContext,
    repo_id: &str,
    include_pub: bool,
    limit: usize,
    json: bool,
) -> anyhow::Result<()> {
    let val = ctx.query_dead_code(repo_id, include_pub, limit)?;
    let dead = val.get("dead_functions").and_then(|v| v.as_array()).cloned().unwrap_or_default();
    if json {
        println!("{}", serde_json::to_string_pretty(&serde_json::json!({
            "repo_id": repo_id,
            "count": dead.len(),
            "dead_functions": dead
        }))?);
    } else {
        println!("Potentially dead functions in [{}]: {}", repo_id, dead.len());
        for d in dead {
            let fp = d.get("file_path").and_then(|v| v.as_str()).unwrap_or("");
            let n = d.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let line = d.get("line_start").and_then(|v| v.as_i64()).unwrap_or(0);
            let sig = d.get("signature").and_then(|v| v.as_str());
            let sig_str = sig.map(|s| format!("  {}", s)).unwrap_or_default();
            println!("  {}:{} {}{}", fp, line, n, sig_str);
        }
    }
    Ok(())
}

