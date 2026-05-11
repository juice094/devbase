#![allow(dead_code)]
// SPDX-License-Identifier: MIT
// Copyright (c) 2026 juice094

pub mod hybrid;
pub mod symbol_index;

use crate::storage::StorageBackend;
use std::path::PathBuf;
use tantivy::{
    Index, IndexReader, IndexWriter, ReloadPolicy, TantivyDocument, TantivyError,
    collector::TopDocs,
    query::{BooleanQuery, Occur, QueryParser, TermQuery},
    schema::{STORED, Schema, TEXT, Value},
};

const INDEX_DIR: &str = "devbase/search_index";

fn index_path() -> Result<PathBuf, TantivyError> {
    crate::storage::DefaultStorageBackend {}
        .index_path()
        .map_err(|e| TantivyError::InvalidArgument(e.to_string()))
}

fn build_schema() -> Schema {
    let mut schema_builder = Schema::builder();
    schema_builder.add_text_field("id", TEXT | STORED);
    schema_builder.add_text_field("title", TEXT | STORED);
    schema_builder.add_text_field("content", TEXT);
    schema_builder.add_text_field("tags", TEXT);
    schema_builder.add_text_field("doc_type", TEXT | STORED);
    schema_builder.build()
}

pub fn init_index() -> Result<(Index, IndexReader), TantivyError> {
    let path = index_path()?;
    init_index_at(&path)
}

