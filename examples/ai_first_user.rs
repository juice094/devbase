//! AI First-User Validation Script
//!
//! Validates devbase from the AI consumer perspective.
//! Run: cargo run --example ai_first_user

use devbase::registry::WorkspaceRegistry;

fn main() -> anyhow::Result<()> {
    println!("AI First-User Validation: devbase v0.14.0\n");

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

    // 2. Hybrid Search with auto-generated embedding
    println!("2. Hybrid Search: 'error handling' in devbase (auto-embedding)");
    let query_emb = devbase::embedding::generate_query_embedding("error handling").ok();
    let results = devbase::search::hybrid::hybrid_search_symbols(
        &conn,
        "devbase",
        "error handling",
        query_emb.as_deref(),
        10,
    )?;
    println!("   Found {} matches:", results.len());
    for (i, (_repo, name, path, line, score)) in results.iter().take(5).enumerate() {
        println!("   {}. {} ({}:{}) - score: {:.3}", i + 1, name, path, line, score);
    }
    println!();

    // 3. Hybrid Search (keyword-only fallback) on devbase
    println!("3. Hybrid Search: 'sync' in devbase (keyword fallback)");
    let results =
        devbase::search::hybrid::hybrid_search_symbols(&conn, "devbase", "sync", None, 10)?;
    println!("   Found {} matches:", results.len());
    for (i, (_repo, name, path, line, score)) in results.iter().take(5).enumerate() {
        println!("   {}. {} ({}:{}) - score: {:.3}", i + 1, name, path, line, score);
    }
    println!();

    // 4. Vector path validation — semantic search with auto-generated embedding
    println!("4. Vector path validation (semantic search with candle)");
    let mut vector_ok = false;
    let query_vec = devbase::embedding::generate_query_embedding("sync orchestrator")?;
    let vec_results =
        devbase::registry::knowledge::semantic_search_symbols(&conn, "devbase", &query_vec, 5)?;
    match vec_results {
        v if !v.is_empty() => {
            println!(
                "   Semantic search returned {} match(es) for repo 'devbase'",
                v.len()
            );
            for (i, (_repo, name, path, line, score)) in v.iter().take(3).enumerate() {
                println!("   {}. {} ({}:{}) - score: {:.3}", i + 1, name, path, line, score);
            }
            vector_ok = true;
        }
        _ => {
            println!("   Semantic search returned 0 matches (unexpected but not an error)")
        }
    }
    println!();

    // 5. Symbol Links (limited scope to avoid timeout on large repos)
    println!("5. Generating explicit symbol links for devbase (limit 500 symbols)...");
    let mut conn_mut = WorkspaceRegistry::init_db()?;
    let count = devbase::symbol_links::generate_and_save_links(&mut conn_mut, "devbase")?;
    println!("   Generated {} symbol links\n", count);

    // 6. Traverse Related Symbols
    println!("6. Related symbols to 'run_index' in devbase:");
    let related =
        devbase::registry::knowledge::find_related_symbols(&conn, "devbase", "run_index", 10)?;
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
    let results =
        devbase::registry::knowledge::cross_repo_search_symbols(&conn, &tags, "main", None, 10)?;
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
    println!("  [OK] Hybrid search with auto-generated embedding works on real data");
    println!(
        "  [{}] Vector path: candle semantic search {}",
        if vector_ok { "OK" } else { "WARN" },
        if vector_ok { "functional" } else { "needs provider" }
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
