// SPDX-License-Identifier: MIT
// Copyright (c) 2026 juice094
//! Knowledge engine: repository indexing, summary extraction, and module analysis.
//!
//! Orchestrates Tantivy full-text indexing, SQLite registry persistence,
//! semantic code indexing (AST + call graph), and optional embedding generation.
//!
//! Entry points:
//! - [`run_index`] — batch index all registered repos or a single path
//! - [`index_repo`] — index a single repo (standalone writer)
//! - [`index_repo_with_writer`] — index a single repo reusing an existing writer

pub mod fallback;
pub mod index;
pub mod index_state;
pub mod llm;
pub mod module;
pub mod readme;

pub use fallback::*;
pub use index::*;
pub use llm::*;
pub use module::*;
pub use readme::*;

#[derive(Debug, Clone)]
pub struct ModuleInfo {
    pub name: String,
    pub kind: String, // "lib", "bin", "test", "example", "unknown"
}

/// Run an async future from a synchronous context safely.
///
/// If already inside a tokio runtime (e.g. `spawn_blocking`), spawns the
/// future onto that runtime and blocks the current thread on a std channel.
/// If outside any runtime, creates a temporary runtime.
pub(crate) fn block_on_async<T>(
    future: impl std::future::Future<Output = T> + Send + 'static,
) -> Option<T>
where
    T: Send + 'static,
{
    match tokio::runtime::Handle::try_current() {
        Ok(handle) => {
            let (tx, rx) = std::sync::mpsc::channel();
            handle.spawn(async move {
                let _ = tx.send(future.await);
            });
            rx.recv().ok()
        }
        Err(_) => {
            let rt = tokio::runtime::Runtime::new().ok()?;
            Some(rt.block_on(future))
        }
    }
}

impl crate::clients::KnowledgeClient for crate::storage::AppContext {
    fn run_index(&self, path: &str) -> anyhow::Result<serde_json::Value> {
        let mut conn = self.conn()?;
        let count = crate::knowledge_engine::run_index(&mut conn, path, false)?;
        Ok(serde_json::json!({ "success": true, "indexed": count, "errors": 0 }))
    }

    fn save_note(
        &self,
        repo_id: &str,
        text: &str,
        author: &str,
    ) -> anyhow::Result<serde_json::Value> {
        let conn = self.conn()?;
        crate::registry::knowledge::save_note(&conn, repo_id, text, author)?;
        Ok(serde_json::json!({ "success": true }))
    }

    fn save_summary(
        &self,
        repo_id: &str,
        desc: &str,
        author: &str,
    ) -> anyhow::Result<serde_json::Value> {
        let conn = self.conn()?;
        crate::registry::knowledge::save_summary(&conn, repo_id, desc, author)?;
        Ok(serde_json::json!({ "success": true }))
    }

    fn get_paper(&self, arxiv_id: &str) -> anyhow::Result<serde_json::Value> {
        let conn = self.conn()?;
        let papers = crate::registry::knowledge::list_papers(&conn)?;
        match papers.into_iter().find(|p| p.id == arxiv_id) {
            Some(p) => Ok(serde_json::json!({
                "success": true,
                "id": p.id,
                "title": p.title,
                "venue": p.venue,
                "year": p.year,
                "pdf_path": p.pdf_path,
                "tags": p.tags,
            })),
            None => Ok(serde_json::json!({ "success": false, "error": "Paper not found" })),
        }
    }
}
