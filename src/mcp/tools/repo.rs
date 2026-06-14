// SPDX-License-Identifier: MIT
// Copyright (c) 2026 juice094

//! MCP tools for repository management.
//!
//! This module re-exports the legacy monolithic `repo.rs` split into domain
//! submodules: scan, health, sync, index, query_repos, natural-language query.

pub mod health;
pub mod index;
pub mod nl_query;
pub mod query_repos;
pub mod scan;
pub mod sync;

pub use health::DevkitHealthTool;
pub use index::DevkitIndexTool;
pub use nl_query::{DevkitNaturalLanguageQueryTool, nl_filter_repos_at};
pub use query_repos::DevkitQueryReposTool;
pub use scan::DevkitScanTool;
pub use sync::DevkitSyncTool;

/// Parse a GitHub URL into (owner, repo).
pub fn parse_github_repo(url: &str) -> Option<(String, String)> {
    let url = url.trim_end_matches(".git");
    if let Some(rest) = url.strip_prefix("https://github.com/") {
        let parts: Vec<&str> = rest.split('/').collect();
        if parts.len() >= 2 && !parts[0].is_empty() && !parts[1].is_empty() {
            return Some((parts[0].to_string(), parts[1].to_string()));
        }
    }
    if let Some(rest) = url.strip_prefix("http://github.com/") {
        let parts: Vec<&str> = rest.split('/').collect();
        if parts.len() >= 2 && !parts[0].is_empty() && !parts[1].is_empty() {
            return Some((parts[0].to_string(), parts[1].to_string()));
        }
    }
    if let Some(rest) = url.strip_prefix("git@github.com:") {
        let parts: Vec<&str> = rest.split('/').collect();
        if parts.len() >= 2 && !parts[0].is_empty() && !parts[1].is_empty() {
            return Some((parts[0].to_string(), parts[1].to_string()));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_github_repo_https() {
        assert_eq!(
            parse_github_repo("https://github.com/owner/repo"),
            Some(("owner".to_string(), "repo".to_string()))
        );
        assert_eq!(
            parse_github_repo("https://github.com/owner/repo.git"),
            Some(("owner".to_string(), "repo".to_string()))
        );
    }

    #[test]
    fn test_parse_github_repo_ssh() {
        assert_eq!(
            parse_github_repo("git@github.com:owner/repo"),
            Some(("owner".to_string(), "repo".to_string()))
        );
    }

    #[test]
    fn test_parse_github_repo_invalid() {
        assert_eq!(parse_github_repo("https://gitlab.com/owner/repo"), None);
        assert_eq!(parse_github_repo("not-a-url"), None);
    }
}
