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
    crate::registry::health::save_stars_cache(&conn, "test-repo", 42).unwrap();

    // Read back
    let (stars, fetched_at) = crate::registry::health::get_stars_cache(&conn, "test-repo")
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

    let result = crate::registry::health::get_stars_cache(&conn, "nonexistent").unwrap();
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

    crate::registry::health::save_stars_cache(&conn, "repo-a", 10).unwrap();
    let (stars1, at1) = crate::registry::health::get_stars_cache(&conn, "repo-a").unwrap().unwrap();
    assert_eq!(stars1, 10);

    // Small sleep to ensure timestamp changes
    std::thread::sleep(std::time::Duration::from_millis(50));

    crate::registry::health::save_stars_cache(&conn, "repo-a", 20).unwrap();
    let (stars2, at2) = crate::registry::health::get_stars_cache(&conn, "repo-a").unwrap().unwrap();
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
    crate::registry::workspace::save_oplog(&conn, &entry).unwrap();
    let list = crate::registry::workspace::list_oplog(&conn, 10).unwrap();
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
    let list = crate::registry::workspace::list_oplog(&conn, 10).unwrap();
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
    assert_eq!(OplogEventType::KnownLimit.as_str(), "known_limit");

    assert_eq!("scan".parse::<OplogEventType>().unwrap(), OplogEventType::Scan);
    assert_eq!("sync".parse::<OplogEventType>().unwrap(), OplogEventType::Sync);
    assert_eq!("index".parse::<OplogEventType>().unwrap(), OplogEventType::Index);
    assert_eq!("health_check".parse::<OplogEventType>().unwrap(), OplogEventType::HealthCheck);
    assert_eq!("health".parse::<OplogEventType>().unwrap(), OplogEventType::HealthCheck); // backward compat
    assert!("unknown".parse::<OplogEventType>().is_err());
}

// ---------------------------------------------------------------------------
// Dead-code detection SQL logic tests
// ---------------------------------------------------------------------------

#[test]
fn test_dead_code_excludes_pub_variants_and_main() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.execute(
        "CREATE TABLE code_symbols (
            repo_id TEXT NOT NULL,
            file_path TEXT NOT NULL,
            symbol_type TEXT NOT NULL,
            name TEXT NOT NULL,
            line_start INTEGER,
            line_end INTEGER,
            signature TEXT,
            PRIMARY KEY (repo_id, file_path, name)
        )",
        [],
    )
    .unwrap();
    conn.execute(
        "CREATE TABLE code_call_graph (
            repo_id TEXT NOT NULL,
            caller_file TEXT NOT NULL,
            caller_symbol TEXT NOT NULL,
            caller_line INTEGER,
            callee_name TEXT NOT NULL
        )",
        [],
    )
    .unwrap();

    let repo = "test-repo";
    let symbols = [
        ("private_fn", "fn private_fn() {}", true), // should be dead
        ("pub_fn", "pub fn pub_fn() {}", false),    // pub — excluded
        ("pub_async_fn", "pub async fn pub_async_fn() {}", false), // pub async — excluded
        ("pub_crate_fn", "pub(crate) fn pub_crate_fn() {}", false), // pub(crate) — excluded
        ("pub_unsafe_fn", "pub unsafe fn pub_unsafe_fn() {}", false), // pub unsafe — excluded
        ("main", "fn main() {}", false),            // main — excluded
        ("called_fn", "fn called_fn() {}", false),  // has incoming call
    ];

    for (name, sig, _) in &symbols {
        conn.execute(
            "INSERT INTO code_symbols (repo_id, file_path, symbol_type, name, line_start, signature)
             VALUES (?1, 'src/lib.rs', 'function', ?2, 1, ?3)",
            rusqlite::params![repo, name, sig],
        )
        .unwrap();
    }

    // Add a call edge to called_fn
    conn.execute(
        "INSERT INTO code_call_graph (repo_id, caller_file, caller_symbol, caller_line, callee_name)
         VALUES (?1, 'src/lib.rs', 'other', 10, 'called_fn')",
        [repo],
    )
    .unwrap();

    // Run the same SQL that devkit_dead_code uses (include_pub = false)
    let sql = "SELECT cs.name FROM code_symbols cs \
         WHERE cs.repo_id = ?1 AND cs.symbol_type = 'function' \
         AND NOT EXISTS ( \
             SELECT 1 FROM code_call_graph ccg \
             WHERE ccg.repo_id = cs.repo_id AND ccg.callee_name = cs.name \
         ) \
         AND (cs.signature IS NULL OR cs.signature NOT LIKE 'pub%fn%') \
         AND cs.name != 'main' \
         ORDER BY cs.name"
        .to_string();

    let mut stmt = conn.prepare(&sql).unwrap();
    let rows = stmt
        .query_map([repo], |row| {
            let name: String = row.get(0)?;
            Ok(name)
        })
        .unwrap();

    let dead: Vec<String> = rows.filter_map(Result::ok).collect();
    assert_eq!(dead, vec!["private_fn"]);
}

