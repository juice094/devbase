use std::path::PathBuf;
use crate::registry::RepoEntry;

fn index_repo_in_search(
    repo: &crate::registry::RepoEntry,
    summary: &str,
    keywords: &str,
) -> anyhow::Result<()> {
    let (index, _reader) = crate::search::init_index()?;
    let mut writer = crate::search::get_writer(&index)?;
    let schema = index.schema();
    crate::search::delete_repo_doc(&mut writer, &schema, &repo.id)?;
    crate::search::add_repo_doc(&mut writer, &schema, &repo.id, summary, keywords, &repo.tags)?;
    crate::search::commit_writer(&mut writer)?;
    Ok(())
}

pub fn index_repo(
    conn: &mut rusqlite::Connection,
    repo: &crate::registry::RepoEntry,
) -> anyhow::Result<()> {
    use tracing::{info, warn};

    let config = crate::config::Config::load().ok();
    let (summary, keywords) = config
        .as_ref()
        .and_then(|cfg| super::try_llm_summary(&repo.local_path, &cfg.llm))
        .or_else(|| super::extract_readme_summary(&repo.local_path).map(|(s, k)| (s, k.join(", "))))
        .unwrap_or_else(|| {
            warn!("No README found for {}, generating fallback summary", repo.id);
            super::generate_fallback_summary(&repo.local_path)
        });

    let modules = super::extract_module_structure(&repo.local_path);

    crate::registry::knowledge::save_summary(conn, &repo.id, &summary, &keywords)?;

    if let Err(e) = index_repo_in_search(repo, &summary, &keywords) {
        warn!("Failed to index repo in search: {}", e);
    }

    let modules_tuple: Vec<(String, String)> =
        modules.into_iter().map(|m| (m.name, m.kind)).collect();
    crate::registry::knowledge::save_modules(conn, &repo.id, &modules_tuple)?;

    let detected_lang = crate::scan::detect_language(&repo.local_path);
    if let Some(ref lang) = detected_lang {
        crate::registry::repo::update_repo_language(conn, &repo.id, Some(lang))?;
    }

    info!(
        "Indexed [{}] -> \"{}\" (keywords: {}) language={:?}",
        repo.id, summary, keywords, detected_lang
    );
    Ok(())
}

