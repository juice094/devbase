// SPDX-License-Identifier: MIT
// Copyright (c) 2026 juice094
//! Agent Context registry: CRUD for agent_contexts and agent_memories tables.
//!
//! Provides persistent AI session contexts (Claude Projects inspired) with
//! associated typed memories. All operations are transactional where needed.

use chrono::{DateTime, Utc};
use rusqlite::{Connection, OptionalExtension};

/// A persisted AI agent context (session / project scope).
#[derive(Debug, Clone, PartialEq)]
pub struct AgentContext {
    pub id: String,
    pub name: String,
    pub intent: Option<String>,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// A typed memory entry attached to an AgentContext.
#[derive(Debug, Clone, PartialEq)]
pub struct AgentMemory {
    pub id: i64,
    pub context_id: String,
    pub memory_type: String,
    pub content: String,
    pub created_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Context CRUD
// ---------------------------------------------------------------------------

/// Insert or replace an agent context.
pub fn upsert_context(
    conn: &mut Connection,
    id: &str,
    name: &str,
    intent: Option<&str>,
) -> anyhow::Result<()> {
    let tx = conn.transaction()?;
    let now = Utc::now().to_rfc3339();
    tx.execute(
        "INSERT INTO agent_contexts (id, name, intent, status, created_at, updated_at)
         VALUES (?1, ?2, ?3, 'active', ?4, ?4)
         ON CONFLICT(id) DO UPDATE SET
             name = excluded.name,
             intent = excluded.intent,
             status = 'active',
             updated_at = excluded.updated_at",
        rusqlite::params![id, name, intent, now],
    )?;
    tx.commit()?;
    Ok(())
}

/// List all contexts ordered by most recently updated.
pub fn list_contexts(conn: &Connection) -> anyhow::Result<Vec<AgentContext>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, intent, status, created_at, updated_at
         FROM agent_contexts
         ORDER BY updated_at DESC",
    )?;
    let rows = stmt.query_map([], |row| {
        let created_at = parse_datetime(row.get(4)?).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(
                4,
                rusqlite::types::Type::Text,
                Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string())),
            )
        })?;
        let updated_at = parse_datetime(row.get(5)?).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(
                5,
                rusqlite::types::Type::Text,
                Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string())),
            )
        })?;
        Ok(AgentContext {
            id: row.get(0)?,
            name: row.get(1)?,
            intent: row.get(2)?,
            status: row.get(3)?,
            created_at,
            updated_at,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

/// Get a single context by ID.
pub fn get_context(conn: &Connection, id: &str) -> anyhow::Result<Option<AgentContext>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, intent, status, created_at, updated_at
         FROM agent_contexts
         WHERE id = ?1",
    )?;
    stmt.query_row([id], |row| {
        let created_at = parse_datetime(row.get(4)?).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(
                4,
                rusqlite::types::Type::Text,
                Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string())),
            )
        })?;
        let updated_at = parse_datetime(row.get(5)?).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(
                5,
                rusqlite::types::Type::Text,
                Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string())),
            )
        })?;
        Ok(AgentContext {
            id: row.get(0)?,
            name: row.get(1)?,
            intent: row.get(2)?,
            status: row.get(3)?,
            created_at,
            updated_at,
        })
    })
    .optional()
    .map_err(Into::into)
}

/// Get a context together with all its memories.
pub fn get_context_with_memories(
    conn: &Connection,
    id: &str,
) -> anyhow::Result<Option<(AgentContext, Vec<AgentMemory>)>> {
    let ctx = match get_context(conn, id)? {
        Some(c) => c,
        None => return Ok(None),
    };
    let memories = list_memories(conn, id)?;
    Ok(Some((ctx, memories)))
}

/// Archive a context (soft-delete via status change).
pub fn archive_context(conn: &mut Connection, id: &str) -> anyhow::Result<bool> {
    let tx = conn.transaction()?;
    let now = Utc::now().to_rfc3339();
    let rows = tx.execute(
        "UPDATE agent_contexts SET status = 'archived', updated_at = ?1 WHERE id = ?2",
        rusqlite::params![now, id],
    )?;
    tx.commit()?;
    Ok(rows > 0)
}

/// Hard-delete a context and cascade-delete its memories.
pub fn delete_context(conn: &mut Connection, id: &str) -> anyhow::Result<bool> {
    let tx = conn.transaction()?;
    let rows = tx.execute("DELETE FROM agent_contexts WHERE id = ?1", [id])?;
    tx.commit()?;
    Ok(rows > 0)
}

// ---------------------------------------------------------------------------
// Memory CRUD
// ---------------------------------------------------------------------------

/// Insert a memory and return its auto-generated row id.
pub fn insert_memory(
    conn: &mut Connection,
    context_id: &str,
    memory_type: &str,
    content: &str,
) -> anyhow::Result<i64> {
    let tx = conn.transaction()?;
    tx.execute(
        "INSERT INTO agent_memories (context_id, memory_type, content, created_at)
         VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![context_id, memory_type, content, Utc::now().to_rfc3339()],
    )?;
    let id = tx.last_insert_rowid();
    tx.commit()?;
    Ok(id)
}

