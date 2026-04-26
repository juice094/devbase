use super::WorkspaceRegistry;
use chrono::{DateTime, Utc};

/// A single known limit (L3 risk layer entry).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct KnownLimit {
    pub id: String,
    pub category: String,
    pub description: String,
    pub source: Option<String>,
    pub severity: Option<i32>,
    pub first_seen_at: DateTime<Utc>,
    pub last_checked_at: Option<DateTime<Utc>>,
    pub mitigated: bool,
}

impl WorkspaceRegistry {
    pub fn save_known_limit(conn: &rusqlite::Connection, limit: &KnownLimit) -> anyhow::Result<()> {
        let is_update: bool = conn
            .query_row("SELECT 1 FROM known_limits WHERE id = ?1", [&limit.id], |_| Ok(true))
            .unwrap_or(false);
        conn.execute(
            "INSERT OR REPLACE INTO known_limits
             (id, category, description, source, severity, first_seen_at, last_checked_at, mitigated)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            rusqlite::params![
                &limit.id,
                &limit.category,
                &limit.description,
                limit.source.as_ref(),
                limit.severity,
                limit.first_seen_at.to_rfc3339(),
                limit.last_checked_at.map(|d| d.to_rfc3339()),
                if limit.mitigated { 1 } else { 0 },
            ],
        )?;
        let action = if is_update { "updated" } else { "created" };
        let details = serde_json::json!({
            "action": action,
            "limit_id": &limit.id,
            "category": &limit.category,
        });
        let _ = super::WorkspaceRegistry::save_oplog(
            conn,
            &super::OplogEntry {
                id: None,
                event_type: super::OplogEventType::KnownLimit,
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

    pub fn get_known_limit(
        conn: &rusqlite::Connection,
        id: &str,
    ) -> anyhow::Result<Option<KnownLimit>> {
        let mut stmt = conn.prepare(
            "SELECT id, category, description, source, severity,
                    first_seen_at, last_checked_at, mitigated
             FROM known_limits WHERE id = ?1",
        )?;
        let row = stmt.query_row([id], |row| {
            Ok(KnownLimit {
                id: row.get(0)?,
                category: row.get(1)?,
                description: row.get(2)?,
                source: row.get(3)?,
                severity: row.get(4)?,
                first_seen_at: row.get::<_, String>(5)?.parse().unwrap_or_else(|_| Utc::now()),
                last_checked_at: row.get::<_, Option<String>>(6)?.and_then(|s| s.parse().ok()),
                mitigated: row.get::<_, i32>(7)? != 0,
            })
        });
        match row {
            Ok(limit) => Ok(Some(limit)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn list_known_limits(
        conn: &rusqlite::Connection,
        category: Option<&str>,
        mitigated: Option<bool>,
    ) -> anyhow::Result<Vec<KnownLimit>> {
        let mut sql = String::from(
            "SELECT id, category, description, source, severity,
                    first_seen_at, last_checked_at, mitigated
             FROM known_limits WHERE 1=1",
        );
        if category.is_some() {
            sql.push_str(" AND category = ?");
        }
        if mitigated.is_some() {
            sql.push_str(" AND mitigated = ?");
        }
        sql.push_str(" ORDER BY first_seen_at DESC");

        let mut stmt = conn.prepare(&sql)?;
        let mut params: Vec<&dyn rusqlite::ToSql> = Vec::new();
        let cat_owned: Option<String> = category.map(|s| s.to_string());
        let mitigated_val: Option<i32> = mitigated.map(|m| if m { 1 } else { 0 });
        if let Some(ref c) = cat_owned {
            params.push(c);
        }
        if let Some(ref m) = mitigated_val {
            params.push(m);
        }

        let rows = stmt.query_map(rusqlite::params_from_iter(params), |row| {
            Ok(KnownLimit {
                id: row.get(0)?,
                category: row.get(1)?,
                description: row.get(2)?,
                source: row.get(3)?,
                severity: row.get(4)?,
                first_seen_at: row.get::<_, String>(5)?.parse().unwrap_or_else(|_| Utc::now()),
                last_checked_at: row.get::<_, Option<String>>(6)?.and_then(|s| s.parse().ok()),
                mitigated: row.get::<_, i32>(7)? != 0,
            })
        })?;

        let mut limits = Vec::new();
        for row in rows {
            limits.push(row?);
        }
        Ok(limits)
    }

    pub fn delete_known_limit(conn: &rusqlite::Connection, id: &str) -> anyhow::Result<bool> {
        let rows = conn.execute("DELETE FROM known_limits WHERE id = ?1", [id])?;
        let deleted = rows > 0;
        if deleted {
            let details = serde_json::json!({ "action": "deleted", "limit_id": id });
            let _ = super::WorkspaceRegistry::save_oplog(
                conn,
                &super::OplogEntry {
                    id: None,
                    event_type: super::OplogEventType::KnownLimit,
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

    pub fn resolve_known_limit(conn: &rusqlite::Connection, id: &str) -> anyhow::Result<bool> {
        let rows = conn.execute(
            "UPDATE known_limits SET mitigated = 1, last_checked_at = ?1 WHERE id = ?2",
            [Utc::now().to_rfc3339(), id.to_string()],
        )?;
        let resolved = rows > 0;
        if resolved {
            let details = serde_json::json!({ "action": "resolved", "limit_id": id });
            let _ = super::WorkspaceRegistry::save_oplog(
                conn,
                &super::OplogEntry {
                    id: None,
                    event_type: super::OplogEventType::KnownLimit,
                    repo_id: None,
                    details: Some(details.to_string()),
                    status: "success".to_string(),
                    timestamp: Utc::now(),
                    duration_ms: None,
                    event_version: 1,
                },
            );
        }
        Ok(resolved)
    }

    /// Seed known_limits with hard vetoes from AGENTS.md.
    pub fn seed_hard_vetoes(conn: &rusqlite::Connection) -> anyhow::Result<usize> {
        let vetoes = vec![
            ("hard-veto", "禁止闭源 / 云端强制 / 数据外泄", Some("AGENTS.md")),
            ("hard-veto", "禁止 Docker / RAG(Qdrant) / GUI(Electron)", Some("AGENTS.md")),
            ("hard-veto", "禁止项目广度 > 5 核心工具", Some("AGENTS.md")),
            ("hard-veto", "本地 LLM 优先", Some("AGENTS.md")),
            ("hard-veto", "Rust 核心模块不可外包给子 Agent", Some("AGENTS.md")),
        ];
        let mut inserted = 0;
        for (category, description, source) in vetoes {
            let id = format!("{}-{}", category, blake3::hash(description.as_bytes()).to_hex());
            let exists: bool = conn
                .query_row("SELECT 1 FROM known_limits WHERE id = ?1", [&id], |_| Ok(true))
                .unwrap_or(false);
            if !exists {
                let limit = KnownLimit {
                    id: id.clone(),
                    category: category.to_string(),
                    description: description.to_string(),
                    source: source.map(|s| s.to_string()),
                    severity: Some(5),
                    first_seen_at: Utc::now(),
                    last_checked_at: None,
                    mitigated: false,
                };
                Self::save_known_limit(conn, &limit)?;
                inserted += 1;
            }
        }
        if inserted > 0 {
            let details = serde_json::json!({ "action": "seed", "count": inserted });
            let _ = super::WorkspaceRegistry::save_oplog(
                conn,
                &super::OplogEntry {
                    id: None,
                    event_type: super::OplogEventType::KnownLimit,
                    repo_id: None,
                    details: Some(details.to_string()),
                    status: "success".to_string(),
                    timestamp: Utc::now(),
                    duration_ms: None,
                    event_version: 1,
                },
            );
        }
        Ok(inserted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::WorkspaceRegistry;

    #[test]
    fn test_known_limit_crud() {
        let conn = WorkspaceRegistry::init_in_memory().unwrap();
        let limit = KnownLimit {
            id: "test-limit-1".to_string(),
            category: "known-bug".to_string(),
            description: "Test description".to_string(),
            source: Some("test".to_string()),
            severity: Some(3),
            first_seen_at: Utc::now(),
            last_checked_at: None,
            mitigated: false,
        };

        WorkspaceRegistry::save_known_limit(&conn, &limit).unwrap();
        let fetched = WorkspaceRegistry::get_known_limit(&conn, "test-limit-1").unwrap().unwrap();
        assert_eq!(fetched.id, "test-limit-1");
        assert_eq!(fetched.category, "known-bug");
        assert!(!fetched.mitigated);

        let resolved = WorkspaceRegistry::resolve_known_limit(&conn, "test-limit-1").unwrap();
        assert!(resolved);
        let resolved_fetched =
            WorkspaceRegistry::get_known_limit(&conn, "test-limit-1").unwrap().unwrap();
        assert!(resolved_fetched.mitigated);

        let deleted = WorkspaceRegistry::delete_known_limit(&conn, "test-limit-1").unwrap();
        assert!(deleted);
        assert!(WorkspaceRegistry::get_known_limit(&conn, "test-limit-1").unwrap().is_none());

        // Verify oplog entries
        let oplog = WorkspaceRegistry::list_oplog(&conn, 10).unwrap();
        let limit_logs: Vec<_> = oplog.into_iter().filter(|e| matches!(e.event_type, super::super::OplogEventType::KnownLimit)).collect();
        assert_eq!(limit_logs.len(), 3, "expected create + resolve + delete oplog entries");
        assert!(limit_logs.iter().any(|e| e.details.as_ref().unwrap().contains("created")));
        assert!(limit_logs.iter().any(|e| e.details.as_ref().unwrap().contains("resolved")));
        assert!(limit_logs.iter().any(|e| e.details.as_ref().unwrap().contains("deleted")));
    }

    #[test]
    fn test_list_known_limits_by_category() {
        let conn = WorkspaceRegistry::init_in_memory().unwrap();
        WorkspaceRegistry::seed_hard_vetoes(&conn).unwrap();
        let hard_vetoes =
            WorkspaceRegistry::list_known_limits(&conn, Some("hard-veto"), Some(false)).unwrap();
        assert!(!hard_vetoes.is_empty());
        for v in &hard_vetoes {
            assert_eq!(v.category, "hard-veto");
            assert!(!v.mitigated);
        }
    }
}