/// Initialize a Tantivy index at an explicit path, bypassing the global storage backend.
/// Used by tests and any code that needs hermetic index isolation.
pub fn init_index_at(path: &std::path::Path) -> Result<(Index, IndexReader), TantivyError> {
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

pub fn add_repo_doc(
    writer: &mut IndexWriter,
    schema: &Schema,
    repo_id: &str,
    title: &str,
    content: &str,
    tags: &[String],
) -> Result<(), TantivyError> {
    add_doc(writer, schema, repo_id, title, content, tags, "repo")
}

pub fn add_vault_doc(
    writer: &mut IndexWriter,
    schema: &Schema,
    note_id: &str,
    title: &str,
    content: &str,
    tags: &[String],
) -> Result<(), TantivyError> {
    add_doc(writer, schema, note_id, title, content, tags, "vault")
}

fn add_doc(
    writer: &mut IndexWriter,
    schema: &Schema,
    id: &str,
    title: &str,
    content: &str,
    tags: &[String],
    doc_type: &str,
) -> Result<(), TantivyError> {
    let id_f = schema.get_field("id")?;
    let title_f = schema.get_field("title")?;
    let content_f = schema.get_field("content")?;
    let tags_f = schema.get_field("tags")?;
    let doc_type_f = schema.get_field("doc_type")?;

    let mut doc = TantivyDocument::default();
    doc.add_text(id_f, id);
    doc.add_text(title_f, title);
    doc.add_text(content_f, content);
    doc.add_text(tags_f, tags.join(","));
    doc.add_text(doc_type_f, doc_type);

    writer.add_document(doc)?;
    Ok(())
}

pub fn delete_repo_doc(
    writer: &mut IndexWriter,
    schema: &Schema,
    repo_id: &str,
) -> Result<(), TantivyError> {
    let id = schema.get_field("id")?;
    let term = tantivy::Term::from_field_text(id, repo_id);
    writer.delete_term(term);
    Ok(())
}

pub fn commit_writer(writer: &mut IndexWriter) -> Result<(), TantivyError> {
    writer.commit()?;
    Ok(())
}

pub fn index_is_empty() -> Result<bool, TantivyError> {
    let path = index_path()?;
    index_is_empty_at(&path)
}

pub fn index_is_empty_at(path: &std::path::Path) -> Result<bool, TantivyError> {
    let (_index, reader) = init_index_at(path)?;
    let searcher = reader.searcher();
    Ok(searcher.num_docs() == 0)
}

/// List all repo IDs currently stored in the Tantivy index.
/// Used by startup consistency scan to detect orphan documents.
pub fn list_indexed_repo_ids() -> Result<Vec<String>, TantivyError> {
    let (index, reader) = init_index()?;
    list_indexed_repo_ids_with_reader(&reader, &index)
}

/// List indexed repo IDs using an already-opened index reader.
fn list_indexed_repo_ids_with_reader(
    reader: &IndexReader,
    index: &Index,
) -> Result<Vec<String>, TantivyError> {
    let searcher = reader.searcher();
    let schema = index.schema();
    let id_field = schema.get_field("id")?;

    let all_query = tantivy::query::AllQuery;
    // Use a generous limit; typical deployment has < 1000 repos.
    let top_docs = searcher.search(&all_query, &TopDocs::with_limit(10_000).order_by_score())?;

    let mut ids = Vec::new();
    for (_score, doc_address) in top_docs {
        let doc: TantivyDocument = searcher.doc(doc_address)?;
        if let Some(id) = doc.get_first(id_field).and_then(|v| v.as_str()) {
            ids.push(id.to_string());
        }
    }
    ids.sort_unstable();
    ids.dedup();
    Ok(ids)
}

/// List indexed repo IDs from an index at an explicit path.
pub fn list_indexed_repo_ids_at(path: &std::path::Path) -> Result<Vec<String>, TantivyError> {
    let (index, reader) = init_index_at(path)?;
    list_indexed_repo_ids_with_reader(&reader, &index)
}

/// Scan Tantivy index against SQLite entities table and remove orphan documents.
/// Returns the number of deleted orphan documents.
pub fn sync_index_to_db(conn: &rusqlite::Connection) -> anyhow::Result<usize> {
    let index_path = index_path()?;
    sync_index_to_db_at(&index_path, conn)
}

/// Sync Tantivy index to DB at an explicit index path, bypassing global storage backend.
pub fn sync_index_to_db_at(
    index_path: &std::path::Path,
    conn: &rusqlite::Connection,
) -> anyhow::Result<usize> {
    let (index, reader) = init_index_at(index_path)?;
    let tantivy_ids = list_indexed_repo_ids_with_reader(&reader, &index)?;
    let db_ids: Vec<String> = conn
        .prepare("SELECT id FROM entities WHERE entity_type = ?1")?
        .query_map([crate::registry::ENTITY_TYPE_REPO], |row| row.get(0))?
        .filter_map(Result::ok)
        .collect();
    let db_set: std::collections::HashSet<String> = db_ids.into_iter().collect();
    let orphans: Vec<String> = tantivy_ids.into_iter().filter(|id| !db_set.contains(id)).collect();
    if orphans.is_empty() {
        return Ok(0);
    }
    let (index, schema) = open_index_at(index_path)?;
    let mut writer = get_writer(&index)?;
    for repo_id in &orphans {
        delete_repo_doc(&mut writer, &schema, repo_id)?;
    }
    commit_writer(&mut writer)?;
    Ok(orphans.len())
}

pub fn search_repos(query_str: &str, limit: usize) -> Result<Vec<(String, f32)>, TantivyError> {
    let path = index_path()?;
    search_repos_at(&path, query_str, limit)
}

pub fn search_repos_at(
    path: &std::path::Path,
    query_str: &str,
    limit: usize,
) -> Result<Vec<(String, f32)>, TantivyError> {
    search_by_doc_type_at(path, query_str, limit, None)
}

pub fn search_vault(query_str: &str, limit: usize) -> Result<Vec<(String, f32)>, TantivyError> {
    search_by_doc_type(query_str, limit, Some("vault"))
}

fn search_by_doc_type(
    query_str: &str,
    limit: usize,
    doc_type_filter: Option<&str>,
) -> Result<Vec<(String, f32)>, TantivyError> {
    let path = index_path()?;
    search_by_doc_type_at(&path, query_str, limit, doc_type_filter)
}

fn search_by_doc_type_at(
    path: &std::path::Path,
    query_str: &str,
    limit: usize,
    doc_type_filter: Option<&str>,
) -> Result<Vec<(String, f32)>, TantivyError> {
    let (index, reader) = init_index_at(path)?;
    search_with_reader(&index, &reader, query_str, limit, doc_type_filter)
}

fn search_with_reader(
    index: &Index,
    reader: &IndexReader,
    query_str: &str,
    limit: usize,
    doc_type_filter: Option<&str>,
) -> Result<Vec<(String, f32)>, TantivyError> {
    let schema = index.schema();
    let searcher = reader.searcher();

    let title = schema.get_field("title")?;
    let content = schema.get_field("content")?;
    let tags = schema.get_field("tags")?;
    let doc_type_f = schema.get_field("doc_type")?;

    let query_parser = QueryParser::for_index(index, vec![title, content, tags]);
    let text_query = query_parser.parse_query(query_str)?;

    // Build combined query: text_query AND doc_type:filter (if specified)
    let final_query: Box<dyn tantivy::query::Query> = if let Some(dt) = doc_type_filter {
        let term_query = TermQuery::new(
            tantivy::Term::from_field_text(doc_type_f, dt),
            tantivy::schema::IndexRecordOption::Basic,
        );
        Box::new(BooleanQuery::new(vec![
            (Occur::Must, text_query),
            (Occur::Must, Box::new(term_query)),
        ]))
    } else {
        text_query
    };

    let top_docs = searcher.search(&*final_query, &TopDocs::with_limit(limit).order_by_score())?;

    let id_field = schema.get_field("id")?;
    let mut results = Vec::new();
    for (score, doc_address) in top_docs {
        let doc: TantivyDocument = searcher.doc(doc_address)?;
        if let Some(id) = doc.get_first(id_field).and_then(|v| v.as_str()) {
            results.push((id.to_string(), score));
        }
    }
    Ok(results)
}

fn open_index() -> Result<(Index, Schema), TantivyError> {
    let path = index_path()?;
    open_index_at(&path)
}

fn open_index_at(path: &std::path::Path) -> Result<(Index, Schema), TantivyError> {
    let schema = build_schema();
    let dir = tantivy::directory::MmapDirectory::open(path)?;
    let idx = Index::open_or_create(dir, schema.clone())?;
    Ok((idx, schema))
}

#[cfg(test)]
pub(crate) static SEARCH_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(test)]
mod tests {
    use super::*;

