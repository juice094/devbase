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
    fn check_health(
        &self,
        detail: bool,
    ) -> impl std::future::Future<Output = Result<Value>> + Send;
}

/// Sync registered repositories.
pub trait SyncClient: Send + Sync {
    fn sync_repos(
        &self,
        dry_run: bool,
        filter_tags: Option<Vec<String>>,
    ) -> impl std::future::Future<Output = Result<Value>> + Send;
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
