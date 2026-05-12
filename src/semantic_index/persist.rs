// SPDX-License-Identifier: MIT
// Copyright (c) 2026 juice094
use super::{CodeCall, CodeSymbol};

/// Batch save symbols to the SQLite registry.
pub fn save_symbols(
    conn: &mut rusqlite::Connection,
    repo_id: &str,
    symbols: &[CodeSymbol],
) -> anyhow::Result<usize> {
    let tx = conn.transaction()?;

    // Clear old symbols for this repo
    tx.execute("DELETE FROM code_symbols WHERE repo_id = ?1", [repo_id])?;

    let mut inserted = 0;
    for sym in symbols {
        tx.execute(
            "INSERT INTO code_symbols
             (repo_id, file_path, symbol_type, name, line_start, line_end, signature, attributes)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(repo_id, file_path, name) DO UPDATE SET
             symbol_type = excluded.symbol_type,
             line_start = excluded.line_start,
             line_end = excluded.line_end,
             signature = excluded.signature,
             attributes = excluded.attributes",
            (
                repo_id,
                sym.file_path.to_string_lossy().as_ref(),
                sym.symbol_type.as_str(),
                &sym.name,
                sym.line_start as i64,
                sym.line_end as i64,
                sym.signature.as_deref(),
                sym.attributes.as_deref(),
            ),
        )?;
        inserted += 1;
    }

    tx.commit()?;
    Ok(inserted)
}

/// Delete symbols and calls for specific files (used in incremental indexing).
pub fn delete_symbols_for_files(
    conn: &mut rusqlite::Connection,
    repo_id: &str,
    files: &[String],
) -> anyhow::Result<usize> {
    let tx = conn.transaction()?;
    let mut deleted = 0;
    for file in files {
        deleted += tx.execute(
            "DELETE FROM code_symbols WHERE repo_id = ?1 AND file_path = ?2",
            [repo_id, file],
        )?;
        tx.execute(
            "DELETE FROM code_call_graph WHERE repo_id = ?1 AND caller_file = ?2",
            [repo_id, file],
        )?;
    }
    tx.commit()?;
    Ok(deleted)
}

/// Incremental save symbols without clearing the repo first.
pub fn save_symbols_incremental(
    conn: &mut rusqlite::Connection,
    repo_id: &str,
    symbols: &[CodeSymbol],
) -> anyhow::Result<usize> {
    let tx = conn.transaction()?;
    let mut inserted = 0;
    for sym in symbols {
        tx.execute(
            "INSERT INTO code_symbols
             (repo_id, file_path, symbol_type, name, line_start, line_end, signature, attributes)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(repo_id, file_path, name) DO UPDATE SET
             symbol_type = excluded.symbol_type,
             line_start = excluded.line_start,
             line_end = excluded.line_end,
             signature = excluded.signature,
             attributes = excluded.attributes",
            (
                repo_id,
                sym.file_path.to_string_lossy().as_ref(),
                sym.symbol_type.as_str(),
                &sym.name,
                sym.line_start as i64,
                sym.line_end as i64,
                sym.signature.as_deref(),
                sym.attributes.as_deref(),
            ),
        )?;
        inserted += 1;
    }
    tx.commit()?;
    Ok(inserted)
}

/// Batch save call relationships to the SQLite registry.
pub fn save_calls(
    conn: &mut rusqlite::Connection,
    repo_id: &str,
    calls: &[CodeCall],
) -> anyhow::Result<usize> {
    let tx = conn.transaction()?;

    // Clear old calls for this repo
    tx.execute("DELETE FROM code_call_graph WHERE repo_id = ?1", [repo_id])?;

    let mut inserted = 0;
    for call in calls {
        tx.execute(
            "INSERT INTO code_call_graph
             (repo_id, caller_file, caller_symbol, caller_line, callee_name)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT DO NOTHING",
            (
                repo_id,
                call.caller_file.to_string_lossy().as_ref(),
                &call.caller_symbol,
                call.caller_line as i64,
                &call.callee_name,
            ),
        )?;
        inserted += 1;
    }

    tx.commit()?;
    Ok(inserted)
}

