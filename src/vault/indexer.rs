// SPDX-License-Identifier: MIT
// Copyright (c) 2026 juice094
use crate::search;
use crate::vault::fs_io;
use tantivy::IndexWriter;
use tantivy::schema::Schema;
use tracing::info;

/// Index all vault notes from the registry into Tantivy.
///
/// This deletes the existing vault segment and re-adds all notes,
/// ensuring the search index stays in sync with the registry.
///
/// P1-1: filesystem-first — note content is read from disk on demand,
/// the SQLite registry only stores lightweight metadata.
pub fn reindex_vault(conn: &rusqlite::Connection) -> anyhow::Result<()> {
    let notes = crate::registry::vault::list_vault_notes(conn)?;

    let (index, _reader) = search::init_index()?;
    let mut writer = search::get_writer(&index)?;
    let schema = index.schema();
    reindex_vault_core(&notes, &mut writer, &schema)
}

fn reindex_vault_core(
    notes: &[crate::registry::VaultNote],
    writer: &mut IndexWriter,
    schema: &Schema,
) -> anyhow::Result<()> {
    // Delete all existing vault docs
    let doc_type = schema.get_field("doc_type")?;
    let term = tantivy::Term::from_field_text(doc_type, "vault");
    writer.delete_term(term);

    let mut indexed = 0;
    for note in notes {
        let title = note.title.as_deref().unwrap_or(&note.id);
        let tags: Vec<String> = note.tags.clone();

        // P1-1: read content from filesystem; fallback to empty string if unreadable
        let content = fs_io::read_note_body(&note.path).map(|(body, _fm)| body).unwrap_or_default();

        if let Err(e) = search::add_vault_doc(writer, schema, &note.id, title, &content, &tags) {
            tracing::warn!("Failed to index vault note {}: {}", note.id, e);
        } else {
            indexed += 1;
        }
    }

    writer.commit()?;
    info!("Vault reindex complete: {} notes indexed", indexed);
    Ok(())
}

/// Add or update a single vault note in the search index.
pub fn index_vault_note(note: &crate::registry::VaultNote) -> anyhow::Result<()> {
    let (index, _reader) = search::init_index()?;
    let mut writer = search::get_writer(&index)?;
    let schema = index.schema();
    index_vault_note_core(note, &mut writer, &schema)
}

