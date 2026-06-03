// SPDX-License-Identifier: MIT
// Copyright (c) 2026 juice094
//! Tantivy-based BM25 search index for code symbols.
//!
//! Replaces the SQLite `LIKE` fallback in `hybrid.rs` with proper
//! full-text retrieval over symbol names and signatures.

use crate::storage::StorageBackend;
use std::path::Path;
use tantivy::{
    Index, IndexReader, IndexWriter, ReloadPolicy, TantivyDocument, TantivyError,
    collector::TopDocs,
    query::QueryParser,
    schema::{STORED, Schema, TEXT, Value},
};

fn symbol_index_path() -> Result<std::path::PathBuf, TantivyError> {
    crate::storage::DefaultStorageBackend {}
        .symbol_index_path()
        .map_err(|e| TantivyError::InvalidArgument(e.to_string()))
}

fn build_schema() -> Schema {
    let mut sb = Schema::builder();
    sb.add_text_field("repo_id", TEXT | STORED);
    sb.add_text_field("name", TEXT | STORED);
    sb.add_text_field("signature", TEXT | STORED);
    sb.add_text_field("file_path", TEXT | STORED);
    sb.add_text_field("line_start", STORED);
    sb.build()
}

/// Initialize the symbol index (uses default storage backend).
pub fn init_index() -> Result<(Index, IndexReader), TantivyError> {
    let path = symbol_index_path()?;
    init_index_at(&path)
}

/// Initialize at an explicit path (tests + hermetic isolation).
pub fn init_index_at(path: &Path) -> Result<(Index, IndexReader), TantivyError> {
    std::fs::create_dir_all(path)?;
    let schema = build_schema();
    let index = match Index::open_in_dir(path) {
        Ok(idx) => {
            if idx.schema() == schema {
                idx
            } else {
                drop(idx);
                let _ = std::fs::remove_dir_all(path);
                std::fs::create_dir_all(path)?;
                Index::create_in_dir(path, schema)?
            }
        }
        Err(_) => Index::create_in_dir(path, schema)?,
    };
    let reader = index.reader_builder().reload_policy(ReloadPolicy::Manual).try_into()?;
    Ok((index, reader))
}

pub fn get_writer(index: &Index) -> Result<IndexWriter, TantivyError> {
    index.writer(50_000_000)
}

/// Add a single code symbol document.
pub fn add_symbol_doc(
    writer: &mut IndexWriter,
    schema: &Schema,
    repo_id: &str,
    name: &str,
    signature: Option<&str>,
    file_path: &str,
    line_start: usize,
) -> Result<(), TantivyError> {
    let repo_f = schema.get_field("repo_id")?;
    let name_f = schema.get_field("name")?;
    let sig_f = schema.get_field("signature")?;
    let path_f = schema.get_field("file_path")?;
    let line_f = schema.get_field("line_start")?;

    let mut doc = TantivyDocument::default();
    doc.add_text(repo_f, repo_id);
    doc.add_text(name_f, name);
    if let Some(s) = signature {
        doc.add_text(sig_f, s);
    }
    doc.add_text(path_f, file_path);
    doc.add_text(line_f, line_start.to_string());
    writer.add_document(doc)?;
    Ok(())
}

/// Bulk-add symbols for a repo (deletes existing repo symbols first).
pub fn add_symbols(
    writer: &mut IndexWriter,
    schema: &Schema,
    repo_id: &str,
    symbols: &[crate::semantic_index::CodeSymbol],
) -> Result<(), TantivyError> {
    // Delete existing symbols for this repo
    delete_repo_symbols(writer, schema, repo_id)?;
    for sym in symbols {
        add_symbol_doc(
            writer,
            schema,
            repo_id,
            &sym.name,
            sym.signature.as_deref(),
            &sym.file_path.to_string_lossy(),
            sym.line_start,
        )?;
    }
    Ok(())
}

/// Delete all symbols belonging to a repo.
pub fn delete_repo_symbols(
    writer: &mut IndexWriter,
    schema: &Schema,
    repo_id: &str,
) -> Result<(), TantivyError> {
    let repo_f = schema.get_field("repo_id")?;
    let term = tantivy::Term::from_field_text(repo_f, repo_id);
    writer.delete_term(term);
    Ok(())
}

