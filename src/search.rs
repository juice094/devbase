#![allow(dead_code)]

use std::path::PathBuf;
use tantivy::{
    collector::TopDocs,
    query::QueryParser,
    schema::{Schema, Value, STORED, TEXT},
    Index, IndexReader, IndexWriter, ReloadPolicy, TantivyDocument, TantivyError,
};

const INDEX_DIR: &str = "devbase/search_index";

fn index_path() -> PathBuf {
    dirs::data_local_dir()
        .expect("local data dir")
        .join(INDEX_DIR)
}

fn build_schema() -> Schema {
    let mut schema_builder = Schema::builder();
    schema_builder.add_text_field("id", TEXT | STORED);
    schema_builder.add_text_field("title", TEXT | STORED);
    schema_builder.add_text_field("content", TEXT);
    schema_builder.add_text_field("tags", TEXT);
    schema_builder.build()
}

pub fn init_index() -> Result<(Index, IndexReader), TantivyError> {
    let path = index_path();
    std::fs::create_dir_all(&path)?;
    let schema = build_schema();
    let index = Index::open_or_create(
        tantivy::directory::MmapDirectory::open(&path)?,
        schema.clone(),
    )?;
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
    repo_id: &str,
    title: &str,
    content: &str,
    tags: &[String],
) -> Result<(), TantivyError> {
    let schema = open_index().0.schema();
    let id = schema.get_field("id").unwrap();
    let title_f = schema.get_field("title").unwrap();
    let content_f = schema.get_field("content").unwrap();
    let tags_f = schema.get_field("tags").unwrap();

    let mut doc = TantivyDocument::default();
    doc.add_text(id, repo_id);
    doc.add_text(title_f, title);
    doc.add_text(content_f, content);
    doc.add_text(tags_f, &tags.join(","));

    writer.add_document(doc)?;
    Ok(())
}

pub fn delete_repo_doc(writer: &mut IndexWriter, schema: &Schema, repo_id: &str) -> Result<(), TantivyError> {
    let id = schema.get_field("id").unwrap();
    let term = tantivy::Term::from_field_text(id, repo_id);
    writer.delete_term(term);
    Ok(())
}

pub fn commit_writer(writer: &mut IndexWriter) -> Result<(), TantivyError> {
    writer.commit()?;
    Ok(())
}

pub fn search_repos(query_str: &str, limit: usize) -> Result<Vec<(String, f32)>, TantivyError> {
    let (index, reader) = init_index()?;
    let schema = index.schema();
    let searcher = reader.searcher();

    let title = schema.get_field("title").unwrap();
    let content = schema.get_field("content").unwrap();
    let tags = schema.get_field("tags").unwrap();

    let query_parser = QueryParser::for_index(&index, vec![title, content, tags]);
    let query = query_parser.parse_query(query_str)?;

    let top_docs = searcher.search(&query, &TopDocs::with_limit(limit).order_by_score())?;

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

fn open_index() -> (Index, Schema) {
    let path = index_path();
    let schema = build_schema();
    let idx = Index::open_or_create(
        tantivy::directory::MmapDirectory::open(&path).expect("open index dir"),
        schema.clone(),
    )
    .expect("open or create index");
    (idx, schema)
}