/// List memories for a context, newest first.
pub fn list_memories(conn: &Connection, context_id: &str) -> anyhow::Result<Vec<AgentMemory>> {
    let mut stmt = conn.prepare(
        "SELECT id, context_id, memory_type, content, created_at
         FROM agent_memories
         WHERE context_id = ?1
         ORDER BY created_at DESC",
    )?;
    let rows = stmt.query_map([context_id], |row| {
        let created_at = parse_datetime(row.get(4)?).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(
                4,
                rusqlite::types::Type::Text,
                Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string())),
            )
        })?;
        Ok(AgentMemory {
            id: row.get(0)?,
            context_id: row.get(1)?,
            memory_type: row.get(2)?,
            content: row.get(3)?,
            created_at,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn parse_datetime(s: String) -> anyhow::Result<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(&s)
        .map(|dt| dt.with_timezone(&Utc))
        .or_else(|_| {
            chrono::NaiveDateTime::parse_from_str(&s, "%Y-%m-%d %H:%M:%S")
                .map(|ndt| DateTime::from_naive_utc_and_offset(ndt, Utc))
        })
        .map_err(|e| anyhow::anyhow!("Invalid datetime '{}': {}", s, e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::test_helpers::WorkspaceRegistry;

    #[test]
    fn test_context_crud() {
        let mut conn = WorkspaceRegistry::init_in_memory().unwrap();

        // Create
        upsert_context(&mut conn, "ctx-1", "Project Alpha", Some("Rewrite auth layer")).unwrap();
        let ctx = get_context(&conn, "ctx-1").unwrap().expect("context exists");
        assert_eq!(ctx.name, "Project Alpha");
        assert_eq!(ctx.intent.as_deref(), Some("Rewrite auth layer"));
        assert_eq!(ctx.status, "active");

        // Update
        upsert_context(&mut conn, "ctx-1", "Project Alpha+", Some("Rewrite auth + RBAC")).unwrap();
        let ctx2 = get_context(&conn, "ctx-1").unwrap().expect("context still exists");
        assert_eq!(ctx2.name, "Project Alpha+");

        // List
        let list = list_contexts(&conn).unwrap();
        assert_eq!(list.len(), 1);

        // Archive
        assert!(archive_context(&mut conn, "ctx-1").unwrap());
        let archived = get_context(&conn, "ctx-1").unwrap().expect("context not deleted");
        assert_eq!(archived.status, "archived");

        // Delete
        assert!(delete_context(&mut conn, "ctx-1").unwrap());
        assert!(get_context(&conn, "ctx-1").unwrap().is_none());
    }

    #[test]
    fn test_memory_crud() {
        let mut conn = WorkspaceRegistry::init_in_memory().unwrap();
        upsert_context(&mut conn, "ctx-mem", "Test", None).unwrap();

        let id1 = insert_memory(&mut conn, "ctx-mem", "decision", "Use SQLite").unwrap();
        let id2 = insert_memory(&mut conn, "ctx-mem", "constraint", "Must be <50ms").unwrap();
        assert!(id1 > 0);
        assert!(id2 > 0);

        let memories = list_memories(&conn, "ctx-mem").unwrap();
        assert_eq!(memories.len(), 2);
        // Newest first
        assert_eq!(memories[0].memory_type, "constraint");
        assert_eq!(memories[1].memory_type, "decision");
    }

    #[test]
    fn test_get_context_with_memories() {
        let mut conn = WorkspaceRegistry::init_in_memory().unwrap();
        upsert_context(&mut conn, "ctx-full", "Full", Some("intent")).unwrap();
        insert_memory(&mut conn, "ctx-full", "note", "content").unwrap();

        let result = get_context_with_memories(&conn, "ctx-full").unwrap();
        assert!(result.is_some());
        let (ctx, mems) = result.unwrap();
        assert_eq!(ctx.name, "Full");
        assert_eq!(mems.len(), 1);
        assert_eq!(mems[0].content, "content");
    }

    #[test]
    fn test_missing_context() {
        let conn = WorkspaceRegistry::init_in_memory().unwrap();
        assert!(get_context(&conn, "nope").unwrap().is_none());
        assert!(get_context_with_memories(&conn, "nope").unwrap().is_none());
    }

    #[test]
    fn test_cascade_delete() {
        let mut conn = WorkspaceRegistry::init_in_memory().unwrap();
        upsert_context(&mut conn, "ctx-cascade", "Cascade", None).unwrap();
        insert_memory(&mut conn, "ctx-cascade", "t", "data").unwrap();

        delete_context(&mut conn, "ctx-cascade").unwrap();
        let mems = list_memories(&conn, "ctx-cascade").unwrap();
        assert!(mems.is_empty());
    }
}