    fn with_temp_index<F>(f: F)
    where
        F: FnOnce(&Index, &Schema, &mut IndexWriter),
    {
        let _guard = super::SEARCH_TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let schema = build_schema();
        // Use RamDirectory to avoid Windows file-lock races with MmapDirectory.
        let idx = Index::create_in_ram(schema.clone());
        let mut writer = idx.writer(15_000_000).unwrap();
        f(&idx, &schema, &mut writer);
        drop(writer);
        drop(idx);
    }

    #[test]
    fn test_build_schema() {
        let schema = build_schema();
        assert!(schema.get_field("id").is_ok());
        assert!(schema.get_field("title").is_ok());
        assert!(schema.get_field("content").is_ok());
        assert!(schema.get_field("tags").is_ok());
    }

    #[test]
    fn test_add_and_search_repo() {
        with_temp_index(|_idx, schema, writer| {
            add_repo_doc(
                writer,
                schema,
                "repo1",
                "devbase",
                "A developer workspace manager",
                &["rust".into(), "cli".into()],
            )
            .unwrap();
            writer.commit().unwrap();

            let reader = _idx.reader().unwrap();
            let searcher = reader.searcher();
            let title = schema.get_field("title").unwrap();
            let content = schema.get_field("content").unwrap();
            let tags = schema.get_field("tags").unwrap();
            let parser = QueryParser::for_index(_idx, vec![title, content, tags]);
            let query = parser.parse_query("workspace").unwrap();
            let top_docs: Vec<(f32, tantivy::DocAddress)> =
                searcher.search(&query, &TopDocs::with_limit(10).order_by_score()).unwrap();
            assert_eq!(top_docs.len(), 1);
        });
    }