#[test]
fn test_dead_code_include_pub() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.execute(
        "CREATE TABLE code_symbols (
            repo_id TEXT NOT NULL,
            file_path TEXT NOT NULL,
            symbol_type TEXT NOT NULL,
            name TEXT NOT NULL,
            line_start INTEGER,
            line_end INTEGER,
            signature TEXT,
            PRIMARY KEY (repo_id, file_path, name)
        )",
        [],
    )
    .unwrap();
    conn.execute(
        "CREATE TABLE code_call_graph (repo_id TEXT, caller_file TEXT, caller_symbol TEXT, caller_line INTEGER, callee_name TEXT)",
        [],
    )
    .unwrap();

    let repo = "test-repo";
    conn.execute(
        "INSERT INTO code_symbols (repo_id, file_path, symbol_type, name, line_start, signature)
         VALUES (?1, 'src/lib.rs', 'function', 'pub_fn', 1, 'pub fn pub_fn() {}')",
        [repo],
    )
    .unwrap();

    // include_pub = true → skip the pub filter, but still exclude main
    let sql = "SELECT cs.name FROM code_symbols cs \
         WHERE cs.repo_id = ?1 AND cs.symbol_type = 'function' \
         AND NOT EXISTS ( \
             SELECT 1 FROM code_call_graph ccg \
             WHERE ccg.repo_id = cs.repo_id AND ccg.callee_name = cs.name \
         ) \
         AND cs.name != 'main' \
         ORDER BY cs.name"
        .to_string();

    let mut stmt = conn.prepare(&sql).unwrap();
    let rows = stmt
        .query_map([repo], |row| {
            let name: String = row.get(0)?;
            Ok(name)
        })
        .unwrap();

    let dead: Vec<String> = rows.filter_map(Result::ok).collect();
    assert_eq!(dead, vec!["pub_fn"]);
}

#[test]
fn test_primary_remote_prefers_origin() {
    let entry = RepoEntry {
        id: "test".to_string(),
        local_path: std::path::PathBuf::from("/tmp/test"),
        tags: vec![],
        discovered_at: Utc::now(),
        language: None,
        workspace_type: "git".to_string(),
        data_tier: "private".to_string(),
        last_synced_at: None,
        stars: None,
        remotes: vec![
            RemoteEntry {
                remote_name: "upstream".to_string(),
                upstream_url: Some("https://example.com/upstream".to_string()),
                default_branch: Some("main".to_string()),
                last_sync: None,
            },
            RemoteEntry {
                remote_name: "origin".to_string(),
                upstream_url: Some("https://example.com/origin".to_string()),
                default_branch: Some("main".to_string()),
                last_sync: None,
            },
        ],
    };
    let remote = entry.primary_remote().unwrap();
    assert_eq!(remote.remote_name, "origin");
}

#[test]
fn test_primary_remote_fallback_to_first() {
    let entry = RepoEntry {
        id: "test".to_string(),
        local_path: std::path::PathBuf::from("/tmp/test"),
        tags: vec![],
        discovered_at: Utc::now(),
        language: None,
        workspace_type: "git".to_string(),
        data_tier: "private".to_string(),
        last_synced_at: None,
        stars: None,
        remotes: vec![RemoteEntry {
            remote_name: "upstream".to_string(),
            upstream_url: Some("https://example.com/upstream".to_string()),
            default_branch: Some("main".to_string()),
            last_sync: None,
        }],
    };
    let remote = entry.primary_remote().unwrap();
    assert_eq!(remote.remote_name, "upstream");
}

#[test]
fn test_primary_remote_none() {
    let entry = RepoEntry {
        id: "test".to_string(),
        local_path: std::path::PathBuf::from("/tmp/test"),
        tags: vec![],
        discovered_at: Utc::now(),
        language: None,
        workspace_type: "git".to_string(),
        data_tier: "private".to_string(),
        last_synced_at: None,
        stars: None,
        remotes: vec![],
    };
    assert!(entry.primary_remote().is_none());
}
