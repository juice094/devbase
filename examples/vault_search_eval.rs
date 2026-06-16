// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (c) 2026 juice094
//!
//! Pilot evaluation of `devkit_vault_search` on the agri-paper dataset.
//!
//! Splits `C:\Users\22414\dev\agri-paper\paper\main.md` into vault notes,
//! indexes them with Tantivy, and compares three retrieval paths:
//!
//! 1. Tantivy BM25 (`search_vault_at`)
//! 2. Linear substring scan over note title/content/tags
//! 3. Filename/title keyword matching
//!
//! Metrics: Precision@k, Recall@k, MRR, latency.

use std::path::{Path, PathBuf};
use std::time::Instant;

use devbase::registry::WorkspaceRegistry;
use devbase::storage::StorageBackend;

const PAPER_PATH: &str = r"C:\Users\22414\dev\agri-paper\paper\main.md";

/// Isolated storage backend for the evaluation example.
struct EvalStorageBackend {
    dir: tempfile::TempDir,
}

impl EvalStorageBackend {
    fn new() -> anyhow::Result<Self> {
        Ok(Self { dir: tempfile::tempdir()? })
    }
}

impl StorageBackend for EvalStorageBackend {
    fn db_path(&self) -> anyhow::Result<PathBuf> {
        Ok(self.dir.path().join("registry.db"))
    }

    fn workspace_dir(&self) -> anyhow::Result<PathBuf> {
        let ws = self.dir.path().join("workspace");
        std::fs::create_dir_all(ws.join("vault"))?;
        std::fs::create_dir_all(ws.join("assets"))?;
        Ok(ws)
    }

    fn index_path(&self) -> anyhow::Result<PathBuf> {
        Ok(self.dir.path().join("search_index"))
    }

    fn symbol_index_path(&self) -> anyhow::Result<PathBuf> {
        Ok(self.dir.path().join("symbol_index"))
    }

    fn backup_dir(&self) -> anyhow::Result<PathBuf> {
        let backup = self.dir.path().join("backups");
        std::fs::create_dir_all(&backup)?;
        Ok(backup)
    }
}

#[derive(Debug, Clone)]
struct Note {
    id: String,
    title: String,
    content: String,
}

