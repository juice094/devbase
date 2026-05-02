
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
             (repo_id, file_path, symbol_type, name, line_start, line_end, signature)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(repo_id, file_path, name) DO UPDATE SET
             symbol_type = excluded.symbol_type,
             line_start = excluded.line_start,
             line_end = excluded.line_end,
             signature = excluded.signature",
            (
                repo_id,
                sym.file_path.to_string_lossy().as_ref(),
                sym.symbol_type.as_str(),
                &sym.name,
                sym.line_start as i64,
                sym.line_end as i64,
                sym.signature.as_deref(),
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

