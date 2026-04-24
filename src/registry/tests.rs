use super::*;
use chrono::Utc;

#[test]
fn test_stars_cache_roundtrip() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.execute(
        "CREATE TABLE repo_stars_cache (
            repo_id TEXT PRIMARY KEY,
            stars INTEGER,
            fetched_at TEXT
        )",
        [],
    )
    .unwrap();
    conn.execute(
        "CREATE TABLE repo_stars_history (
            repo_id TEXT,
            stars INTEGER,
            fetched_at TEXT
        )",
        [],
    )
    .unwrap();

    // Save
    WorkspaceRegistry::save_stars_cache(&conn, "test-repo", 42).unwrap();

    // Read back
    let (stars, fetched_at) = WorkspaceRegistry::get_stars_cache(&conn, "test-repo")
        .unwrap()
        .expect("cache entry should exist");
    assert_eq!(stars, 42);
    let elapsed = Utc::now().signed_duration_since(fetched_at).num_seconds();
    assert!((0..5).contains(&elapsed), "fetched_at should be very recent");
}

#[test]
fn test_stars_cache_miss() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.execute(
        "CREATE TABLE repo_stars_cache (
            repo_id TEXT PRIMARY KEY,
            stars INTEGER,
            fetched_at TEXT
        )",
        [],
    )
    .unwrap();

    let result = WorkspaceRegistry::get_stars_cache(&conn, "nonexistent").unwrap();
    assert!(result.is_none());
}

#[test]
fn test_stars_cache_update() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.execute(
        "CREATE TABLE repo_stars_cache (
            repo_id TEXT PRIMARY KEY,
            stars INTEGER,
            fetched_at TEXT
        )",
        [],
    )
    .unwrap();
    conn.execute(
        "CREATE TABLE repo_stars_history (
            repo_id TEXT,
            stars INTEGER,
            fetched_at TEXT
        )",
        [],
    )
    .unwrap();

    WorkspaceRegistry::save_stars_cache(&conn, "repo-a", 10).unwrap();
    let (stars1, at1) = WorkspaceRegistry::get_stars_cache(&conn, "repo-a").unwrap().unwrap();
    assert_eq!(stars1, 10);

    // Small sleep to ensure timestamp changes
    std::thread::sleep(std::time::Duration::from_millis(50));

    WorkspaceRegistry::save_stars_cache(&conn, "repo-a", 20).unwrap();
    let (stars2, at2) = WorkspaceRegistry::get_stars_cache(&conn, "repo-a").unwrap().unwrap();
    assert_eq!(stars2, 20);
    assert!(at2 > at1, "updated timestamp should be newer");
}

#[test]
fn test_oplog_save_and_list() {
    let conn = WorkspaceRegistry::init_in_memory().unwrap();
    let entry = OplogEntry {
        id: None,
        event_type: OplogEventType::Sync,
        repo_id: Some("repo-a".to_string()),
        details: Some(r#"{"dry_run":true,"repo_count":3}"#.to_string()),
        status: "success".to_string(),
        timestamp: Utc::now(),
        duration_ms: Some(42),
        event_version: 1,
    };
    WorkspaceRegistry::save_oplog(&conn, &entry).unwrap();
    let list = WorkspaceRegistry::list_oplog(&conn, 10).unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].event_type, OplogEventType::Sync);
    assert_eq!(list[0].repo_id, Some("repo-a".to_string()));
    assert_eq!(list[0].duration_ms, Some(42));
    assert_eq!(list[0].event_version, 1);
}

#[test]
fn test_oplog_migration_compat() {
    // Simulate a migrated schema with a legacy-format row (event_version = 0)
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.execute(
        "CREATE TABLE oplog (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            operation TEXT NOT NULL,
            repo_id TEXT,
            details TEXT,
            status TEXT NOT NULL,
            timestamp TEXT NOT NULL,
            event_type TEXT,
            duration_ms INTEGER,
            event_version INTEGER DEFAULT 1
        )",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO oplog (operation, event_type, repo_id, details, status, timestamp, duration_ms, event_version)
         VALUES ('health', 'health_check', NULL, 'legacy details', 'success', '2024-01-01T00:00:00Z', NULL, 0)",
        [],
    )
    .unwrap();
    let list = WorkspaceRegistry::list_oplog(&conn, 10).unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].event_type, OplogEventType::HealthCheck);
    assert_eq!(list[0].details, Some("legacy details".to_string()));
    assert_eq!(list[0].event_version, 0);
    assert_eq!(list[0].duration_ms, None);
}

#[test]
fn test_oplog_event_type_roundtrip() {
    assert_eq!(OplogEventType::Scan.as_str(), "scan");
    assert_eq!(OplogEventType::Sync.as_str(), "sync");
    assert_eq!(OplogEventType::Index.as_str(), "index");
    assert_eq!(OplogEventType::HealthCheck.as_str(), "health_check");

    assert_eq!("scan".parse::<OplogEventType>().unwrap(), OplogEventType::Scan);
    assert_eq!("sync".parse::<OplogEventType>().unwrap(), OplogEventType::Sync);
    assert_eq!("index".parse::<OplogEventType>().unwrap(), OplogEventType::Index);
    assert_eq!("health_check".parse::<OplogEventType>().unwrap(), OplogEventType::HealthCheck);
    assert_eq!("health".parse::<OplogEventType>().unwrap(), OplogEventType::HealthCheck); // backward compat
    assert!("unknown".parse::<OplogEventType>().is_err());
}