    #[test]
    fn test_delete_repo_doc() {
        with_temp_index(|_idx, schema, writer| {
            add_repo_doc(writer, schema, "repo1", "devbase", "A developer workspace manager", &[])
                .unwrap();
            writer.commit().unwrap();

            delete_repo_doc(writer, schema, "repo1").unwrap();
            writer.commit().unwrap();

            let reader = _idx.reader().unwrap();
            let searcher = reader.searcher();
            let title = schema.get_field("title").unwrap();
            let content = schema.get_field("content").unwrap();
            let tags = schema.get_field("tags").unwrap();
            let parser = QueryParser::for_index(_idx, vec![title, content, tags]);
            let query = parser.parse_query("devbase").unwrap();
            let top_docs: Vec<(f32, tantivy::DocAddress)> =
                searcher.search(&query, &TopDocs::with_limit(10).order_by_score()).unwrap();
            assert!(top_docs.is_empty());
        });
    }

    #[test]
    fn test_add_vault_doc() {
        with_temp_index(|idx, schema, writer| {
            add_vault_doc(
                writer,
                schema,
                "note1",
                "My Note",
                "Vault note content",
                &["tag1".into()],
            )
            .unwrap();
            writer.commit().unwrap();

            let reader = idx.reader().unwrap();
            let searcher = reader.searcher();
            let title = schema.get_field("title").unwrap();
            let content = schema.get_field("content").unwrap();
            let tags = schema.get_field("tags").unwrap();
            let parser = QueryParser::for_index(idx, vec![title, content, tags]);
            let query = parser.parse_query("Vault").unwrap();
            let top_docs: Vec<(f32, tantivy::DocAddress)> =
                searcher.search(&query, &TopDocs::with_limit(10).order_by_score()).unwrap();
            assert_eq!(top_docs.len(), 1);
        });
    }

    #[test]
    fn test_index_is_empty() {
        with_temp_index(|idx, _schema, writer| {
            let reader =
                idx.reader_builder().reload_policy(ReloadPolicy::Manual).try_into().unwrap();
            let searcher = reader.searcher();
            assert_eq!(searcher.num_docs(), 0);

            add_repo_doc(writer, _schema, "repo1", "title", "content", &[]).unwrap();
            writer.commit().unwrap();

            // Re-create reader to pick up committed docs
            let reader =
                idx.reader_builder().reload_policy(ReloadPolicy::Manual).try_into().unwrap();
            let searcher = reader.searcher();
            assert!(searcher.num_docs() > 0);
        });
    }

    #[test]
    fn test_search_repos() {
        with_temp_index(|index, schema, writer| {
            add_repo_doc(
                writer,
                schema,
                "repo1",
                "devbase",
                "developer workspace manager",
                &["rust".into()],
            )
            .unwrap();
            add_vault_doc(writer, schema, "note1", "My Note", "note content", &[]).unwrap();
            commit_writer(writer).unwrap();

            let reader =
                index.reader_builder().reload_policy(ReloadPolicy::Manual).try_into().unwrap();

            let results = search_with_reader(index, &reader, "workspace", 10, None).unwrap();
            assert_eq!(results.len(), 1);
            assert_eq!(results[0].0, "repo1");

            // search_repos does not filter by doc_type, so vault content is also searchable
            let note_results = search_with_reader(index, &reader, "note", 10, None).unwrap();
            assert_eq!(note_results.len(), 1);
            assert_eq!(note_results[0].0, "note1");
        });
    }

    #[test]
    fn test_search_vault() {
        with_temp_index(|index, schema, writer| {
            add_repo_doc(writer, schema, "repo1", "devbase", "developer workspace manager", &[])
                .unwrap();
            add_vault_doc(writer, schema, "note1", "My Note", "vault note content", &[]).unwrap();
            commit_writer(writer).unwrap();

            let reader =
                index.reader_builder().reload_policy(ReloadPolicy::Manual).try_into().unwrap();

            let results = search_with_reader(index, &reader, "vault", 10, Some("vault")).unwrap();
            assert_eq!(results.len(), 1);
            assert_eq!(results[0].0, "note1");

            // repo doc should not appear in vault search
            let repo_results =
                search_with_reader(index, &reader, "workspace", 10, Some("vault")).unwrap();
            assert!(repo_results.is_empty());
        });
    }

