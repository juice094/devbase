// SPDX-License-Identifier: MIT
// Copyright (c) 2026 juice094
use rusqlite::Connection;

pub fn run(conn: &Connection) -> anyhow::Result<()> {
    conn.execute("PRAGMA user_version = 1", [])?;
    Ok(())
}
