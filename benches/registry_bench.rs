use criterion::{Criterion, black_box, criterion_group, criterion_main};
use devbase::registry::repo;
use devbase::registry::{RepoEntry, WorkspaceRegistry};
use std::cell::Cell;
use std::path::PathBuf;

fn bench_format_mcp_message(c: &mut Criterion) {
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "result": {
            "content": [{"type": "text", "text": "benchmark payload"}],
            "isError": false
        }
    });
    c.bench_function("format_mcp_message", |b| {
        b.iter(|| {
            let msg = devbase::mcp::format_mcp_message(&body);
            black_box(msg);
        });
    });
}

fn sample_repo(id: &str) -> RepoEntry {
    RepoEntry {
        id: id.to_string(),
        local_path: PathBuf::from(format!("/tmp/{}", id)),
        tags: vec![],
        language: Some("rust".to_string()),
        discovered_at: chrono::Utc::now(),
        workspace_type: "git".to_string(),
        data_tier: "private".to_string(),
        last_synced_at: None,
        stars: None,
        remotes: vec![],
    }
}

fn bench_save_repo(c: &mut Criterion) {
    let tmp = tempfile::tempdir().unwrap();
    // SAFETY: Benchmark-only env mutation. Single-process scope, no concurrent access.
    unsafe {
        std::env::set_var("DEVBASE_DATA_DIR", tmp.path());
    }
    let mut conn = WorkspaceRegistry::init_db().unwrap();

    let counter = Cell::new(0usize);
    c.bench_function("save_repo", |b| {
        b.iter(|| {
            let i = counter.get();
            counter.set(i + 1);
            let repo = sample_repo(&format!("bench-repo-{}", i));
            repo::save_repo(&mut conn, &repo).unwrap();
        });
    });
}

fn bench_list_repos(c: &mut Criterion) {
    let tmp = tempfile::tempdir().unwrap();
    // SAFETY: Benchmark-only env mutation. Single-process scope, no concurrent access.
    unsafe {
        std::env::set_var("DEVBASE_DATA_DIR", tmp.path());
    }
    let mut conn = WorkspaceRegistry::init_db().unwrap();

    for i in 0..500 {
        let repo = sample_repo(&format!("prefill-{}", i));
        repo::save_repo(&mut conn, &repo).unwrap();
    }

    c.bench_function("list_repos_500", |b| {
        b.iter(|| {
            let repos = repo::list_repos(&conn).unwrap();
            black_box(repos);
        });
    });
}

fn bench_get_health(c: &mut Criterion) {
    let tmp = tempfile::tempdir().unwrap();
    // SAFETY: Benchmark-only env mutation. Single-process scope, no concurrent access.
    unsafe {
        std::env::set_var("DEVBASE_DATA_DIR", tmp.path());
    }
    let mut conn = WorkspaceRegistry::init_db().unwrap();

    let repo = sample_repo("health-bench");
    repo::save_repo(&mut conn, &repo).unwrap();

    let health = devbase::registry::HealthEntry {
        status: "ok".to_string(),
        ahead: 0,
        behind: 0,
        checked_at: chrono::Utc::now(),
    };
    devbase::registry::health::save_health(&conn, "health-bench", &health).unwrap();

    c.bench_function("get_health", |b| {
        b.iter(|| {
            let h = devbase::registry::health::get_health(&conn, "health-bench").unwrap();
            black_box(h);
        });
    });
}

criterion_group!(
    benches,
    bench_save_repo,
    bench_list_repos,
    bench_get_health,
    bench_format_mcp_message
);
criterion_main!(benches);
