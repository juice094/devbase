use devbase::*;
use skill_runtime::{parser, registry};

pub fn run_skill(cmd: crate::SkillCommands) -> anyhow::Result<()> {
    let conn = crate::registry::WorkspaceRegistry::init_db()?;
    match cmd {
        crate::SkillCommands::List { skill_type, category, json } => {
            let st = skill_type.as_deref().and_then(|s| s.parse().ok());
            let cat = category.as_deref();
            let skills = registry::list_skills(&conn, st, cat)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&skills)?);
            } else {
                if skills.is_empty() {
                    println!("No skills found.");
                } else {
                    println!("{:<24} {:<10} {:<12} Description", "ID", "Type", "Version");
                    for s in &skills {
                        println!(
                            "{:<24} {:<10} {:<12} {}",
                            s.id,
                            s.skill_type.as_str(),
                            s.version,
                            s.description
                        );
                    }
                }
            }
        }
        crate::SkillCommands::Install { source, git } => {
            let is_git = git
                || source.starts_with("http://")
                || source.starts_with("https://")
                || source.starts_with("git@");
            let skill = if is_git {
                let s = registry::install_skill_from_git(&conn, &source, None)?;
                println!("Installed skill '{}' ({}) from {}", s.name, s.id, source);
                s
            } else {
                let p = std::path::PathBuf::from(&source);
                let skill_md = if p.is_dir() {
                    p.join("SKILL.md")
                } else {
                    p.clone()
                };
                if !skill_md.exists() {
                    println!("SKILL.md not found at: {}", skill_md.display());
                    return Ok(());
                }
                let s = parser::parse_skill_md(&skill_md)?;
                registry::install_skill(&conn, &s)?;
                println!("Installed skill '{}' ({})", s.name, s.id);
                s
            };
            match skill_runtime::dependency::install_missing_dependencies(
                &conn,
                &skill,
                Some(&source),
            ) {
                Ok(deps) if !deps.is_empty() => {
                    println!("  Installed dependencies: {}", deps.join(", "));
                }
                Ok(_) => {}
                Err(e) => {
                    eprintln!("Warning: failed to install dependencies: {}", e);
                }
            }
        }
        crate::SkillCommands::Uninstall { skill_id } => {
            let removed = registry::uninstall_skill(&conn, &skill_id)?;
            if removed {
                println!("Uninstalled skill '{}'.", skill_id);
            } else {
                println!("Skill '{}' not found.", skill_id);
            }
        }
        crate::SkillCommands::Info { skill_id, json } => {
            match registry::get_skill(&conn, &skill_id)? {
                Some(s) => {
                    if json {
                        println!("{}", serde_json::to_string_pretty(&s)?);
                    } else {
                        println!("ID:          {}", s.id);
                        println!("Name:        {}", s.name);
                        println!("Version:     {}", s.version);
                        println!("Type:        {}", s.skill_type.as_str());
                        println!("Author:      {}", s.author.as_deref().unwrap_or("-"));
                        println!("Tags:        {}", s.tags.join(", "));
                        println!("Path:        {}", s.local_path);
                        println!("Installed:   {}", s.installed_at.format("%Y-%m-%d %H:%M:%S"));
                        println!("Description: {}", s.description);
                    }
                }
                None => {
                    if json {
                        println!("{{\"error\":\"Skill '{}' not found\"}}", skill_id);
                    } else {
                        println!("Skill '{}' not found.", skill_id);
                    }
                }
            }
        }
        crate::SkillCommands::Search {
            query,
            semantic,
            category,
            limit,
            json,
        } => {
            let cat = category.as_deref();
            let results = if semantic {
                match crate::embedding::generate_query_embedding(&query) {
                    Ok(embedding) => {
                        registry::search_skills_semantic(&conn, &embedding, limit, cat)?
                    }
                    Err(e) => {
                        eprintln!("Warning: semantic search failed ({}), falling back to text.", e);
                        registry::search_skills_text(&conn, &query, limit, cat)?
                    }
                }
            } else {
                registry::search_skills_text(&conn, &query, limit, cat)?
            };
            if json {
                println!("{}", serde_json::to_string_pretty(&results)?);
            } else if results.is_empty() {
                println!("No skills matching '{}'.", query);
            } else {
                println!("Found {} skill(s):", results.len());
                for s in &results {
                    println!("  [{}] {} — {}", s.id, s.name, s.description);
                }
            }
        }
        crate::SkillCommands::Run { skill_id, args, timeout, json } => {
            match registry::get_skill(&conn, &skill_id)? {
                Some(skill) => {
                    match skill_runtime::dependency::resolve_dependencies(&conn, &skill_id) {
                        Ok(deps) => {
                            if !deps.is_empty() && !json {
                                println!(
                                    "Resolved {} dependency(ies): {}",
                                    deps.len(),
                                    deps.iter()
                                        .map(|d| d.id.as_str())
                                        .collect::<Vec<_>>()
                                        .join(", ")
                                );
                            }
                        }
                        Err(e) => {
                            if json {
                                println!("{{\"error\":\"Dependency resolution failed: {}\"}}", e);
                            }
                            return Err(anyhow::anyhow!("Dependency resolution failed: {}", e));
                        }
                    }
                    let exec_id = registry::record_execution_start(
                        &conn,
                        &skill_id,
                        &serde_json::to_string(&args).unwrap_or_default(),
                    )?;
                    let result = skill_runtime::executor::run_skill(
                        &skill,
                        &args,
                        std::time::Duration::from_secs(timeout),
                    )?;
                    registry::record_execution_finish(&conn, exec_id, &result)?;
                    if let Ok(scores) =
                        skill_runtime::scoring::calculate_skill_scores(&conn, &skill_id)
                    {
                        let _ =
                            skill_runtime::scoring::update_skill_scores(&conn, &skill_id, &scores);
                    }
                    if json {
                        println!("{}", serde_json::to_string_pretty(&result)?);
                    } else {
                        println!("Exit code: {:?}", result.exit_code);
                        if !result.stdout.is_empty() {
                            println!("--- stdout ---\n{}", result.stdout);
                        }
                        if !result.stderr.is_empty() {
                            eprintln!("--- stderr ---\n{}", result.stderr);
                        }
                    }
                }
                None => {
                    if json {
                        println!("{{\"error\":\"Skill '{}' not found\"}}", skill_id);
                    } else {
                        println!("Skill '{}' not found.", skill_id);
                    }
                }
            }
        }
        crate::SkillCommands::Validate { path } => {
            let p = std::path::PathBuf::from(&path);
            let skill_md = if p.is_dir() { p.join("SKILL.md") } else { p };
            match parser::parse_skill_md(&skill_md) {
                Ok(skill) => {
                    println!("✓ Valid SKILL.md: '{}' ({})", skill.name, skill.id);
                    if !skill.inputs.is_empty() {
                        println!("  Inputs:  {}", skill.inputs.len());
                    }
                    if !skill.outputs.is_empty() {
                        println!("  Outputs: {}", skill.outputs.len());
                    }
                    let missing = skill_runtime::dependency::validate_dependencies(&conn, &skill)
                        .unwrap_or_default();
                    if missing.is_empty() {
                        println!("  Dependencies: satisfied");
                    } else {
                        println!("  Dependencies: MISSING — {}", missing.join(", "));
                    }
                }
                Err(e) => {
                    return Err(anyhow::anyhow!("Invalid SKILL.md: {}", e));
                }
            }
        }
        crate::SkillCommands::Sync { target } => {
            if target != "clarity" {
                return Err(anyhow::anyhow!(
                    "Unsupported sync target: '{}'. Only 'clarity' is supported.",
                    target
                ));
            }
            let clarity_dir = std::path::PathBuf::from("C:\\Users\\22414\\.clarity");
            if !clarity_dir.exists() {
                return Err(anyhow::anyhow!(
                    "Clarity directory not found: {}",
                    clarity_dir.display()
                ));
            }
            match skill_runtime::clarity_sync::sync_skills_to_clarity(&conn, &clarity_dir) {
                Ok(count) => println!("Synced {} skill(s) to Clarity.", count),
                Err(e) => {
                    return Err(anyhow::anyhow!("Skill sync failed: {}", e));
                }
            }
        }
        crate::SkillCommands::Discover { path, skill_id, dry_run, json } => {
            let is_git_url = path.starts_with("http://")
                || path.starts_with("https://")
                || path.starts_with("git@");

            let computed_id = skill_id.clone().unwrap_or_else(|| {
                path.trim_end_matches('/')
                    .rsplit('/')
                    .next()
                    .unwrap_or("discovered-skill")
                    .trim_end_matches(".git")
                    .to_lowercase()
                    .replace('_', "-")
            });

            let project_path = if is_git_url {
                let skill_dir = crate::registry::WorkspaceRegistry::workspace_dir()?
                    .join("skills")
                    .join(&computed_id);
                if skill_dir.exists() {
                    std::fs::remove_dir_all(&skill_dir)?;
                }
                println!("Cloning {} ...", path);
                git2::Repository::clone(&path, &skill_dir)
                    .map_err(|e| anyhow::anyhow!("Git clone failed: {}", e))?;
                skill_dir
            } else {
                std::path::PathBuf::from(&path)
            };

            match skill_runtime::discover::discover_and_install(
                &conn,
                &project_path,
                skill_id.as_deref(),
                dry_run,
            ) {
                Ok(skill) => {
                    if json {
                        println!(
                            "{{\"id\":\"{}\",\"name\":\"{}\",\"version\":\"{}\",\"description\":\"{}\",\"local_path\":\"{}\"}}",
                            skill.id,
                            skill.name,
                            skill.version,
                            skill.description.replace('"', "\\\""),
                            skill.local_path.display()
                        );
                    } else {
                        println!("Discovered Skill: {} ({})", skill.name, skill.id);
                        println!("Version: {}", skill.version);
                        println!("Description: {}", skill.description);
                        println!(
                            "Entry script: {}",
                            skill.entry_script.as_deref().unwrap_or("none")
                        );
                        if dry_run {
                            println!("\n(Dry-run: no files written or registry updated)");
                        } else {
                            println!("Installed to: {}", skill.local_path.display());
                        }
                    }
                }
                Err(e) => {
                    return Err(anyhow::anyhow!("Skill discovery failed: {}", e));
                }
            }
        }
        crate::SkillCommands::RecalcScores => {
            let updated = skill_runtime::scoring::recalculate_all_skill_scores(&conn)?;
            println!("Recalculated scores for {} skill(s).", updated);
        }
        crate::SkillCommands::Top { limit } => {
            let top = skill_runtime::scoring::get_top_skills(&conn, limit)?;
            println!("Top {} skills:", top.len());
            for (i, s) in top.iter().enumerate() {
                println!(
                    "  {}. {} — rating: {:.2}, success_rate: {:.1}%, usage: {}",
                    i + 1,
                    s.name,
                    s.rating,
                    s.success_rate * 100.0,
                    s.usage_count
                );
            }
        }
        crate::SkillCommands::Recommend { category, limit } => {
            let recs = skill_runtime::scoring::recommend_skills(&conn, category.as_deref(), limit)?;
            println!("Recommended skills ({}):", recs.len());
            for s in &recs {
                println!("  [{}] {} (v{}) — rating: {:.2}", s.id, s.name, s.version, s.rating);
            }
        }
        crate::SkillCommands::Publish { path, dry_run } => {
            let p = std::path::PathBuf::from(&path);
            match skill_runtime::publish::validate_skill_for_publish(&p) {
                Ok(v) => {
                    println!("Skill: {} ({})", v.name, v.skill_id);
                    println!("Version: {}", v.version);
                    println!("Description: {}", v.description);
                    if v.is_git_repo {
                        println!(
                            "Git repo: yes (branch: {})",
                            v.git_branch.as_deref().unwrap_or("unknown")
                        );
                        if v.git_clean {
                            println!("Git status: clean");
                        } else {
                            println!("Git status: ✗ has uncommitted changes");
                        }
                    } else {
                        println!("Git repo: no (not a git repository)");
                    }
                    if dry_run {
                        println!("\nDry-run complete. No changes made.");
                    } else if v.git_clean && v.is_git_repo {
                        let tag = format!("v{}", v.version);
                        match skill_runtime::publish::create_version_tag(
                            &p,
                            &tag,
                            &format!("Release {} {}", v.name, v.version),
                        ) {
                            Ok(()) => match skill_runtime::publish::push_tag_to_remote(&p, &tag) {
                                Ok(()) => {
                                    println!("\n✓ Created and pushed tag: {}", tag);
                                    if skill_runtime::publish::has_gh_cli() {
                                        println!(
                                            "  Tip: run `gh release create {}` to create a GitHub Release.",
                                            tag
                                        );
                                    }
                                }
                                Err(e) => {
                                    println!("\n✓ Created git tag: {}", tag);
                                    return Err(anyhow::anyhow!(
                                        "Failed to push tag to remote: {}. You can push manually with: git push origin {}",
                                        e,
                                        tag
                                    ));
                                }
                            },
                            Err(e) => {
                                return Err(anyhow::anyhow!("Failed to create tag: {}", e));
                            }
                        }
                    } else {
                        return Err(anyhow::anyhow!(
                            "Cannot publish: working tree not clean or not a git repo."
                        ));
                    }
                }
                Err(e) => {
                    return Err(anyhow::anyhow!("Validation failed: {}", e));
                }
            }
        }
    }
    Ok(())
}