    #[test]
    fn test_list_indexed_repo_ids() {
        with_temp_index(|index, schema, writer| {
            // Empty index
            {
                let reader =
                    index.reader_builder().reload_policy(ReloadPolicy::Manual).try_into().unwrap();
                let ids = list_indexed_repo_ids_from_reader(&reader, index).unwrap();
                assert!(ids.is_empty());
            }

            add_repo_doc(writer, schema, "repo_a", "Title A", "content a", &[]).unwrap();
            add_repo_doc(writer, schema, "repo_b", "Title B", "content b", &[]).unwrap();
            commit_writer(writer).unwrap();

            let reader =
                index.reader_builder().reload_policy(ReloadPolicy::Manual).try_into().unwrap();
            let ids = list_indexed_repo_ids_from_reader(&reader, index).unwrap();
            assert_eq!(ids.len(), 2);
            assert!(ids.contains(&"repo_a".to_string()));
            assert!(ids.contains(&"repo_b".to_string()));
        });
    }

    #[test]
    fn test_sync_index_to_db_removes_orphans() {
        let _guard = super::SEARCH_TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let backend = crate::storage::TempStorageBackend::new();
        let index_path = backend.index_path().unwrap();
        let db_path = backend.db_path().unwrap();

        // Initialize DB with full schema
        let conn = crate::registry::WorkspaceRegistry::init_db_at(&db_path).unwrap();

        // Add 2 repo docs to Tantivy
        let (index, _reader) = init_index_at(&index_path).unwrap();
        let mut writer = get_writer(&index).unwrap();
        let schema = index.schema();
        add_repo_doc(&mut writer, &schema, "foo", "Foo", "foo content", &[]).unwrap();
        add_repo_doc(&mut writer, &schema, "bar", "Bar", "bar content", &[]).unwrap();
        commit_writer(&mut writer).unwrap();
        drop(writer);
        drop(index);
        // Windows releases Tantivy mmap handles asynchronously.
        std::thread::sleep(std::time::Duration::from_millis(800));

        // Register only repo_a in SQLite
        conn.execute(
            "INSERT INTO entities (id, entity_type, name, local_path, metadata, created_at, updated_at)
             VALUES ('foo', ?1, 'Foo', '/tmp/foo', '{}', datetime('now'), datetime('now'))",
            [crate::registry::ENTITY_TYPE_REPO],
        )
        .unwrap();

        // Sync should delete bar (orphan)
        let deleted = sync_index_to_db_at(&index_path, &conn).unwrap();
        assert_eq!(deleted, 1);

        // Windows may need extra time before reopening the index
        std::thread::sleep(std::time::Duration::from_millis(1500));

        // Verify only foo remains in index
        let remaining = list_indexed_repo_ids_at(&index_path).unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0], "foo");
    }

    // Helper for test_list_indexed_repo_ids that works with an existing reader
    fn list_indexed_repo_ids_from_reader(
        reader: &tantivy::IndexReader,
        index: &tantivy::Index,
    ) -> Result<Vec<String>, TantivyError> {
        let searcher = reader.searcher();
        let schema = index.schema();
        let id_field = schema.get_field("id").expect("schema field 'id' defined in init_index");
        let all_query = tantivy::query::AllQuery;
        let top_docs =
            searcher.search(&all_query, &TopDocs::with_limit(10_000).order_by_score())?;
        let mut ids = Vec::new();
        for (_score, doc_address) in top_docs {
            let doc: TantivyDocument = searcher.doc(doc_address)?;
            if let Some(id) = doc.get_first(id_field).and_then(|v| v.as_str()) {
                ids.push(id.to_string());
            }
        }
        ids.sort_unstable();
        ids.dedup();
        Ok(ids)
    }
}