pub fn commit_writer(writer: &mut IndexWriter) -> Result<(), TantivyError> {
    writer.commit()?;
    Ok(())
}

/// BM25 search over code symbols, optionally filtered by repo_id.
///
/// Queries the `name` and `signature` fields. Returns
/// Vec<(repo_id, name, file_path, line_start, bm25_score)>.
pub fn search_symbols(
    query_str: &str,
    limit: usize,
    repo_id: Option<&str>,
) -> Result<Vec<crate::semantic_index::SemanticSearchRow>, TantivyError> {
    let path = symbol_index_path()?;
    search_symbols_at(&path, query_str, limit, repo_id)
}

/// Search at an explicit path.
pub fn search_symbols_at(
    path: &Path,
    query_str: &str,
    limit: usize,
    repo_id: Option<&str>,
) -> Result<Vec<crate::semantic_index::SemanticSearchRow>, TantivyError> {
    let (index, reader) = init_index_at(path)?;
    let schema = index.schema();
    let searcher = reader.searcher();

    let name_f = schema.get_field("name")?;
    let sig_f = schema.get_field("signature")?;
    let repo_f = schema.get_field("repo_id")?;
    let path_f = schema.get_field("file_path")?;
    let line_f = schema.get_field("line_start")?;

    let parser = QueryParser::for_index(&index, vec![name_f, sig_f]);
    let text_query = parser.parse_query(query_str)?;

    // Build combined query: text_query AND repo_id:filter (if specified)
    let final_query: Box<dyn tantivy::query::Query> = if let Some(rid) = repo_id {
        let repo_term_query = tantivy::query::TermQuery::new(
            tantivy::Term::from_field_text(repo_f, rid),
            tantivy::schema::IndexRecordOption::Basic,
        );
        Box::new(tantivy::query::BooleanQuery::new(vec![
            (tantivy::query::Occur::Must, text_query),
            (tantivy::query::Occur::Must, Box::new(repo_term_query)),
        ]))
    } else {
        text_query
    };

    let top_docs = searcher.search(&*final_query, &TopDocs::with_limit(limit).order_by_score())?;

    let mut results = Vec::new();
    for (score, doc_addr) in top_docs {
        let doc: TantivyDocument = searcher.doc(doc_addr)?;
        let repo_id = doc.get_first(repo_f).and_then(|v| v.as_str()).unwrap_or("").to_string();
        let name = doc.get_first(name_f).and_then(|v| v.as_str()).unwrap_or("").to_string();
        let file_path = doc.get_first(path_f).and_then(|v| v.as_str()).unwrap_or("").to_string();
        let line_start: i64 = doc
            .get_first(line_f)
            .and_then(|v| v.as_str())
            .unwrap_or("0")
            .parse()
            .unwrap_or(0);
        results.push((repo_id, name, file_path, line_start, score));
    }
    Ok(results)
}

/// List all repo IDs in the symbol index.
pub fn list_indexed_repo_ids() -> Result<Vec<String>, TantivyError> {
    let path = symbol_index_path()?;
    list_indexed_repo_ids_at(&path)
}

pub fn list_indexed_repo_ids_at(path: &Path) -> Result<Vec<String>, TantivyError> {
    let (index, reader) = init_index_at(path)?;
    let searcher = reader.searcher();
    let schema = index.schema();
    let repo_f = schema.get_field("repo_id")?;

    let all_query = tantivy::query::AllQuery;
    let top_docs = searcher.search(&all_query, &TopDocs::with_limit(10_000).order_by_score())?;

    let mut ids = Vec::new();
    for (_score, doc_addr) in top_docs {
        let doc: TantivyDocument = searcher.doc(doc_addr)?;
        if let Some(id) = doc.get_first(repo_f).and_then(|v| v.as_str()) {
            ids.push(id.to_string());
        }
    }
    ids.sort_unstable();
    ids.dedup();
    Ok(ids)
}

#[cfg(test)]
pub(crate) static SYMBOL_INDEX_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(test)]
mod tests {
    use super::*;

    fn with_temp_symbol_index<F>(f: F)
    where
        F: FnOnce(&Index, &Schema, &mut IndexWriter),
    {
        let _guard = SYMBOL_INDEX_TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let schema = build_schema();
        let idx = Index::create_in_ram(schema.clone());
        let mut writer = idx.writer(15_000_000).unwrap();
        f(&idx, &schema, &mut writer);
        drop(writer);
        drop(idx);
    }

