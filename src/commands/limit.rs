pub fn run_limit(
    ctx: &mut crate::storage::AppContext,
    cmd: crate::LimitCommands,
) -> anyhow::Result<()> {
    let conn = ctx.conn_mut()?;
    match cmd {
        crate::LimitCommands::Add {
            id,
            category,
            description,
            source,
            severity,
        } => {
            let description = description.unwrap_or_else(|| {
                println!("Enter description (or leave blank for 'TBD'):");
                let mut buf = String::new();
                std::io::stdin().read_line(&mut buf).ok();
                let s = buf.trim();
                if s.is_empty() {
                    "TBD".to_string()
                } else {
                    s.to_string()
                }
            });
            let limit = crate::registry::known_limits::KnownLimit {
                id: id.clone(),
                category,
                description,
                source,
                severity,
                first_seen_at: chrono::Utc::now(),
                last_checked_at: None,
                mitigated: false,
            };
            crate::registry::known_limits::save_known_limit(&conn, &limit)?;
            println!("Saved known limit '{}'.", id);
        }
        crate::LimitCommands::List { category, mitigated, json } => {
            let limits = crate::registry::known_limits::list_known_limits(
                &conn,
                category.as_deref(),
                mitigated,
            )?;
            if json {
                println!("{}", serde_json::to_string_pretty(&limits)?);
            } else {
                if limits.is_empty() {
                    println!("No known limits found.");
                } else {
                    println!("Known limits ({}):", limits.len());
                    for l in &limits {
                        let status = if l.mitigated { "✓" } else { "✗" };
                        let sev = l.severity.map(|s| format!(" [sev:{}]", s)).unwrap_or_default();
                        println!(
                            "  {} [{}] {}{}{}",
                            status,
                            l.id,
                            l.category,
                            sev,
                            if l.mitigated { " (mitigated)" } else { "" }
                        );
                        println!("    {}", l.description);
                        if let Some(ref src) = l.source {
                            println!("    Source: {}", src);
                        }
                    }
                }
            }
        }
        crate::LimitCommands::Resolve { id, reason } => {
            if crate::registry::known_limits::resolve_known_limit(&conn, &id)? {
                if let Some(ref r) = reason {
                    let meta = crate::registry::knowledge_meta::KnowledgeMeta {
                        id: format!("resolve-{}", id),
                        target_level: 3,
                        target_id: id.clone(),
                        correction_type: Some("human-feedback".to_string()),
                        correction_json: Some(serde_json::json!({"reason": r}).to_string()),
                        confidence: 1.0,
                        created_at: chrono::Utc::now(),
                    };
                    let _ = crate::registry::knowledge_meta::save_knowledge_meta(&conn, &meta);
                }
                println!("Resolved known limit '{}'.", id);
            } else {
                println!("Known limit '{}' not found.", id);
            }
        }
        crate::LimitCommands::Delete { id } => {
            if crate::registry::known_limits::delete_known_limit(&conn, &id)? {
                println!("Deleted known limit '{}'.", id);
            } else {
                println!("Known limit '{}' not found.", id);
            }
        }
        crate::LimitCommands::Seed => {
            let count = crate::registry::known_limits::seed_hard_vetoes(&conn)?;
            println!("Seeded {} hard vetoes.", count);
        }
    }
    Ok(())
}
