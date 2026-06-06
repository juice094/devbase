/// Import ontology from an OpenClaw-compatible workspace into devbase registry.
pub fn run_import(
    ctx: &mut crate::storage::AppContext,
    workspace: &str,
    dry_run: bool,
) -> anyhow::Result<()> {
    let wp = if workspace.is_empty() {
        dirs::home_dir()
            .unwrap_or_default()
            .join(".kimi_openclaw")
            .join("workspace")
    } else {
        std::path::PathBuf::from(workspace)
    };

    if dry_run {
        println!("Dry-run: would import ontology from {}", wp.display());
        if wp.exists() {
            let entities_dir = wp.join("ontology").join("entities");
            let relations_file = wp.join("ontology").join("relations").join("core-relations.jsonl");
            if entities_dir.is_dir() {
                let count = std::fs::read_dir(&entities_dir)?.count();
                println!("  Entities found: {}", count);
            }
            if relations_file.exists() {
                let lines = std::fs::read_to_string(&relations_file)?
                    .lines()
                    .filter(|l| !l.trim().is_empty())
                    .count();
                println!("  Relations found: {}", lines);
            }
        }
        return Ok(());
    }

    let conn = ctx.conn()?;
    let stats = crate::registry::import_ontology::import_ontology(&conn, &wp)?;

    println!("Ontology import from: {}", wp.display());
    println!("  Entities: {} added, {} updated", stats.entities_added, stats.entities_updated);
    println!("  Relations: {} added, {} updated", stats.relations_added, stats.relations_updated);
    if !stats.errors.is_empty() {
        println!("  Errors: {}", stats.errors.len());
        for e in &stats.errors {
            println!("    - {}", e);
        }
    }
    println!(
        "  Total: {} entities, {} relations",
        stats.entities_added + stats.entities_updated,
        stats.relations_added + stats.relations_updated,
    );

    Ok(())
}
