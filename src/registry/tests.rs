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