    #[test]
    fn test_add_and_search_symbol() {
        with_temp_symbol_index(|idx, schema, writer| {
            add_symbol_doc(
                writer,
                schema,
                "repo1",
                "handle_error",
                Some("pub fn handle_error(e: Error)"),
                "src/lib.rs",
                42,
            )
            .unwrap();
            writer.commit().unwrap();

            let reader = idx.reader().unwrap();
            let results = search_with_reader(&reader, idx, "handle", 10, Some("repo1")).unwrap();
            assert_eq!(results.len(), 1);
            assert_eq!(results[0].1, "handle_error");
            assert_eq!(results[0].3, 42);
        });
    }

    #[test]
    fn test_delete_repo_symbols() {
        with_temp_symbol_index(|idx, schema, writer| {
            add_symbol_doc(writer, schema, "repo1", "foo", None, "a.rs", 1).unwrap();
            add_symbol_doc(writer, schema, "repo2", "bar", None, "b.rs", 2).unwrap();
            writer.commit().unwrap();

            delete_repo_symbols(writer, schema, "repo1").unwrap();
            writer.commit().unwrap();

            let reader = idx.reader().unwrap();
            let results = search_with_reader(&reader, idx, "foo", 10, Some("repo1")).unwrap();
            assert!(results.is_empty());

            let results = search_with_reader(&reader, idx, "bar", 10, Some("repo2")).unwrap();
            assert_eq!(results.len(), 1);
        });
    }

    #[test]
    fn test_search_signature_match() {
        with_temp_symbol_index(|idx, schema, writer| {
            add_symbol_doc(
                writer,
                schema,
                "repo1",
                "authenticate",
                Some("pub fn authenticate(token: &str)"),
                "src/auth.rs",
                10,
            )
            .unwrap();
            writer.commit().unwrap();

            let reader = idx.reader().unwrap();
            let results = search_with_reader(&reader, idx, "token", 10, Some("repo1")).unwrap();
            assert_eq!(results.len(), 1);
            assert_eq!(results[0].1, "authenticate");
        });
    }

    // Helper that works with an existing reader for in-RAM tests.
    fn search_with_reader(
        reader: &IndexReader,
        index: &Index,
        query: &str,
        limit: usize,
        repo_id: Option<&str>,
    ) -> Result<Vec<crate::semantic_index::SemanticSearchRow>, TantivyError> {
        let schema = index.schema();
        let searcher = reader.searcher();
        let name_f = schema.get_field("name").unwrap();
        let sig_f = schema.get_field("signature").unwrap();
        let repo_f = schema.get_field("repo_id").unwrap();
        let path_f = schema.get_field("file_path").unwrap();
        let line_f = schema.get_field("line_start").unwrap();

        let parser = QueryParser::for_index(index, vec![name_f, sig_f]);
        let text_query = parser.parse_query(query).unwrap();

        let final_query: Box<dyn tantivy::query::Query> = if let Some(rid) = repo_id {
            let repo_term = tantivy::query::TermQuery::new(
                tantivy::Term::from_field_text(repo_f, rid),
                tantivy::schema::IndexRecordOption::Basic,
            );
            Box::new(tantivy::query::BooleanQuery::new(vec![
                (tantivy::query::Occur::Must, text_query),
                (tantivy::query::Occur::Must, Box::new(repo_term)),
            ]))
        } else {
            text_query
        };

        let top_docs =
            searcher.search(&*final_query, &TopDocs::with_limit(limit).order_by_score())?;

        let mut results = Vec::new();
        for (score, doc_addr) in top_docs {
            let doc: TantivyDocument = searcher.doc(doc_addr)?;
            let repo_id = doc.get_first(repo_f).and_then(|v| v.as_str()).unwrap_or("").to_string();
            let name = doc.get_first(name_f).and_then(|v| v.as_str()).unwrap_or("").to_string();
            let file_path =
                doc.get_first(path_f).and_then(|v| v.as_str()).unwrap_or("").to_string();
            let line_start: i64 = doc
                .get_first(line_f)
                .and_then(|v| v.as_str())
                .unwrap_or("0")
                .parse()
                .unwrap_or(0);
            results.push((repo_id, name, file_path, line_start, score));
        }
        Ok(results)
    }
}