/// 兼容旧调用的包装层：执行索引逻辑
pub fn run_index(conn: &mut rusqlite::Connection, path: &str) -> anyhow::Result<usize> {
    use tracing::{info, warn};

    let repos: Vec<RepoEntry> = if path.is_empty() {
        crate::registry::repo::list_repos(conn)?
    } else {
        let p = PathBuf::from(path);
        if !p.exists() {
            anyhow::bail!("Path does not exist: {}", path);
        }
        let registered = crate::registry::repo::list_repos(conn)?;
        if let Some(repo) = registered.into_iter().find(|r| r.local_path == p) {
            vec![repo]
        } else {
            info!("Registering {} before indexing", path);
            let repo = crate::scan::inspect_repo(&p, None)?;
            crate::registry::repo::save_repo(conn, &repo)?;
            vec![repo]
        }
    };

    // Initialize Tantivy search index writer once for the batch
    let (search_index, _reader) = crate::search::init_index()?;
    let mut search_writer = crate::search::get_writer(&search_index)?;
    let search_schema = search_index.schema();

    let mut count = 0;
    for repo in &repos {
        let config = crate::config::Config::load().ok();
        let (summary, keywords) = config
            .as_ref()
            .and_then(|cfg| super::try_llm_summary(&repo.local_path, &cfg.llm))
            .or_else(|| super::extract_readme_summary(&repo.local_path).map(|(s, k)| (s, k.join(", "))))
            .unwrap_or_else(|| {
                warn!("No README found for {}, generating fallback summary", repo.id);
                super::generate_fallback_summary(&repo.local_path)
            });

        let modules = super::extract_module_structure(&repo.local_path);

        crate::registry::knowledge::save_summary(conn, &repo.id, &summary, &keywords)?;

        // Add/update repo document in Tantivy index
        crate::search::delete_repo_doc(&mut search_writer, &search_schema, &repo.id)?;
        crate::search::add_repo_doc(
            &mut search_writer,
            &search_schema,
            &repo.id,
            &summary,
            &keywords,
            &repo.tags,
        )?;

        let modules_tuple: Vec<(String, String)> =
            modules.into_iter().map(|m| (m.name, m.kind)).collect();
        crate::registry::knowledge::save_modules(conn, &repo.id, &modules_tuple)?;

        let detected_lang = crate::scan::detect_language(&repo.local_path);
        if let Some(ref lang) = detected_lang {
            crate::registry::repo::update_repo_language(conn, &repo.id, Some(lang))?;
        }

        // Semantic code indexing (tree-sitter AST extraction + call graph)
        let (symbols, calls) = crate::semantic_index::index_repo_full(&repo.local_path);
        if !symbols.is_empty() {
            match crate::semantic_index::save_symbols(conn, &repo.id, &symbols) {
                Ok(n) => info!("Saved {} code symbols for {}", n, repo.id),
                Err(e) => warn!("Failed to save code symbols for {}: {}", repo.id, e),
            }
        }
        if !calls.is_empty() {
            match crate::semantic_index::save_calls(conn, &repo.id, &calls) {
                Ok(n) => info!("Saved {} call edges for {}", n, repo.id),
                Err(e) => warn!("Failed to save call graph for {}: {}", repo.id, e),
            }
        }

        // Generate embeddings for code symbols (local candle, Sprint 14)
        if !symbols.is_empty() {
            match save_symbol_embeddings(conn, &repo.id, &symbols) {
                Ok(n) => info!("Saved {} symbol embeddings for {}", n, repo.id),
                Err(e) => warn!("Failed to save symbol embeddings for {}: {}", repo.id, e),
            }
        }

        // Cross-repo dependency graph
        match crate::dependency_graph::build_dependency_graph(conn, &repo.id, &repo.local_path) {
            Ok(n) => {
                if n > 0 {
                    info!("Resolved {} local dependencies for {}", n, repo.id);
                }
            }
            Err(e) => warn!("Failed to build dependency graph for {}: {}", repo.id, e),
        }

        println!(
            "Indexed [{}] -> \"{}\" (keywords: {}) language={:?} symbols={} calls={}",
            repo.id,
            summary,
            keywords,
            detected_lang,
            symbols.len(),
            calls.len(),
        );
        count += 1;
    }

    crate::search::commit_writer(&mut search_writer)?;

    println!("\nIndexed {} repositories.", count);
    Ok(count)
}

/// Generate and save embeddings for code symbols using the default provider.
/// Clears old embeddings for the repo before inserting new ones.
fn save_symbol_embeddings(
    conn: &mut rusqlite::Connection,
    repo_id: &str,
    symbols: &[crate::semantic_index::CodeSymbol],
) -> anyhow::Result<usize> {
    use tracing::{info, warn};

    let tx = conn.transaction()?;
    tx.execute("DELETE FROM code_embeddings WHERE repo_id = ?1", [repo_id])?;

    let mut inserted = 0usize;
    for symbol in symbols {
        let text = format!("{} {}", symbol.name, symbol.signature.as_deref().unwrap_or(""));
        match crate::embedding::generate_query_embedding(&text) {
            Ok(embedding) => {
                let blob = crate::embedding::embedding_to_bytes(&embedding);
                let now = chrono::Utc::now().to_rfc3339();
                match tx.execute(
                    "INSERT INTO code_embeddings (repo_id, symbol_name, embedding, generated_at) VALUES (?1, ?2, ?3, ?4)",
                    rusqlite::params![repo_id, &symbol.name, &blob, &now],
                ) {
                    Ok(_) => inserted += 1,
                    Err(e) => warn!("Failed to insert embedding for {}: {}", symbol.name, e),
                }
            }
            Err(e) => {
                warn!("Embedding generation failed for '{}': {}", symbol.name, e);
            }
        }
    }

    tx.commit()?;
    info!("Saved {} symbol embeddings for {}", inserted, repo_id);
    Ok(inserted)
}