fn slugify(s: &str) -> String {
    s.to_lowercase()
        .replace(|c: char| !c.is_alphanumeric() && c != ' ', " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join("-")
}

/// Split a Markdown document into top-level sections by `# Heading`.
fn split_markdown(source: &str) -> Vec<(String, String)> {
    let mut sections = Vec::new();
    let mut current_title = String::from("frontmatter");
    let mut current_body = String::new();

    for line in source.lines() {
        if let Some(title) = line.strip_prefix("# ") {
            if !current_body.trim().is_empty() {
                sections.push((current_title.clone(), current_body.clone()));
            }
            current_title = title.split("{#").next().unwrap_or(title).trim().to_string();
            current_body = line.to_string();
        } else {
            current_body.push('\n');
            current_body.push_str(line);
        }
    }

    if !current_body.trim().is_empty() {
        sections.push((current_title, current_body));
    }

    sections
}

fn prepare_notes(vault_dir: &Path) -> anyhow::Result<Vec<Note>> {
    let source = std::fs::read_to_string(PAPER_PATH)?;
    let sections = split_markdown(&source);

    let mut notes = Vec::new();
    for (raw_title, body) in sections {
        let slug = slugify(&raw_title);
        let id = format!("{}.md", slug);
        let path = vault_dir.join(&id);

        let tags = extract_tags(&raw_title);
        let frontmatter =
            format!("---\ntitle: {}\ntags: [{}]\n---\n\n", raw_title, tags.join(", "));
        std::fs::write(&path, format!("{}{}", frontmatter, body))?;

        notes.push(Note {
            id,
            title: raw_title,
            content: body,
        });
    }

    Ok(notes)
}

fn extract_tags(title: &str) -> Vec<String> {
    let lower = title.to_lowercase();
    let mut tags = Vec::new();
    for (keyword, tag) in [
        ("introduction", "intro"),
        ("related work", "related-work"),
        ("theoretical", "theory"),
        ("algorithm", "algorithm"),
        ("experiments", "experiments"),
        ("results", "results"),
        ("discussion", "discussion"),
        ("conclusion", "conclusion"),
        ("rag", "rag"),
        ("retrieval", "retrieval"),
        ("cognitive load", "cognitive-load"),
    ] {
        if lower.contains(keyword) {
            tags.push(tag.to_string());
        }
    }
    tags
}

#[derive(Debug, Clone)]
struct Query {
    id: String,
    text: String,
    relevant: Vec<String>,
}

fn queries() -> Vec<Query> {
    vec![
        Query {
            id: "q1".into(),
            text: "retrieval depth capacity".into(),
            relevant: vec![
                "introduction.md".into(),
                "theoretical-framework.md".into(),
                "experimental-results.md".into(),
                "intermediate-k-probing-results.md".into(),
                "detailed-delta-k-breakdown.md".into(),
            ],
        },
        Query {
            id: "q2".into(),
            text: "ACR-Select".into(),
            relevant: vec![
                "acr-select-adaptive-context-retrieval.md".into(),
                "theoretical-framework.md".into(),
            ],
        },
        Query {
            id: "q3".into(),
            text: "cognitive load".into(),
            relevant: vec![
                "theoretical-framework.md".into(),
                "related-work.md".into(),
                "discussion.md".into(),
            ],
        },
        Query {
            id: "q4".into(),
            text: "agricultural benchmark".into(),
            relevant: vec![
                "experimental-setup.md".into(),
                "experimental-results.md".into(),
                "introduction.md".into(),
            ],
        },
        Query {
            id: "q5".into(),
            text: "Adaptive-RAG".into(),
            relevant: vec![
                "related-work.md".into(),
                "acr-select-adaptive-context-retrieval.md".into(),
            ],
        },
        Query {
            id: "q6".into(),
            text: "format collapse".into(),
            relevant: vec!["theoretical-framework.md".into(), "experimental-results.md".into()],
        },
        Query {
            id: "q7".into(),
            text: "marginal gain".into(),
            relevant: vec![
                "acr-select-adaptive-context-retrieval.md".into(),
                "theoretical-framework.md".into(),
            ],
        },
        Query {
            id: "q8".into(),
            text: "limitations".into(),
            relevant: vec!["discussion.md".into(), "conclusion.md".into()],
        },
    ]
}

fn rank_tantivy(
    index_path: &Path,
    query: &str,
    limit: usize,
) -> anyhow::Result<Vec<(String, f32)>> {
    Ok(devbase::search::search_vault_at(index_path, query, limit)?)
}

fn rank_linear_scan(notes: &[Note], query: &str, limit: usize) -> Vec<(String, f32)> {
    let keywords: Vec<String> = query.split_whitespace().map(|s| s.to_lowercase()).collect();

    let mut scored: Vec<(String, f32)> = notes
        .iter()
        .filter_map(|n| {
            let hay = format!(
                "{} {} {}",
                n.title.to_lowercase(),
                n.id.to_lowercase(),
                n.content.to_lowercase()
            );
            let matches = keywords.iter().filter(|kw| hay.contains(kw.as_str())).count();
            if matches == keywords.len() {
                Some((n.id.clone(), matches as f32))
            } else {
                None
            }
        })
        .collect();

    // Tie-break by id for stability.
    scored.sort_by(|a, b| {
        b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal).then(a.0.cmp(&b.0))
    });
    scored.truncate(limit);
    scored
}

fn rank_filename_match(notes: &[Note], query: &str, limit: usize) -> Vec<(String, f32)> {
    let keywords: Vec<String> = query.split_whitespace().map(|s| s.to_lowercase()).collect();

    let mut scored: Vec<(String, f32)> = notes
        .iter()
        .filter_map(|n| {
            let hay = format!("{} {}", n.title.to_lowercase(), n.id.to_lowercase());
            let matches = keywords.iter().filter(|kw| hay.contains(kw.as_str())).count();
            if matches > 0 {
                Some((n.id.clone(), matches as f32))
            } else {
                None
            }
        })
        .collect();

    scored.sort_by(|a, b| {
        b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal).then(a.0.cmp(&b.0))
    });
    scored.truncate(limit);
    scored
}

#[derive(Default, Debug)]
struct Metrics {
    precision_at_5: f64,
    precision_at_10: f64,
    recall_at_5: f64,
    recall_at_10: f64,
    mrr: f64,
    latency_ms: f64,
}

