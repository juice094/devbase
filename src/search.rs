#![allow(dead_code)]

use std::path::PathBuf;
use tantivy::{
    Index, IndexReader, IndexWriter, ReloadPolicy, TantivyDocument, TantivyError,
    collector::TopDocs,
    query::{BooleanQuery, Occur, QueryParser, TermQuery},
    schema::{STORED, Schema, TEXT, Value},
};

const INDEX_DIR: &str = "devbase/search_index";

fn index_path() -> Result<PathBuf, TantivyError> {
    let base = dirs::data_local_dir()
        .ok_or_else(|| TantivyError::InvalidArgument("local data dir not found".into()))?;
    Ok(base.join(INDEX_DIR))
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
    std::fs::create_dir_all(&path)?;
    let schema = build_schema();
    let index = match Index::open_in_dir(&path) {
        Ok(idx) => {
            if idx.schema() == schema {
                idx
            } else {
                drop(idx);
                let _ = std::fs::remove_dir_all(&path);
                std::fs::create_dir_all(&path)?;
                Index::create_in_dir(&path, schema)?
            }
        }
        Err(_) => Index::create_in_dir(&path, schema)?,
    };
    let reader = index
        .reader_builder()
        .reload_policy(ReloadPolicy::OnCommitWithDelay)
        .try_into()?;
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
    let id_f = schema.get_field("id").unwrap();
    let title_f = schema.get_field("title").unwrap();
    let content_f = schema.get_field("content").unwrap();
    let tags_f = schema.get_field("tags").unwrap();
    let doc_type_f = schema.get_field("doc_type").unwrap();

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
    let id = schema.get_field("id").unwrap();
    let term = tantivy::Term::from_field_text(id, repo_id);
    writer.delete_term(term);
    Ok(())
}

pub fn commit_writer(writer: &mut IndexWriter) -> Result<(), TantivyError> {
    writer.commit()?;
    Ok(())
}

pub fn index_is_empty() -> Result<bool, TantivyError> {
    let (_index, reader) = init_index()?;
    let searcher = reader.searcher();
    Ok(searcher.num_docs() == 0)
}

pub fn search_repos(query_str: &str, limit: usize) -> Result<Vec<(String, f32)>, TantivyError> {
    search_by_doc_type(query_str, limit, None)
}

pub fn search_vault(query_str: &str, limit: usize) -> Result<Vec<(String, f32)>, TantivyError> {
    search_by_doc_type(query_str, limit, Some("vault"))
}

fn search_by_doc_type(
    query_str: &str,
    limit: usize,
    doc_type_filter: Option<&str>,
) -> Result<Vec<(String, f32)>, TantivyError> {
    let (index, reader) = init_index()?;
    let schema = index.schema();
    let searcher = reader.searcher();

    let title = schema.get_field("title").unwrap();
    let content = schema.get_field("content").unwrap();
    let tags = schema.get_field("tags").unwrap();
    let doc_type_f = schema.get_field("doc_type").unwrap();

    let query_parser = QueryParser::for_index(&index, vec![title, content, tags]);
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

    let id_field = schema.get_field("id").unwrap();
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
    let schema = build_schema();
    let dir = tantivy::directory::MmapDirectory::open(&path)?;
    let idx = Index::open_or_create(dir, schema.clone())?;
    Ok((idx, schema))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static SEARCH_TEST_LOCK: Mutex<()> = Mutex::new(());

    fn with_temp_index<F>(f: F)
    where
        F: FnOnce(&Index, &Schema, &mut IndexWriter),
    {
        let _guard = SEARCH_TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let tmp = tempfile::tempdir().unwrap();
        let old = std::env::var("LOCALAPPDATA").ok();
        unsafe {
            std::env::set_var("LOCALAPPDATA", tmp.path());
        }
        // Rebuild index path inside temp dir
        let schema = build_schema();
        let index_dir = tmp.path().join(INDEX_DIR);
        std::fs::create_dir_all(&index_dir).unwrap();
        let idx = Index::create_in_dir(&index_dir, schema.clone()).unwrap();
        let mut writer = idx.writer(15_000_000).unwrap();
        f(&idx, &schema, &mut writer);
        if let Some(v) = old {
            unsafe {
                std::env::set_var("LOCALAPPDATA", v);
            }
        } else {
            unsafe {
                std::env::remove_var("LOCALAPPDATA");
            }
        }
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
}