/// Incremental save calls without clearing the repo first.
pub fn save_calls_incremental(
    conn: &mut rusqlite::Connection,
    repo_id: &str,
    calls: &[CodeCall],
) -> anyhow::Result<usize> {
    let tx = conn.transaction()?;
    let mut inserted = 0;
    for call in calls {
        tx.execute(
            "INSERT INTO code_call_graph
             (repo_id, caller_file, caller_symbol, caller_line, callee_name)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT DO NOTHING",
            (
                repo_id,
                call.caller_file.to_string_lossy().as_ref(),
                &call.caller_symbol,
                call.caller_line as i64,
                &call.callee_name,
            ),
        )?;
        inserted += 1;
    }
    tx.commit()?;
    Ok(inserted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::test_helpers::WorkspaceRegistry;
    use crate::semantic_index::SymbolType;
    use std::path::PathBuf;

    fn sample_symbol(name: &str, file: &str) -> CodeSymbol {
        CodeSymbol {
            symbol_type: SymbolType::Function,
            name: name.to_string(),
            file_path: PathBuf::from(file),
            line_start: 1,
            line_end: 10,
            signature: Some(format!("fn {}()", name)),
            attributes: None,
        }
    }

    fn sample_call(caller_file: &str, caller_symbol: &str, callee: &str) -> CodeCall {
        CodeCall {
            caller_file: PathBuf::from(caller_file),
            caller_symbol: caller_symbol.to_string(),
            caller_line: 5,
            callee_name: callee.to_string(),
        }
    }

    #[test]
    fn test_save_symbols_replaces_old() {
        let mut conn = WorkspaceRegistry::init_in_memory().unwrap();
        let old = vec![sample_symbol("old_fn", "src/old.rs")];
        let new = vec![sample_symbol("new_fn", "src/new.rs")];

        save_symbols(&mut conn, "repo1", &old).unwrap();
        save_symbols(&mut conn, "repo1", &new).unwrap();

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM code_symbols WHERE repo_id = ?1", ["repo1"], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_save_symbols_incremental() {
        let mut conn = WorkspaceRegistry::init_in_memory().unwrap();
        let first = vec![sample_symbol("fn_a", "src/a.rs")];
        let second = vec![sample_symbol("fn_b", "src/b.rs")];

        save_symbols_incremental(&mut conn, "repo1", &first).unwrap();
        save_symbols_incremental(&mut conn, "repo1", &second).unwrap();

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM code_symbols WHERE repo_id = ?1", ["repo1"], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(count, 2);
    }

    #[test]
    fn test_delete_symbols_for_files() {
        let mut conn = WorkspaceRegistry::init_in_memory().unwrap();
        let symbols = vec![sample_symbol("fn_a", "src/a.rs"), sample_symbol("fn_b", "src/b.rs")];
        let calls = vec![
            sample_call("src/a.rs", "fn_a", "helper"),
            sample_call("src/b.rs", "fn_b", "helper"),
        ];

        save_symbols(&mut conn, "repo1", &symbols).unwrap();
        save_calls(&mut conn, "repo1", &calls).unwrap();

        delete_symbols_for_files(&mut conn, "repo1", &["src/a.rs".to_string()]).unwrap();

        let sym_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM code_symbols WHERE repo_id = ?1", ["repo1"], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(sym_count, 1);

        let call_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM code_call_graph WHERE repo_id = ?1",
                ["repo1"],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(call_count, 1);
    }

    #[test]
    fn test_save_calls_replaces_old() {
        let mut conn = WorkspaceRegistry::init_in_memory().unwrap();
        let old = vec![sample_call("src/old.rs", "old_fn", "callee1")];
        let new = vec![sample_call("src/new.rs", "new_fn", "callee2")];

        save_calls(&mut conn, "repo1", &old).unwrap();
        save_calls(&mut conn, "repo1", &new).unwrap();

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM code_call_graph WHERE repo_id = ?1",
                ["repo1"],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_save_calls_incremental() {
        let mut conn = WorkspaceRegistry::init_in_memory().unwrap();
        let first = vec![sample_call("src/a.rs", "fn_a", "callee1")];
        let second = vec![sample_call("src/b.rs", "fn_b", "callee2")];

        save_calls_incremental(&mut conn, "repo1", &first).unwrap();
        save_calls_incremental(&mut conn, "repo1", &second).unwrap();

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM code_call_graph WHERE repo_id = ?1",
                ["repo1"],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 2);
    }
}
