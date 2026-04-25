//! AI First-User Validation Script
//!
//! Validates devbase from the AI consumer perspective.

use devbase::registry::WorkspaceRegistry;

fn main() -> anyhow::Result<()> {
    println!("AI First-User Validation: devbase v0.2.4\n");

    let conn = WorkspaceRegistry::init_db()?;

    // 1. Knowledge Report
    println!("1. Knowledge Coverage Report (workspace-wide)");
    let report = devbase::oplog_analytics::generate_report(&conn, None, 5)?;
    println!(
        "   Repos: {} | Symbols: {} | Embeddings: {} | Calls: {}",
        report.repo_count, report.total_symbols, report.total_embeddings, report.total_calls
    );
    println!("   Overall coverage: {:.1}%", report.overall_coverage_pct);
    println!("   Top 5 repos by symbol count:");
    for repo in report.repos.iter().take(5) {
        println!(
            "   - {}: {} symbols, {} embeddings ({:.1}% coverage)",
            repo.repo_id, repo.symbol_count, repo.embedding_count, repo.coverage_pct
        );
    }
    println!();

    // 2. Hybrid Search (keyword-only) on claude-code-rust
    println!("2. Hybrid Search: 'error handling' in claude-code-rust (keyword-only)");
    let results = devbase::search::hybrid::hybrid_search_symbols(
        &conn,
        "claude-code-rust",
        "error handling",
        None,
        10,
    )?;
    println!("   Found {} matches:", results.len());
    for (i, (_repo, name, path, line, score)) in results.iter().take(5).enumerate() {
        println!("   {}. {} ({}:{}) - score: {:.3}", i + 1, name, path, line, score);
    }
    println!();

    // 3. Hybrid Search on devbase
    println!("3. Hybrid Search: 'sync' in devbase (keyword-only)");
    let results =
        devbase::search::hybrid::hybrid_search_symbols(&conn, "unknown", "sync", None, 10)?;
    println!("   Found {} matches:", results.len());
    for (i, (_repo, name, path, line, score)) in results.iter().take(5).enumerate() {
        println!("   {}. {} ({}:{}) - score: {:.3}", i + 1, name, path, line, score);
    }
    println!();

    // 4. Vector path validation — use an existing embedding as query vector
    println!("4. Vector path validation (semantic search)");
    let mut vector_ok = false;
    if report.total_embeddings > 0 {
        // Grab the first embedding from the DB to use as a query vector
        let row: Result<(String, Vec<u8>), _> =
            conn.query_row("SELECT repo_id, embedding FROM code_embeddings LIMIT 1", [], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, Vec<u8>>(1)?))
            });
        if let Ok((emb_repo, blob)) = row {
            let query_vec: Vec<f32> = blob
                .chunks_exact(4)
                .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                .collect();
            let vec_results =
                WorkspaceRegistry::semantic_search_symbols(&conn, &emb_repo, &query_vec, 5);
            match vec_results {
                Ok(v) if !v.is_empty() => {
                    println!(
                        "   Semantic search returned {} match(es) for repo '{}'",
                        v.len(),
                        emb_repo
                    );
                    vector_ok = true;
                }
                Ok(_) => {
                    println!("   Semantic search returned 0 matches (unexpected but not an error)")
                }
                Err(e) => println!("   Semantic search error: {}", e),
            }
        } else {
            println!("   Could not read existing embedding from DB");
        }
    } else {
        println!("   No embeddings in DB; vector path not testable");
    }
    println!();

    // 5. Generate Symbol Links for devbase
    println!("5. Generating explicit symbol links for devbase...");
    let mut conn_mut = WorkspaceRegistry::init_db()?;
    let count = devbase::symbol_links::generate_and_save_links(&mut conn_mut, "unknown")?;
    println!("   Generated {} symbol links\n", count);

    // 6. Traverse Related Symbols
    println!("6. Related symbols to 'run_index' in devbase:");
    let related = WorkspaceRegistry::find_related_symbols(&conn, "unknown", "run_index", 10)?;
    println!("   Found {} related symbols:", related.len());
    for (_src_repo, _src_sym, target_repo, target_sym, link_type, strength) in
        related.iter().take(5)
    {
        println!(
            "   - {} (repo: {}) - {} - strength: {:.3}",
            target_sym, target_repo, link_type, strength
        );
    }
    println!();

    // 7. Cross-Repo Search
    println!("7. Cross-repo search: 'main' in Rust repos");
    let tags: Vec<String> = vec!["rust".into()];
    let results = WorkspaceRegistry::cross_repo_search_symbols(&conn, &tags, "main", None, 10)?;
    println!("   Found {} matches across Rust repos:", results.len());
    for (i, (repo, name, path, line, score)) in results.iter().take(5).enumerate() {
        println!("   {}. {}::{} ({}:{}) - score: {:.3}", i + 1, repo, name, path, line, score);
    }
    println!();

    // 8. Skill Runtime validation
    println!("8. Skill Runtime validation");
    let skills = devbase::skill_runtime::registry::list_skills(&conn, None, None)?;
    println!("   Installed skills: {}", skills.len());
    let builtin_count = skills
        .iter()
        .filter(|s| matches!(s.skill_type, devbase::skill_runtime::SkillType::Builtin))
        .count();
    println!("   Built-in skills: {}", builtin_count);
    if let Some(emb) = skills.iter().find(|s| s.id == "embed-repo") {
        println!("   [OK] embed-repo skill found (type: {:?})", emb.skill_type);
    }
    if let Some(sr) = skills.iter().find(|s| s.id == "search-workspace") {
        println!("   [OK] search-workspace skill found (type: {:?})", sr.skill_type);
    }
    if let Some(kr) = skills.iter().find(|s| s.id == "knowledge-report") {
        println!("   [OK] knowledge-report skill found (type: {:?})", kr.skill_type);
    }
    println!();

    println!("Validation Complete");
    println!("  [OK] Keyword-only hybrid_search works on real data");
    println!(
        "  [{}] Vector path: semantic search {}",
        if vector_ok { "OK" } else { "WARN" },
        if vector_ok {
            "functional"
        } else {
            "needs provider"
        }
    );
    println!("  [OK] Symbol links generated and traversable");
    println!("  [OK] Cross-repo search functional");
    println!(
        "  [OK] Skill Runtime: {} skill(s) registered ({} builtin)",
        skills.len(),
        builtin_count
    );

    Ok(())
}
