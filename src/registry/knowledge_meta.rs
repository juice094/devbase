use chrono::{DateTime, Utc};

/// L4 metacognition layer entry — human corrections and cross-session consistency tracking.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct KnowledgeMeta {
    pub id: String,
    pub target_level: i32, // 1=L1 methods, 2=L2 philosophy, 3=L3 risks
    pub target_id: String, // foreign key into skills/known_limits/vault_notes
    pub correction_type: Option<String>, // 'human-feedback' | 'cross-session-drift' | 'formal-proof'
    pub correction_json: Option<String>,
    pub confidence: f64,
    pub created_at: DateTime<Utc>,
}

pub fn save_knowledge_meta(
    conn: &rusqlite::Connection,
    meta: &KnowledgeMeta,
) -> anyhow::Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO knowledge_meta
         (id, target_level, target_id, correction_type, correction_json, confidence, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        rusqlite::params![
            &meta.id,
            meta.target_level,
            &meta.target_id,
            meta.correction_type.as_ref(),
            meta.correction_json.as_ref(),
            meta.confidence,
            meta.created_at.to_rfc3339(),
        ],
    )?;
    let details = serde_json::json!({
        "action": "save",
        "meta_id": &meta.id,
        "target_level": meta.target_level,
        "target_id": &meta.target_id,
    });
    let _ = crate::registry::workspace::save_oplog(
        conn,
        &crate::registry::OplogEntry {
            id: None,
            event_type: crate::registry::OplogEventType::KnownLimit,
            repo_id: None,
            details: Some(details.to_string()),
            status: "success".to_string(),
            timestamp: Utc::now(),
            duration_ms: None,
            event_version: 1,
        },
    );
    Ok(())
}

pub fn get_knowledge_meta(
    conn: &rusqlite::Connection,
    id: &str,
) -> anyhow::Result<Option<KnowledgeMeta>> {
    let mut stmt = conn.prepare(
        "SELECT id, target_level, target_id, correction_type, correction_json, confidence, created_at
         FROM knowledge_meta WHERE id = ?1",
    )?;
    let row = stmt.query_row([id], |row| {
        Ok(KnowledgeMeta {
            id: row.get(0)?,
            target_level: row.get(1)?,
            target_id: row.get(2)?,
            correction_type: row.get(3)?,
            correction_json: row.get(4)?,
            confidence: row.get(5)?,
            created_at: row.get::<_, String>(6)?.parse().unwrap_or_else(|_| Utc::now()),
        })
    });
    match row {
        Ok(meta) => Ok(Some(meta)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

pub fn list_knowledge_meta(
    conn: &rusqlite::Connection,
    target_level: Option<i32>,
    target_id: Option<&str>,
) -> anyhow::Result<Vec<KnowledgeMeta>> {
    let mut sql = String::from(
        "SELECT id, target_level, target_id, correction_type, correction_json, confidence, created_at
         FROM knowledge_meta WHERE 1=1",
    );
    if target_level.is_some() {
        sql.push_str(" AND target_level = ?");
    }
    if target_id.is_some() {
        sql.push_str(" AND target_id = ?");
    }
    sql.push_str(" ORDER BY created_at DESC");

    let mut stmt = conn.prepare(&sql)?;
    let mut params: Vec<&dyn rusqlite::ToSql> = Vec::new();
    let level_owned = target_level;
    let id_owned = target_id.map(|s| s.to_string());
    if let Some(ref l) = level_owned {
        params.push(l);
    }
    if let Some(ref i) = id_owned {
        params.push(i);
    }

    let rows = stmt.query_map(rusqlite::params_from_iter(params), |row| {
        Ok(KnowledgeMeta {
            id: row.get(0)?,
            target_level: row.get(1)?,
            target_id: row.get(2)?,
            correction_type: row.get(3)?,
            correction_json: row.get(4)?,
            confidence: row.get(5)?,
            created_at: row.get::<_, String>(6)?.parse().unwrap_or_else(|_| Utc::now()),
        })
    })?;

    let mut results = Vec::new();
    for row in rows {
        results.push(row?);
    }
    Ok(results)
}

pub fn delete_knowledge_meta(conn: &rusqlite::Connection, id: &str) -> anyhow::Result<bool> {
    let rows = conn.execute("DELETE FROM knowledge_meta WHERE id = ?1", [id])?;
    let deleted = rows > 0;
    if deleted {
        let details = serde_json::json!({ "action": "delete", "meta_id": id });
        let _ = crate::registry::workspace::save_oplog(
            conn,
            &crate::registry::OplogEntry {
                id: None,
                event_type: crate::registry::OplogEventType::KnownLimit,
                repo_id: None,
                details: Some(details.to_string()),
                status: "success".to_string(),
                timestamp: Utc::now(),
                duration_ms: None,
                event_version: 1,
            },
        );
    }
    Ok(deleted)
}

impl super::WorkspaceRegistry {
    pub fn save_knowledge_meta(
        conn: &rusqlite::Connection,
        meta: &KnowledgeMeta,
    ) -> anyhow::Result<()> {
        save_knowledge_meta(conn, meta)
    }
    pub fn get_knowledge_meta(
        conn: &rusqlite::Connection,
        id: &str,
    ) -> anyhow::Result<Option<KnowledgeMeta>> {
        get_knowledge_meta(conn, id)
    }
    pub fn list_knowledge_meta(
        conn: &rusqlite::Connection,
        target_level: Option<i32>,
        target_id: Option<&str>,
    ) -> anyhow::Result<Vec<KnowledgeMeta>> {
        list_knowledge_meta(conn, target_level, target_id)
    }
    pub fn delete_knowledge_meta(conn: &rusqlite::Connection, id: &str) -> anyhow::Result<bool> {
        delete_knowledge_meta(conn, id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::WorkspaceRegistry;

    #[test]
    fn test_knowledge_meta_crud() {
        let conn = WorkspaceRegistry::init_in_memory().unwrap();
        let meta = KnowledgeMeta {
            id: "meta-1".to_string(),
            target_level: 3,
            target_id: "test-limit-1".to_string(),
            correction_type: Some("human-feedback".to_string()),
            correction_json: Some(r#"{"reason":"resolved by user"}"#.to_string()),
            confidence: 0.95,
            created_at: Utc::now(),
        };

        save_knowledge_meta(&conn, &meta).unwrap();
        let fetched = get_knowledge_meta(&conn, "meta-1").unwrap().unwrap();
        assert_eq!(fetched.id, "meta-1");
        assert_eq!(fetched.target_level, 3);
        assert_eq!(fetched.target_id, "test-limit-1");
        assert_eq!(fetched.confidence, 0.95);

        let list = list_knowledge_meta(&conn, Some(3), Some("test-limit-1")).unwrap();
        assert_eq!(list.len(), 1);

        let deleted = delete_knowledge_meta(&conn, "meta-1").unwrap();
        assert!(deleted);
        assert!(get_knowledge_meta(&conn, "meta-1").unwrap().is_none());
    }
}
