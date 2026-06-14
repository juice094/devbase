use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use devbase::registry::VaultNote;
use devbase::search::{commit_writer, get_writer, init_index_at, search_vault_at};
use devbase::vault::indexer::reindex_vault_core;

fn generate_notes(root: &std::path::Path, count: usize) -> Vec<VaultNote> {
    let mut notes = Vec::with_capacity(count);
    for i in 0..count {
        let id = format!("note-{}.md", i);
        let path = root.join(&id);
        let title = format!("Vault Note {}", i);
        let tags = if i % 3 == 0 {
            vec!["rust".to_string(), "architecture".to_string()]
        } else if i % 3 == 1 {
            vec!["design".to_string()]
        } else {
            vec!["cli".to_string(), "rust".to_string()]
        };
        let body_tag = if i % 2 == 0 { "rust" } else { "architecture" };
        let content = format!(
            "---\ntitle: {}\ntags: [{}]\n---\n\n# {}\n\nThis is content for note {}. It discusses {} patterns and design decisions.\n",
            title,
            tags.join(", "),
            title,
            i,
            body_tag
        );
        std::fs::write(&path, content).unwrap();
        notes.push(VaultNote {
            id,
            path: path.to_string_lossy().to_string(),
            title: Some(title),
            content: String::new(),
            frontmatter: None,
            tags,
            outgoing_links: vec![],
            block_refs: vec![],
            linked_repo: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        });
    }
    notes
}

fn bench_reindex_vault_core(c: &mut Criterion) {
    let mut group = c.benchmark_group("reindex_vault");
    group.sample_size(50);

    for count in [50usize, 200usize] {
        let notes_tmp = tempfile::tempdir().unwrap();
        let notes = generate_notes(notes_tmp.path(), count);

        let index_tmp = tempfile::tempdir().unwrap();
        let (index, _reader) = init_index_at(index_tmp.path()).unwrap();
        let mut writer = get_writer(&index).unwrap();
        let schema = index.schema();

        group.bench_with_input(BenchmarkId::from_parameter(count), &notes, |b, notes| {
            b.iter(|| {
                reindex_vault_core(notes, &mut writer, &schema).unwrap();
                commit_writer(&mut writer).unwrap();
                black_box(&writer);
            });
        });
    }

    group.finish();
}

fn bench_search_vault_at(c: &mut Criterion) {
    let mut group = c.benchmark_group("search_vault");
    group.sample_size(50);

    for count in [50usize, 200usize] {
        let notes_tmp = tempfile::tempdir().unwrap();
        let notes = generate_notes(notes_tmp.path(), count);

        let index_tmp = tempfile::tempdir().unwrap();
        let index_path = index_tmp.path().to_path_buf();
        let (index, _reader) = init_index_at(&index_path).unwrap();
        let mut writer = get_writer(&index).unwrap();
        let schema = index.schema();
        reindex_vault_core(&notes, &mut writer, &schema).unwrap();
        commit_writer(&mut writer).unwrap();
        drop(writer);
        drop(index);

        for (label, query) in [("single", "rust"), ("multi", "rust design")] {
            group.bench_with_input(
                BenchmarkId::new(format!("{}_docs", count), label),
                &(index_path.clone(), query),
                |b, (path, q)| {
                    b.iter(|| {
                        let results = search_vault_at(path, q, 10).unwrap();
                        black_box(results);
                    });
                },
            );
        }
    }

    group.finish();
}

criterion_group!(benches, bench_reindex_vault_core, bench_search_vault_at);
criterion_main!(benches);
