use std::path::PathBuf;
use crate::registry::RepoEntry;
use rayon::prelude::*;

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
    run_index_with_progress(conn, path, None)
}

/// 带进度上报的索引逻辑。
/// `progress_tx` 接收阶段性进度消息，用于 MCP streaming 等实时反馈场景。
pub fn run_index_with_progress(
    conn: &mut rusqlite::Connection,
    path: &str,
    progress_tx: Option<crossbeam_channel::Sender<String>>,
) -> anyhow::Result<usize> {
    use tracing::{info, warn};

    let notify = |msg: String| {
        if let Some(ref tx) = progress_tx {
            let _ = tx.send(msg);
        }
    };

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

    // Load orphan list for lazy repair; delete_repo_doc below will clean them.
    let orphaned_repos: Vec<String> = conn
        .prepare("SELECT repo_id FROM orphan_tantivy_docs")?
        .query_map([], |row| row.get::<_, String>(0))?
        .filter_map(Result::ok)
        .collect();

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

        // Determine incremental vs full index
        let changed_opt = detect_changes(conn, repo);
        if let Some(ref changed) = changed_opt
            && changed.added.is_empty() && changed.modified.is_empty() && changed.deleted.is_empty() {
                println!("[{}] Already up-to-date", repo.id);
                count += 1;
                continue;
            }
        let is_incremental = changed_opt.is_some();
        notify(format!("detect_changes:{},incremental={}", repo.id, is_incremental));

        // Semantic code indexing (tree-sitter AST extraction + call graph)
        let (symbols, calls) = if let Some(ref changed) = changed_opt {
            // Incremental: delete old symbols for modified/deleted files
            let files_to_delete: Vec<String> = changed.modified.iter().chain(changed.deleted.iter()).cloned().collect();
            if !files_to_delete.is_empty() {
                let _ = crate::semantic_index::persist::delete_symbols_for_files(conn, &repo.id, &files_to_delete);
            }
            crate::semantic_index::index_repo_incremental(&repo.local_path, changed)
        } else {
            // Full index
            crate::semantic_index::index_repo_full(&repo.local_path)
        };

        if !symbols.is_empty() {
            let result = if is_incremental {
                crate::semantic_index::persist::save_symbols_incremental(conn, &repo.id, &symbols)
            } else {
                crate::semantic_index::save_symbols(conn, &repo.id, &symbols)
            };
            match result {
                Ok(n) => {
                    info!("Saved {} code symbols for {}", n, repo.id);
                    notify(format!("semantic_index:{},symbols={}", repo.id, n));
                }
                Err(e) => warn!("Failed to save code symbols for {}: {}", repo.id, e),
            }
        }
        if !calls.is_empty() {
            let result = if is_incremental {
                crate::semantic_index::persist::save_calls_incremental(conn, &repo.id, &calls)
            } else {
                crate::semantic_index::save_calls(conn, &repo.id, &calls)
            };
            match result {
                Ok(n) => {
                    info!("Saved {} call edges for {}", n, repo.id);
                    notify(format!("call_graph:{},calls={}", repo.id, n));
                }
                Err(e) => warn!("Failed to save call graph for {}: {}", repo.id, e),
            }
        }

        // Generate embeddings for code symbols (local candle, Sprint 14)
        if !symbols.is_empty() {
            let result = if is_incremental {
                save_symbol_embeddings_incremental(conn, &repo.id, &symbols)
            } else {
                save_symbol_embeddings(conn, &repo.id, &symbols)
            };
            match result {
                Ok(n) => {
                    info!("Saved {} symbol embeddings for {}", n, repo.id);
                    notify(format!("embeddings:{},count={}", repo.id, n));
                }
                Err(e) => warn!("Failed to save symbol embeddings for {}: {}", repo.id, e),
            }
        }

        // Save repo_index_state for next incremental run
        if let Ok(Some(hash)) = crate::semantic_index::git_diff::current_head_hash(&repo.local_path) {
            let _ = save_repo_index_state(conn, &repo.id, &hash);
        }

        // Cross-repo dependency graph
        match crate::dependency_graph::build_dependency_graph(conn, &repo.id, &repo.local_path) {
            Ok(n) => {
                if n > 0 {
                    info!("Resolved {} local dependencies for {}", n, repo.id);
                }
                notify(format!("dependency_graph:{},count={}", repo.id, n));
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
    notify("tantivy_commit".to_string());

    // Clean up orphan records for repos that were successfully indexed this run.
    if count > 0 && !orphaned_repos.is_empty() {
        let indexed_ids: std::collections::HashSet<&str> =
            repos.iter().map(|r| r.id.as_str()).collect();
        for orphan_id in &orphaned_repos {
            if indexed_ids.contains(orphan_id.as_str()) {
                let _ = conn.execute(
                    "DELETE FROM orphan_tantivy_docs WHERE repo_id = ?1",
                    [orphan_id],
                );
            }
        }
    }

    println!("\nIndexed {} repositories.", count);
    Ok(count)
}

/// Parallel embedding generation for code symbols.
/// Phase 1: CPU-intensive encoding across all available cores (rayon).
/// Phase 2: Single-threaded SQLite batch write to avoid lock contention.
fn generate_and_save_embeddings(
    conn: &mut rusqlite::Connection,
    repo_id: &str,
    symbols: &[crate::semantic_index::CodeSymbol],
    clear_existing: bool,
) -> anyhow::Result<usize> {
    use tracing::{info, warn};

    // Phase 1: parallel encoding
    let items: Vec<(String, String, Vec<f32>)> = symbols
        .par_iter()
        .filter_map(|sym| {
            let text = format!("{} {}", sym.name, sym.signature.as_deref().unwrap_or(""));
            match crate::embedding::generate_query_embedding(&text) {
                Ok(emb) => {
                    let fp = sym.file_path.to_string_lossy().to_string();
                    Some((fp, sym.name.clone(), emb))
                }
                Err(e) => {
                    warn!("Embedding generation failed for '{}': {}", sym.name, e);
                    None
                }
            }
        })
        .collect();

    // Phase 2: single-threaded batch write
    let tx = conn.transaction()?;
    if clear_existing {
        tx.execute("DELETE FROM code_embeddings WHERE repo_id = ?1", [repo_id])?;
    }
    let now = chrono::Utc::now().to_rfc3339();
    let mut inserted = 0usize;
    for (file_path, name, embedding) in items {
        let blob = crate::embedding::embedding_to_bytes(&embedding);
        let sql = "INSERT INTO code_embeddings (repo_id, file_path, symbol_name, embedding, generated_at) VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(repo_id, file_path, symbol_name) DO UPDATE SET
             embedding = excluded.embedding,
             generated_at = excluded.generated_at";
        match tx.execute(sql, rusqlite::params![repo_id, &file_path, &name, &blob, &now]) {
            Ok(_) => inserted += 1,
            Err(e) => warn!("Failed to insert embedding for {}: {}", name, e),
        }
    }
    tx.commit()?;

    let mode = if clear_existing { "" } else { " (incremental)" };
    info!("Saved {} symbol embeddings{} for {}", inserted, mode, repo_id);
    Ok(inserted)
}

fn save_symbol_embeddings(
    conn: &mut rusqlite::Connection,
    repo_id: &str,
    symbols: &[crate::semantic_index::CodeSymbol],
) -> anyhow::Result<usize> {
    generate_and_save_embeddings(conn, repo_id, symbols, true)
}

fn save_symbol_embeddings_incremental(
    conn: &mut rusqlite::Connection,
    repo_id: &str,
    symbols: &[crate::semantic_index::CodeSymbol],
) -> anyhow::Result<usize> {
    generate_and_save_embeddings(conn, repo_id, symbols, false)
}

/// Detect whether a repo can be incrementally indexed.
/// Returns `Some(ChangedFiles)` if incremental is possible and worthwhile.
/// Returns `None` for first-time index, non-Git repos, too many changes, or errors.
fn detect_changes(
    conn: &rusqlite::Connection,
    repo: &crate::registry::RepoEntry,
) -> Option<crate::semantic_index::git_diff::ChangedFiles> {
    use tracing::{info, warn};

    match super::index_state::get_repo_index_state(conn, repo) {
        super::index_state::IndexState::Fresh => {
            Some(crate::semantic_index::git_diff::ChangedFiles {
                added: vec![],
                modified: vec![],
                deleted: vec![],
            })
        }
        super::index_state::IndexState::Stale { added, modified, deleted } => {
            let total = added.len() + modified.len() + deleted.len();
            if total > 100 {
                info!("Repo {} has {} changed files (>100 threshold), falling back to full index", repo.id, total);
                return None;
            }
            info!("Repo {}: incremental index ({} added, {} modified, {} deleted)",
                repo.id, added.len(), modified.len(), deleted.len()
            );
            Some(crate::semantic_index::git_diff::ChangedFiles { added, modified, deleted })
        }
        super::index_state::IndexState::Missing => {
            info!("Repo {}: no prior index state, falling back to full index", repo.id);
            None
        }
        super::index_state::IndexState::Unknown { ref reason } => {
            warn!("Repo {}: index state unknown ({}), falling back to full index", repo.id, reason);
            None
        }
    }
}

fn save_repo_index_state(
    conn: &mut rusqlite::Connection,
    repo_id: &str,
    hash: &str,
) -> anyhow::Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO repo_index_state (repo_id, last_commit_hash, indexed_at)
         VALUES (?1, ?2, datetime('now'))",
        [repo_id, hash],
    )?;
    Ok(())
}