fn index_vault_note_core(
    note: &crate::registry::VaultNote,
    writer: &mut IndexWriter,
    schema: &Schema,
) -> anyhow::Result<()> {
    // Delete old doc by id
    let id_field = schema.get_field("id")?;
    writer.delete_term(tantivy::Term::from_field_text(id_field, &note.id));

    let title = note.title.as_deref().unwrap_or(&note.id);
    search::add_vault_doc(writer, schema, &note.id, title, &note.content, &note.tags)?;
    writer.commit()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::VaultNote;
    use std::io::Write;

    fn init_isolated_index()
    -> (tempfile::TempDir, tantivy::Index, tantivy::IndexWriter, tantivy::schema::Schema) {
        let tmp = tempfile::tempdir().unwrap();
        let (index, _reader) = search::init_index_at(tmp.path()).unwrap();
        let writer = search::get_writer(&index).unwrap();
        let schema = index.schema();
        (tmp, index, writer, schema)
    }

    #[test]
    fn test_reindex_vault_core_empty() {
        let (_tmp, _index, mut writer, schema) = init_isolated_index();
        // Seed a dummy vault doc so we can verify deletion works
        search::add_vault_doc(&mut writer, &schema, "dummy", "Dummy", "content", &[]).unwrap();
        writer.commit().unwrap();

        let notes: Vec<VaultNote> = vec![];
        reindex_vault_core(&notes, &mut writer, &schema).unwrap();

        // After reindex with empty notes, vault docs should be gone
        let reader = _index.reader().unwrap();
        let searcher = reader.searcher();
        let doc_type = schema.get_field("doc_type").unwrap();
        let term = tantivy::Term::from_field_text(doc_type, "vault");
        let count = searcher
            .search(
                &tantivy::query::TermQuery::new(term, tantivy::schema::IndexRecordOption::Basic),
                &tantivy::collector::Count,
            )
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn test_reindex_vault_core_with_notes() {
        let tmp = tempfile::tempdir().unwrap();
        let md_path = tmp.path().join("note.md");
        let mut file = std::fs::File::create(&md_path).unwrap();
        writeln!(file, "# Hello\n\nThis is a test note.").unwrap();
        drop(file);

        let note = VaultNote {
            id: "note-1".to_string(),
            path: md_path.to_str().unwrap().to_string(),
            title: Some("Hello".to_string()),
            content: "ignored".to_string(),
            frontmatter: None,
            tags: vec!["tag1".to_string()],
            outgoing_links: vec![],
            block_refs: vec![],
            linked_repo: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        let (_tmp, _index, mut writer, schema) = init_isolated_index();
        reindex_vault_core(&[note], &mut writer, &schema).unwrap();

        // Verify the note was indexed by searching
        let reader = _index.reader().unwrap();
        let searcher = reader.searcher();
        let doc_type = schema.get_field("doc_type").unwrap();
        let term = tantivy::Term::from_field_text(doc_type, "vault");
        let count = searcher
            .search(
                &tantivy::query::TermQuery::new(term, tantivy::schema::IndexRecordOption::Basic),
                &tantivy::collector::Count,
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_index_vault_note_core_add() {
        let note = VaultNote {
            id: "note-add".to_string(),
            path: "/tmp/add.md".to_string(),
            title: Some("Add".to_string()),
            content: "new content".to_string(),
            frontmatter: None,
            tags: vec!["add".to_string()],
            outgoing_links: vec![],
            block_refs: vec![],
            linked_repo: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        let (_tmp, _index, mut writer, schema) = init_isolated_index();
        index_vault_note_core(&note, &mut writer, &schema).unwrap();

        let reader = _index.reader().unwrap();
        let searcher = reader.searcher();
        let doc_type = schema.get_field("doc_type").unwrap();
        let term = tantivy::Term::from_field_text(doc_type, "vault");
        let count = searcher
            .search(
                &tantivy::query::TermQuery::new(term, tantivy::schema::IndexRecordOption::Basic),
                &tantivy::collector::Count,
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_index_vault_note_core_update() {
        let note = VaultNote {
            id: "note-update".to_string(),
            path: "/tmp/update.md".to_string(),
            title: Some("Original".to_string()),
            content: "original content".to_string(),
            frontmatter: None,
            tags: vec![],
            outgoing_links: vec![],
            block_refs: vec![],
            linked_repo: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        let (_tmp, _index, mut writer, schema) = init_isolated_index();
        index_vault_note_core(&note, &mut writer, &schema).unwrap();

        let updated = VaultNote {
            id: "note-update".to_string(),
            path: "/tmp/update.md".to_string(),
            title: Some("Updated".to_string()),
            content: "updated content".to_string(),
            frontmatter: None,
            tags: vec!["new-tag".to_string()],
            outgoing_links: vec![],
            block_refs: vec![],
            linked_repo: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        index_vault_note_core(&updated, &mut writer, &schema).unwrap();

        // Tantivy delete + add semantics: old doc replaced, only 1 doc remains
        let reader = _index.reader().unwrap();
        let searcher = reader.searcher();
        let doc_type = schema.get_field("doc_type").unwrap();
        let term = tantivy::Term::from_field_text(doc_type, "vault");
        let count = searcher
            .search(
                &tantivy::query::TermQuery::new(term, tantivy::schema::IndexRecordOption::Basic),
                &tantivy::collector::Count,
            )
            .unwrap();
        assert_eq!(count, 1);
    }
}
