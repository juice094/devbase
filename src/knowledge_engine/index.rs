// SPDX-License-Identifier: MIT
// Copyright (c) 2026 juice094
use crate::registry::RepoEntry;
use std::path::PathBuf;
use tantivy::{IndexWriter, schema::Schema};

fn index_repo_core(
    conn: &mut rusqlite::Connection,
    repo: &crate::registry::RepoEntry,
    config: Option<&crate::config::Config>,
    writer: &mut IndexWriter,
    schema: &Schema,
) -> anyhow::Result<()> {
    use tracing::{info, warn};

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

    if let Err(e) = crate::search::delete_repo_doc(writer, schema, &repo.id).and_then(|_| {
        crate::search::add_repo_doc(writer, schema, &repo.id, &summary, &keywords, &repo.tags)
    }) {
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

/// Index a single repo with a standalone Tantivy writer.
/// Suitable for one-off indexing where writer reuse is not needed.
pub fn index_repo(
    conn: &mut rusqlite::Connection,
    repo: &crate::registry::RepoEntry,
    config: Option<&crate::config::Config>,
) -> anyhow::Result<()> {
    let (index, _reader) = crate::search::init_index()?;
    let mut writer = crate::search::get_writer(&index)?;
    let schema = index.schema();
    index_repo_core(conn, repo, config, &mut writer, &schema)?;
    crate::search::commit_writer(&mut writer)?;
    Ok(())
}

/// Index a single repo reusing an existing Tantivy writer.
/// Callers must commit the writer after the batch.
pub fn index_repo_with_writer(
    conn: &mut rusqlite::Connection,
    repo: &crate::registry::RepoEntry,
    config: Option<&crate::config::Config>,
    writer: &mut IndexWriter,
    schema: &Schema,
) -> anyhow::Result<()> {
    index_repo_core(conn, repo, config, writer, schema)
}

/// 兼容旧调用的包装层：执行索引逻辑
pub fn run_index(
    conn: &mut rusqlite::Connection,
    path: &str,
    skip_embeddings: bool,
) -> anyhow::Result<usize> {
    run_index_with_progress(conn, path, None, skip_embeddings)
}

/// Resolve the list of repositories to index for a given path.
/// If `path` is empty, returns all registered repos.
/// If `path` points to an unregistered repo, auto-registers it before returning.
fn prepare_repos(conn: &mut rusqlite::Connection, path: &str) -> anyhow::Result<Vec<RepoEntry>> {
    use tracing::info;

    if path.is_empty() {
        return crate::registry::repo::list_repos(conn);
    }

    let p = PathBuf::from(path);
    if !p.exists() {
        anyhow::bail!("Path does not exist: {}", path);
    }
    let registered = crate::registry::repo::list_repos(conn)?;
    if let Some(repo) = registered.into_iter().find(|r| r.local_path == p) {
        Ok(vec![repo])
    } else {
        info!("Registering {} before indexing", path);
        let repo = crate::scan::inspect_repo(&p, None)?;
        crate::registry::repo::save_repo(conn, &repo)?;
        Ok(vec![repo])
    }
}

/// 带进度上报的索引逻辑。
/// `progress_tx` 接收阶段性进度消息，用于 MCP streaming 等实时反馈场景。
pub fn run_index_with_progress(
    conn: &mut rusqlite::Connection,
    path: &str,
    progress_tx: Option<crossbeam_channel::Sender<String>>,
    skip_embeddings: bool,
) -> anyhow::Result<usize> {
    use tracing::{info, warn};

    let notify = |msg: String| {
        if let Some(ref tx) = progress_tx {
            let _ = tx.send(msg);
        }
    };

    let repos = prepare_repos(conn, path)?;

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

    let config = crate::config::Config::load().ok();
    let mut count = 0;
    for repo in &repos {
        let t0 = std::time::Instant::now();
        let (summary, keywords) = config
            .as_ref()
            .and_then(|cfg| super::try_llm_summary(&repo.local_path, &cfg.llm))
            .or_else(|| {
                super::extract_readme_summary(&repo.local_path).map(|(s, k)| (s, k.join(", ")))
            })
            .unwrap_or_else(|| {
                warn!("No README found for {}, generating fallback summary", repo.id);
                super::generate_fallback_summary(&repo.local_path)
            });
        let t1 = std::time::Instant::now();

        let modules = super::extract_module_structure(&repo.local_path);
        let t2 = std::time::Instant::now();

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
        let t3 = std::time::Instant::now();

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
            && changed.added.is_empty()
            && changed.modified.is_empty()
            && changed.deleted.is_empty()
        {
            println!("[{}] Already up-to-date", repo.id);
            count += 1;
            continue;
        }
        let is_incremental = changed_opt.is_some();
        notify(format!("detect_changes:{},incremental={}", repo.id, is_incremental));

        // Semantic code indexing (tree-sitter AST extraction + call graph)
        let (symbols, calls) = if let Some(ref changed) = changed_opt {
            // Incremental: delete old symbols for modified/deleted files
            let files_to_delete: Vec<String> =
                changed.modified.iter().chain(changed.deleted.iter()).cloned().collect();
            if !files_to_delete.is_empty() {
                let _ = crate::semantic_index::persist::delete_symbols_for_files(
                    conn,
                    &repo.id,
                    &files_to_delete,
                );
            }
            crate::semantic_index::index_repo_incremental(&repo.local_path, changed)
        } else {
            // Full index
            crate::semantic_index::index_repo_full(&repo.local_path)?
        };
        let t4 = std::time::Instant::now();

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

            // Index symbols in Tantivy for BM25 keyword search
            if let Err(e) = index_symbols_in_search(&repo.id, &symbols, is_incremental) {
                warn!("Failed to index symbols in search for {}: {}", repo.id, e);
            } else {
                notify(format!("symbol_index:{},symbols={}", repo.id, symbols.len()));
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
        let t5 = std::time::Instant::now();

        // Generate embeddings for code symbols (local candle, Sprint 14)
        if !skip_embeddings && !symbols.is_empty() {
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
        let t6 = std::time::Instant::now();

        // Save repo_index_state for next incremental run
        if let Ok(Some(hash)) = crate::semantic_index::git_diff::current_head_hash(&repo.local_path)
        {
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
        let t7 = std::time::Instant::now();

        println!(
            "Indexed [{}] -> \"{}\" (keywords: {}) language={:?} symbols={} calls={}",
            repo.id,
            summary,
            keywords,
            detected_lang,
            symbols.len(),
            calls.len(),
        );
        println!(
            "  timings: readme={:.0}ms module={:.0}ms tantivy={:.0}ms semantic={:.0}ms save={:.0}ms embed={:.0}ms deps={:.0}ms total={:.0}ms",
            (t1 - t0).as_millis(),
            (t2 - t1).as_millis(),
            (t3 - t2).as_millis(),
            (t4 - t3).as_millis(),
            (t5 - t4).as_millis(),
            (t6 - t5).as_millis(),
            (t7 - t6).as_millis(),
            (t7 - t0).as_millis(),
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
                let _ =
                    conn.execute("DELETE FROM orphan_tantivy_docs WHERE repo_id = ?1", [orphan_id]);
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
    use rayon::prelude::*;
    use tracing::{info, warn};

    // Phase 1: parallel encoding (rayon par_iter gives best throughput for
    // Candle CPU BERT because per-symbol sequences are short and variable;
    // batching causes excessive padding and Candle's CPU matmul is slower
    // for large padded batches than many small single inferences).
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
                info!(
                    "Repo {} has {} changed files (>100 threshold), falling back to full index",
                    repo.id, total
                );
                return None;
            }
            info!(
                "Repo {}: incremental index ({} added, {} modified, {} deleted)",
                repo.id,
                added.len(),
                modified.len(),
                deleted.len()
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

fn index_symbols_in_search(
    repo_id: &str,
    symbols: &[crate::semantic_index::CodeSymbol],
    _is_incremental: bool,
) -> anyhow::Result<()> {
    let (index, _reader) = crate::search::symbol_index::init_index()?;
    let mut writer = crate::search::symbol_index::get_writer(&index)?;
    let schema = index.schema();
    crate::search::symbol_index::add_symbols(&mut writer, &schema, repo_id, symbols)?;
    crate::search::symbol_index::commit_writer(&mut writer)?;
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::RepoEntry;
    use crate::registry::test_helpers::WorkspaceRegistry;
    use std::path::Path;

    fn init_git_repo(path: &Path) -> git2::Repository {
        let repo = git2::Repository::init(path).unwrap();
        let mut config = repo.config().unwrap();
        config.set_str("user.name", "Test").unwrap();
        config.set_str("user.email", "test@example.com").unwrap();
        let sig = repo.signature().unwrap();
        let tree_id = {
            let mut index = repo.index().unwrap();
            index.write_tree().unwrap()
        };
        let tree = repo.find_tree(tree_id).unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "initial", &tree, &[]).unwrap();
        drop(tree);
        repo
    }

    #[test]
    fn test_prepare_repos_empty_path_returns_all() -> anyhow::Result<()> {
        let mut conn = WorkspaceRegistry::init_in_memory()?;
        let _ = WorkspaceRegistry::seed_test_repo(&mut conn, "repo1")?;
        let _ = WorkspaceRegistry::seed_test_repo(&mut conn, "repo2")?;

        let repos = prepare_repos(&mut conn, "")?;
        assert_eq!(repos.len(), 2);
        assert!(repos.iter().any(|r| r.id == "repo1"));
        assert!(repos.iter().any(|r| r.id == "repo2"));
        Ok(())
    }

    #[test]
    fn test_prepare_repos_matching_path_returns_one() -> anyhow::Result<()> {
        let mut conn = WorkspaceRegistry::init_in_memory()?;
        let tmp = tempfile::tempdir()?;
        let path = tmp.path().join("myrepo");
        std::fs::create_dir(&path)?;

        let repo = RepoEntry {
            id: "myrepo".to_string(),
            local_path: path.clone(),
            tags: vec![],
            language: Some("rust".to_string()),
            discovered_at: chrono::Utc::now(),
            workspace_type: "git".to_string(),
            data_tier: "private".to_string(),
            last_synced_at: None,
            stars: None,
            remotes: vec![],
        };
        crate::registry::repo::save_repo(&mut conn, &repo)?;

        let repos = prepare_repos(&mut conn, path.to_str().unwrap())?;
        assert_eq!(repos.len(), 1);
        assert_eq!(repos[0].id, "myrepo");
        Ok(())
    }

    #[test]
    fn test_prepare_repos_nonexistent_path_errors() -> anyhow::Result<()> {
        let mut conn = WorkspaceRegistry::init_in_memory()?;
        let result = prepare_repos(&mut conn, "/nonexistent/path/12345");
        assert!(result.is_err());
        Ok(())
    }

    #[test]
    fn test_prepare_repos_unregistered_existing_path_auto_registers() -> anyhow::Result<()> {
        let mut conn = WorkspaceRegistry::init_in_memory()?;
        let tmp = tempfile::tempdir()?;
        let path = tmp.path().join("unregistered");
        std::fs::create_dir(&path)?;
        let _ = init_git_repo(&path);

        let repos = prepare_repos(&mut conn, path.to_str().unwrap())?;
        assert_eq!(repos.len(), 1);
        // Use file_name comparison to avoid Windows short-name (8.3) path mismatches
        assert_eq!(repos[0].local_path.file_name(), path.file_name());
        assert!(repos[0].local_path.exists());
        // Verify it was saved to registry
        let all = crate::registry::repo::list_repos(&conn)?;
        assert_eq!(all.len(), 1);
        Ok(())
    }

    #[test]
    fn test_save_and_get_repo_index_state() -> anyhow::Result<()> {
        let mut conn = WorkspaceRegistry::init_in_memory()?;
        let tmp = tempfile::tempdir()?;
        let path = tmp.path().join("gitrepo");
        std::fs::create_dir(&path)?;
        let repo = git2::Repository::init(&path)?;
        let mut config = repo.config().unwrap();
        config.set_str("user.name", "Test").unwrap();
        config.set_str("user.email", "test@example.com").unwrap();
        let sig = repo.signature().unwrap();
        let tree_id = {
            let mut index = repo.index().unwrap();
            index.write_tree().unwrap()
        };
        let tree = repo.find_tree(tree_id).unwrap();
        let oid = repo.commit(Some("HEAD"), &sig, &sig, "initial", &tree, &[])?;

        save_repo_index_state(&mut conn, "test-repo", &oid.to_string())?;

        let hash: Option<String> = conn
            .query_row(
                "SELECT last_commit_hash FROM repo_index_state WHERE repo_id = ?1",
                ["test-repo"],
                |row| row.get(0),
            )
            .unwrap_or(None);
        assert_eq!(hash, Some(oid.to_string()));
        Ok(())
    }
}
