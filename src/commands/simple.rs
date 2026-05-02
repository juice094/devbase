use anyhow::Context;
use devbase::*;
use devbase::mcp::clients::RegistryClient;
use rusqlite::OptionalExtension;
use tracing::{info, warn};

fn parse_github_repo(url: &str) -> Option<(String, String)> {
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
    use storage::{AppContext, StorageBackend};
    use std::path::PathBuf;
    use std::sync::Arc;

    struct TempStorage {
        dir: tempfile::TempDir,
    }

    impl TempStorage {
        fn new() -> Self {
            Self {
                dir: tempfile::tempdir().unwrap(),
            }
        }
    }

    impl StorageBackend for TempStorage {
        fn db_path(&self) -> anyhow::Result<PathBuf> {
            Ok(self.dir.path().join("db"))
        }
        fn workspace_dir(&self) -> anyhow::Result<PathBuf> {
            let ws = self.dir.path().join("ws");
            std::fs::create_dir_all(&ws)?;
            Ok(ws)
        }
        fn index_path(&self) -> anyhow::Result<PathBuf> {
            Ok(self.dir.path().join("idx"))
        }
        fn backup_dir(&self) -> anyhow::Result<PathBuf> {
            Ok(self.dir.path().join("bk"))
        }
    }

    #[tokio::test]
    async fn test_run_vault_list_empty() {
        let storage = Arc::new(TempStorage::new());
        let mut ctx = AppContext::with_storage(storage).unwrap();
        let result = run_vault(&mut ctx, crate::VaultCommands::List { tag: None }).await;
        assert!(result.is_ok());
    }
}
