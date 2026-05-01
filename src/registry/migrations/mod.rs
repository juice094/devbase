use rusqlite::Connection;

pub mod v01_initial;
pub mod v02_add_columns;
pub mod v03_workspace_snapshots;
pub mod v04_oplog;
pub mod v05_placeholder;
pub mod v06_drop_unused;
pub mod v07_vault_notes;
pub mod v08_drop_content;
pub mod v09_code_symbols;
pub mod v10_call_graph;
pub mod v11_embeddings;
pub mod v12_oplog_enrich;
pub mod v13_symbol_links;
pub mod v14_skills;
pub mod v15_skill_deps;
pub mod v16_entities;
pub mod v17_workflows;
pub mod v18_known_limits;
pub mod v19_knowledge_meta;
pub mod v20_flat_ids;
pub mod v21_drop_repos;
pub mod v22_drop_more;
pub mod v23_cleanup;
pub mod v24_relations;
pub mod v25_agent_reads;
pub mod v26_denormalize;

pub fn run_all(conn: &mut Connection) -> anyhow::Result<()> {
    let user_version: i32 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;

    if user_version < 1 {
        v01_initial::run(conn)?;
    }
    if user_version < 2 {
        v02_add_columns::run(conn)?;
    }
    if user_version < 3 {
        v03_workspace_snapshots::run(conn)?;
    }
    if user_version < 4 {
        v04_oplog::run(conn)?;
    }
    if user_version < 5 {
        v05_placeholder::run(conn)?;
    }
    if user_version < 6 {
        v06_drop_unused::run(conn)?;
    }
    if user_version < 7 {
        v07_vault_notes::run(conn)?;
    }
    if user_version < 8 {
        v08_drop_content::run(conn)?;
    }
    if user_version < 9 {
        v09_code_symbols::run(conn)?;
    }
    if user_version < 10 {
        v10_call_graph::run(conn)?;
    }
    if user_version < 11 {
        v11_embeddings::run(conn)?;
    }
    if user_version < 12 {
        v12_oplog_enrich::run(conn)?;
    }
    if user_version < 13 {
        v13_symbol_links::run(conn)?;
    }
    if user_version < 14 {
        v14_skills::run(conn)?;
    }
    if user_version < 15 {
        v15_skill_deps::run(conn)?;
    }
    if user_version < 16 {
        v16_entities::run(conn)?;
    }
    if user_version < 17 {
        v17_workflows::run(conn)?;
    }
    if user_version < 18 {
        v18_known_limits::run(conn)?;
    }
    if user_version < 19 {
        v19_knowledge_meta::run(conn)?;
    }
    if user_version < 20 {
        v20_flat_ids::run(conn)?;
    }
    if user_version < 21 {
        v21_drop_repos::run(conn)?;
    }
    if user_version < 22 {
        v22_drop_more::run(conn)?;
    }
    if user_version < 23 {
        v23_cleanup::run(conn)?;
    }
    if user_version < 24 {
        v24_relations::run(conn)?;
    }
    if user_version < 25 {
        v25_agent_reads::run(conn)?;
    }
    if user_version < 26 {
        v26_denormalize::run(conn)?;
    }

    Ok(())
}
