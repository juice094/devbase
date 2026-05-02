
// Re-exports to preserve main.rs compatibility after commands/simple.rs split
pub use crate::commands::analysis::{
    run_call_graph, run_code_symbols, run_dead_code, run_dependency_graph, run_metrics,
    run_module_graph,
};
pub use crate::commands::knowledge::{
    run_clean, run_digest, run_meta, run_oplog, run_skill_sync, run_tag, run_vault, run_watch,
};
pub use crate::commands::repo::{
    run_discover, run_health, run_index, run_query, run_registry, run_scan, run_sync,
    run_syncthing_push,
};
pub use crate::commands::system::{run_daemon, run_github_info, run_mcp, run_tui};

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