fn evaluate(
    name: &str,
    queries: &[Query],
    _note_ids: &[String],
    rank_fn: &mut dyn FnMut(&Query) -> anyhow::Result<Vec<String>>,
) -> Metrics {
    let mut total = Metrics::default();
    let count = queries.len() as f64;

    for q in queries {
        let start = Instant::now();
        let ranked = match rank_fn(q) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("[{}] query {} failed: {}", name, q.id, e);
                continue;
            }
        };
        total.latency_ms += start.elapsed().as_secs_f64() * 1000.0;

        let relevant: std::collections::HashSet<String> = q.relevant.iter().cloned().collect();

        for k in [5usize, 10usize] {
            let top_k: std::collections::HashSet<String> = ranked.iter().take(k).cloned().collect();
            let hits = relevant.intersection(&top_k).count();
            let p = hits as f64 / k as f64;
            let r = if relevant.is_empty() {
                0.0
            } else {
                hits as f64 / relevant.len() as f64
            };
            if k == 5 {
                total.precision_at_5 += p;
                total.recall_at_5 += r;
            } else {
                total.precision_at_10 += p;
                total.recall_at_10 += r;
            }
        }

        let rr = ranked
            .iter()
            .position(|id| relevant.contains(id))
            .map(|pos| 1.0 / (pos + 1) as f64)
            .unwrap_or(0.0);
        total.mrr += rr;
    }

    total.precision_at_5 /= count;
    total.precision_at_10 /= count;
    total.recall_at_5 /= count;
    total.recall_at_10 /= count;
    total.mrr /= count;
    total.latency_ms /= count;

    total
}

fn main() -> anyhow::Result<()> {
    println!("=== devbase vault search pilot evaluation ===");
    println!("dataset: {}", PAPER_PATH);

    let storage = EvalStorageBackend::new()?;
    let workspace_dir = storage.workspace_dir()?;
    let vault_dir = workspace_dir.join("vault");
    let index_path = storage.index_path()?;

    println!("\n[1/4] Preparing vault notes...");
    let notes = prepare_notes(&vault_dir)?;
    let note_ids: Vec<String> = notes.iter().map(|n| n.id.clone()).collect();
    println!("      {} notes written to {}", notes.len(), vault_dir.display());

    println!("\n[2/4] Initializing registry and scanning vault...");
    let mut conn = WorkspaceRegistry::init_db_with(&storage)?;
    devbase::vault::scanner::scan_vault(&mut conn, Some(&vault_dir))?;
    println!("      scan complete");

    println!("\n[3/4] Building Tantivy index...");
    devbase::search::index_vault_notes_at(&conn, &index_path)?;
    // Allow Windows to release handles.
    std::thread::sleep(std::time::Duration::from_millis(500));
    println!("      index built at {}", index_path.display());

    println!("\n[4/4] Evaluating retrieval methods...");
    let queries = queries();
    println!("      {} queries defined\n", queries.len());

    let tantivy = evaluate("Tantivy", &queries, &note_ids, &mut |q: &Query| {
        Ok(rank_tantivy(&index_path, &q.text, 10)?.into_iter().map(|(id, _)| id).collect())
    });

    let notes_for_linear = notes.clone();
    let linear = evaluate("Linear", &queries, &note_ids, &mut |q: &Query| {
        Ok(rank_linear_scan(&notes_for_linear, &q.text, 10)
            .into_iter()
            .map(|(id, _)| id)
            .collect())
    });

    let notes_for_filename = notes.clone();
    let filename = evaluate("Filename", &queries, &note_ids, &mut |q: &Query| {
        Ok(rank_filename_match(&notes_for_filename, &q.text, 10)
            .into_iter()
            .map(|(id, _)| id)
            .collect())
    });

    println!(
        "{:<12} {:>8} {:>8} {:>8} {:>8} {:>8} {:>10}",
        "Method", "P@5", "P@10", "R@5", "R@10", "MRR", "Latency(ms)"
    );
    println!("{}", "-".repeat(72));
    for (name, m) in [("Tantivy", tantivy), ("Linear", linear), ("Filename", filename)] {
        println!(
            "{:<12} {:>8.3} {:>8.3} {:>8.3} {:>8.3} {:>8.3} {:>10.3}",
            name,
            m.precision_at_5,
            m.precision_at_10,
            m.recall_at_5,
            m.recall_at_10,
            m.mrr,
            m.latency_ms
        );
    }

    Ok(())
}
