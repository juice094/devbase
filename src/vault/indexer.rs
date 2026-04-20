use crate::registry::WorkspaceRegistry;
use crate::search;
use tracing::info;

/// Index all vault notes from the registry into Tantivy.
///
/// This deletes the existing vault segment and re-adds all notes,
/// ensuring the search index stays in sync with the registry.
pub fn reindex_vault() -> anyhow::Result<()> {
    let conn = WorkspaceRegistry::init_db()?;
    let notes = WorkspaceRegistry::list_vault_notes(&conn)?;

    let (index, _reader) = search::init_index()?;
    let mut writer = search::get_writer(&index)?;
    let schema = index.schema();

    // Delete all existing vault docs
    let doc_type = schema.get_field("doc_type")?;
    let term = tantivy::Term::from_field_text(doc_type, "vault");
    writer.delete_term(term);

    for note in &notes {
        let title = note.title.as_deref().unwrap_or(&note.id);
        let tags: Vec<String> = note.tags.clone();
        if let Err(e) =
            search::add_vault_doc(&mut writer, &schema, &note.id, title, &note.content, &tags)
        {
            tracing::warn!("Failed to index vault note {}: {}", note.id, e);
        }
    }

    writer.commit()?;
    info!("Vault reindex complete: {} notes indexed", notes.len());
    Ok(())
}

/// Add or update a single vault note in the search index.
pub fn index_vault_note(note: &crate::registry::VaultNote) -> anyhow::Result<()> {
    let (index, _reader) = search::init_index()?;
    let mut writer = search::get_writer(&index)?;
    let schema = index.schema();

    // Delete old doc by id
    let id_field = schema.get_field("id")?;
    writer.delete_term(tantivy::Term::from_field_text(id_field, &note.id));

    let title = note.title.as_deref().unwrap_or(&note.id);
    search::add_vault_doc(&mut writer, &schema, &note.id, title, &note.content, &note.tags)?;
    writer.commit()?;
    Ok(())
}
