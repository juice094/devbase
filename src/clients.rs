// SPDX-License-Identifier: MIT
// Copyright (c) 2026 juice094
//! MCP tool client traits — decouple tools from concrete devbase modules.
//!
//! **日期**: 2026-05-01 (Batch 3 — MCP trait 化启动)
//! **目标**: 将 `mcp/tools/repo.rs` 中的 `crate::scan::run_json` 等直接调用
//!        改为通过 trait 调用，使 `repo.rs` 的 `crate::` 引用数从 70 → <50。
//!
//! 设计原则:
//! - 每个 trait 对应一个业务领域（scan/health/sync/registry）。
//! - `AppContext` 在 `storage.rs` 中统一实现所有 trait，作为"中央调度器"。
//! - MCP tools 只依赖 trait，不依赖具体模块。

use anyhow::Result;
use serde_json::Value;

/// Scan a directory to discover Git repositories and non-Git workspaces.
pub trait ScanClient: Send + Sync {
    fn scan_directory(
        &self,
        path: &str,
        register: bool,
    ) -> impl std::future::Future<Output = Result<Value>> + Send;
}

/// Check health status of registered repositories.
pub trait HealthClient: Send + Sync {
    fn check_health(&self, detail: bool) -> impl std::future::Future<Output = Result<Value>>;
}

/// Sync registered repositories.
pub trait SyncClient: Send + Sync {
    fn sync_repos(
        &self,
        dry_run: bool,
        filter_tags: Option<Vec<String>>,
    ) -> impl std::future::Future<Output = Result<Value>>;
}

/// Registry CRUD operations exposed to MCP tools.
pub trait RegistryClient: Send + Sync {
    fn list_repos(&self, filter: Option<&str>) -> Result<Value>;
    fn get_repo(&self, repo_id: &str) -> Result<Value>;
    fn list_modules(&self, repo_id: &str) -> Result<Value>;
    fn save_paper(&self, paper: &Value) -> Result<Value>;
    fn save_experiment(&self, exp: &Value) -> Result<Value>;
    fn list_code_metrics(&self) -> Result<Value>;
    fn get_code_metrics(&self, repo_id: &str) -> Result<Value>;
    fn get_health(&self, repo_id: &str) -> Result<Value>;

    fn query_call_graph(
        &self,
        repo_id: &str,
        callee: Option<&str>,
        caller: Option<&str>,
        file: Option<&str>,
        limit: usize,
    ) -> Result<Value>;

    fn query_dependencies(
        &self,
        repo_id: &str,
        direction: &str,
        relation_type: Option<&str>,
    ) -> Result<Value>;

    fn query_code_symbols(
        &self,
        repo_id: &str,
        name: Option<&str>,
        symbol_type: Option<&str>,
        file: Option<&str>,
        limit: usize,
    ) -> Result<Value>;

    fn query_dead_code(&self, repo_id: &str, include_pub: bool, limit: usize) -> Result<Value>;

    fn save_relation(
        &self,
        from: &str,
        to: &str,
        relation_type: &str,
        confidence: f64,
    ) -> Result<Value>;

    fn query_relations(
        &self,
        entity_id: &str,
        direction: &str,
        relation_type: Option<&str>,
    ) -> Result<Value>;

    fn delete_relations(&self, from: &str, to: &str, relation_type: Option<&str>) -> Result<Value>;

    fn list_vault_notes(&self) -> Result<Value>;
}

/// Knowledge engine operations.
pub trait KnowledgeClient: Send + Sync {
    fn run_index(&self, path: &str) -> Result<Value>;
    fn save_note(&self, repo_id: &str, text: &str, author: &str) -> Result<Value>;
    fn save_summary(&self, repo_id: &str, desc: &str, author: &str) -> Result<Value>;
    fn get_paper(&self, arxiv_id: &str) -> Result<Value>;
}

/// Digest generation.
pub trait DigestClient: Send + Sync {
    fn generate_daily_digest(&self) -> Result<Value>;
}

/// Low-level repository analysis (no async, no external state).
pub trait RepoAnalyzer: Send + Sync {
    fn compute_workspace_hash(&self, path: &str) -> Result<String>;
    fn analyze_repo(
        &self,
        path: &str,
        upstream_url: Option<&str>,
        default_branch: Option<&str>,
    ) -> Result<(String, usize, usize)>;
}

/// Tantivy search operations exposed to MCP tools.
pub trait SearchClient: Send + Sync {
    fn index_is_empty_at(&self, path: &std::path::Path) -> Result<bool>;
    fn search_repos_at(
        &self,
        path: &std::path::Path,
        query: &str,
        limit: usize,
    ) -> Result<Vec<(String, f32)>>;
}

/// Workflow management exposed to MCP tools.
pub trait WorkflowClient: Send + Sync {
    fn list_workflows(&self) -> Result<Value>;
    fn get_workflow(&self, workflow_id: &str) -> Result<Value>;
    fn run_workflow(&self, workflow_id: &str, inputs: Value) -> Result<Value>;
    fn get_execution(&self, exec_id: i64) -> Result<Value>;
}

/// Vault (Markdown knowledge-base) operations exposed to MCP tools.
pub trait VaultClient: Send + Sync {
    fn list_vault_notes(&self) -> Result<Value>;
    fn read_vault_note(&self, path: &str) -> Result<Value>;
    fn get_backlinks(&self, note_id: &str) -> Result<Value>;
    fn build_vault_graph(
        &self,
        repo_id: Option<&str>,
        note_id: Option<&str>,
        depth: usize,
    ) -> Result<Value>;
    fn get_vault_history(&self, note_id: &str) -> Result<Value>;
    fn export_vault(&self, output_dir: &str) -> Result<Value>;
}

/// Agent memory intelligence operations (v37+ knowledge graph).
pub trait MemoryClient: Send + Sync {
    /// Link two memories with a typed relationship.
    fn link_memories(
        &self,
        from_id: i64,
        to_id: i64,
        relation_type: &str,
        confidence: f64,
        evidence: Option<&str>,
    ) -> Result<Value>;

    /// Query memory relations (outgoing/incoming/both).
    fn query_memory_relations(
        &self,
        memory_id: i64,
        direction: &str,
        relation_type: Option<&str>,
        limit: usize,
    ) -> Result<Value>;

    /// Build and return a memory sub-graph via BFS.
    fn build_memory_graph(&self, root_memory_id: i64, depth: u32) -> Result<Value>;

    /// Detect duplicate memories within a context by vector similarity.
    fn dedup_memories(&self, context_id: &str, threshold: f32) -> Result<Value>;

    /// Merge two similar memories.
    fn merge_memories(&self, primary_id: i64, secondary_id: i64, strategy: &str) -> Result<Value>;

    /// Apply decay policy to a context's memories.
    fn apply_memory_decay(&self, context_id: &str) -> Result<Value>;

    /// Return memory statistics for a context.
    fn memory_stats(&self, context_id: &str) -> Result<Value>;
}
