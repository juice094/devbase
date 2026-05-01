use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use std::path::PathBuf;
use std::sync::Arc;

/// 抽象数据存储后端，解耦具体路径实现。
///
/// 默认实现保留现有行为（`dirs::data_local_dir()` + `DEVBASE_DATA_DIR` 覆盖）。
/// 未来可通过此 trait 实现内存后端、测试隔离后端或远程后端。
pub trait StorageBackend: Send + Sync {
    /// SQLite registry 数据库的完整路径。
    fn db_path(&self) -> anyhow::Result<PathBuf>;

    /// Workspace 根目录（含 vault/、assets/ 子目录）。
    fn workspace_dir(&self) -> anyhow::Result<PathBuf>;

    /// Tantivy 搜索索引目录。
    fn index_path(&self) -> anyhow::Result<PathBuf>;

    /// 自动备份目录。
    fn backup_dir(&self) -> anyhow::Result<PathBuf>;
}

/// 默认存储后端：使用本地数据目录。
///
/// 环境变量 `DEVBASE_DATA_DIR` 可覆盖默认路径，用于测试和隔离场景。
pub struct DefaultStorageBackend;

impl DefaultStorageBackend {
    fn data_base(&self) -> anyhow::Result<PathBuf> {
        let dir = if let Some(d) = std::env::var_os("DEVBASE_DATA_DIR") {
            PathBuf::from(d)
        } else {
            dirs::data_local_dir()
                .ok_or_else(|| anyhow::anyhow!("Could not determine local data directory"))?
        };
        Ok(dir.join("devbase"))
    }
}

impl StorageBackend for DefaultStorageBackend {
    fn db_path(&self) -> anyhow::Result<PathBuf> {
        let dir = self.data_base()?;
        std::fs::create_dir_all(&dir)?;
        Ok(dir.join("registry.db"))
    }

    fn workspace_dir(&self) -> anyhow::Result<PathBuf> {
        let dir = self.data_base()?;
        let ws = dir.join("workspace");
        std::fs::create_dir_all(&ws)?;
        std::fs::create_dir_all(ws.join("vault"))?;
        std::fs::create_dir_all(ws.join("assets"))?;
        Ok(ws)
    }

    fn index_path(&self) -> anyhow::Result<PathBuf> {
        let dir = self.data_base()?;
        std::fs::create_dir_all(&dir)?;
        Ok(dir.join("search_index"))
    }

    fn backup_dir(&self) -> anyhow::Result<PathBuf> {
        let dir = self.data_base()?;
        let backup = dir.join("backups");
        std::fs::create_dir_all(&backup)?;
        Ok(backup)
    }
}

/// 应用上下文：持有存储后端和配置。
///
/// 命令处理函数应通过此结构体获取所有外部依赖，
/// 避免直接调用全局函数或读取环境变量。
pub struct AppContext {
    pub storage: Arc<dyn StorageBackend>,
    pub config: crate::config::Config,
    pub i18n: crate::i18n::I18n,
    pool: Pool<SqliteConnectionManager>,
}

impl AppContext {
    /// 使用默认存储后端和已加载配置创建上下文。
    pub fn with_defaults() -> anyhow::Result<Self> {
        let storage: Arc<dyn StorageBackend> = Arc::new(DefaultStorageBackend);
        let path = storage.db_path()?;
        // 先执行 init_db() 确保数据库已初始化并迁移
        let _ = crate::registry::WorkspaceRegistry::init_db_at(&path)?;
        let pool = Self::build_pool(&path)?;
        let config = crate::config::Config::load()?;
        let i18n = crate::i18n::from_language(&config.general.language);
        Ok(Self { storage, config, i18n, pool })
    }

    /// 使用自定义存储后端创建上下文（主要用于测试）。
    pub fn with_storage(storage: Arc<dyn StorageBackend>) -> anyhow::Result<Self> {
        let path = storage.db_path()?;
        let _ = crate::registry::WorkspaceRegistry::init_db_at(&path)?;
        let pool = Self::build_pool(&path)?;
        let config = crate::config::Config::load()?;
        let i18n = crate::i18n::from_language(&config.general.language);
        Ok(Self { storage, config, i18n, pool })
    }

    fn build_pool(path: &std::path::Path) -> anyhow::Result<Pool<SqliteConnectionManager>> {
        let manager = SqliteConnectionManager::file(path).with_init(|c| {
            c.execute("PRAGMA foreign_keys = ON", [])?;
            Ok(())
        });
        Ok(Pool::builder().max_size(5).build(manager)?)
    }

    /// 获取数据库连接。
    pub fn conn(&self) -> anyhow::Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        Ok(self.pool.get()?)
    }

    /// 获取数据库连接（可变语义，与 conn() 等价）。
    pub fn conn_mut(&mut self) -> anyhow::Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        Ok(self.pool.get()?)
    }

    /// 获取连接池的克隆，用于 spawn_blocking / thread::spawn 闭包。
    pub fn pool(&self) -> Pool<SqliteConnectionManager> {
        self.pool.clone()
    }
}

impl crate::mcp::clients::ScanClient for AppContext {
    async fn scan_directory(
        &self,
        path: &str,
        register: bool,
    ) -> anyhow::Result<serde_json::Value> {
        crate::scan::run_json(path, register, &self.pool()).await
    }
}

impl crate::mcp::clients::HealthClient for AppContext {
    async fn check_health(&self, detail: bool) -> anyhow::Result<serde_json::Value> {
        let conn = self.conn()?;
        crate::health::run_json(&conn, detail, 0, 1, self.config.cache.ttl_seconds, &self.i18n)
            .await
    }
}

impl crate::mcp::clients::SyncClient for AppContext {
    async fn sync_repos(
        &self,
        dry_run: bool,
        filter_tags: Option<Vec<String>>,
    ) -> anyhow::Result<serde_json::Value> {
        let conn = self.conn()?;
        let filter_tags_str = filter_tags.as_deref().map(|v| v.join(","));
        crate::sync::run_json(&conn, dry_run, filter_tags_str.as_deref(), None, &self.i18n).await
    }
}

impl crate::mcp::clients::DigestClient for AppContext {
    fn generate_daily_digest(&self) -> anyhow::Result<serde_json::Value> {
        let conn = self.conn()?;
        let text = crate::digest::generate_daily_digest(&conn, &self.config, &self.i18n)?;
        Ok(serde_json::json!({ "success": true, "digest": text }))
    }
}

impl crate::mcp::clients::KnowledgeClient for AppContext {
    fn run_index(&self, path: &str) -> anyhow::Result<serde_json::Value> {
        let mut conn = self.conn()?;
        let count = crate::knowledge_engine::run_index(&mut conn, path)?;
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

impl crate::mcp::clients::RegistryClient for AppContext {
    fn list_repos(&self, _filter: Option<&str>) -> anyhow::Result<serde_json::Value> {
        let conn = self.conn()?;
        let repos = crate::registry::repo::list_repos(&conn)?;
        let results: Vec<serde_json::Value> = repos
            .into_iter()
            .map(|r| {
                serde_json::json!({
                    "id": r.id,
                    "local_path": r.local_path,
                    "language": r.language,
                    "tags": r.tags,
                    "workspace_type": r.workspace_type,
                    "data_tier": r.data_tier,
                })
            })
            .collect();
        Ok(serde_json::json!({ "success": true, "count": results.len(), "repos": results }))
    }

    fn get_repo(&self, repo_id: &str) -> anyhow::Result<serde_json::Value> {
        let conn = self.conn()?;
        let repos = crate::registry::repo::list_repos(&conn)?;
        match repos.into_iter().find(|r| r.id == repo_id) {
            Some(r) => Ok(serde_json::json!({
                "success": true,
                "id": r.id,
                "local_path": r.local_path,
                "language": r.language,
                "tags": r.tags,
                "workspace_type": r.workspace_type,
                "data_tier": r.data_tier,
            })),
            None => Ok(serde_json::json!({ "success": false, "error": "repo not found" })),
        }
    }

    fn list_modules(&self, repo_id: &str) -> anyhow::Result<serde_json::Value> {
        let conn = self.conn()?;
        let modules = crate::registry::knowledge::list_modules(&conn, repo_id)?;
        let results: Vec<serde_json::Value> = modules
            .into_iter()
            .map(|(name, ty, path)| {
                serde_json::json!({
                    "name": name,
                    "type": ty,
                    "path": path,
                })
            })
            .collect();
        Ok(serde_json::json!({ "success": true, "count": results.len(), "modules": results }))
    }

    fn save_paper(&self, paper: &serde_json::Value) -> anyhow::Result<serde_json::Value> {
        let conn = self.conn()?;
        let paper_entry: crate::registry::PaperEntry = serde_json::from_value(paper.clone())?;
        crate::registry::knowledge::save_paper(&conn, &paper_entry)?;
        Ok(serde_json::json!({ "success": true }))
    }

    fn save_experiment(&self, exp: &serde_json::Value) -> anyhow::Result<serde_json::Value> {
        let conn = self.conn()?;
        let exp_entry: crate::registry::ExperimentEntry = serde_json::from_value(exp.clone())?;
        crate::registry::WorkspaceRegistry::save_experiment(&conn, &exp_entry)?;
        Ok(serde_json::json!({ "success": true }))
    }

    fn list_code_metrics(&self) -> anyhow::Result<serde_json::Value> {
        let conn = self.conn()?;
        let metrics = crate::registry::metrics::list_code_metrics(&conn)?;
        let repos: Vec<serde_json::Value> = metrics
            .into_iter()
            .map(|(id, m)| {
                serde_json::json!({
                    "repo_id": id,
                    "total_lines": m.total_lines,
                    "source_lines": m.source_lines,
                    "test_lines": m.test_lines,
                    "comment_lines": m.comment_lines,
                    "file_count": m.file_count,
                    "language_breakdown": m.language_breakdown,
                    "updated_at": m.updated_at.to_rfc3339()
                })
            })
            .collect();
        Ok(serde_json::json!({ "success": true, "count": repos.len(), "repos": repos }))
    }

    fn get_code_metrics(&self, repo_id: &str) -> anyhow::Result<serde_json::Value> {
        let conn = self.conn()?;
        match crate::registry::metrics::get_code_metrics(&conn, repo_id)? {
            Some(m) => Ok(serde_json::json!({
                "success": true,
                "repo_id": repo_id,
                "total_lines": m.total_lines,
                "source_lines": m.source_lines,
                "test_lines": m.test_lines,
                "comment_lines": m.comment_lines,
                "file_count": m.file_count,
                "language_breakdown": m.language_breakdown,
                "updated_at": m.updated_at.to_rfc3339()
            })),
            None => {
                Ok(serde_json::json!({ "success": false, "error": "No metrics found for repo" }))
            }
        }
    }

    fn get_health(&self, repo_id: &str) -> anyhow::Result<serde_json::Value> {
        let conn = self.conn()?;
        match crate::registry::health::get_health(&conn, repo_id)? {
            Some(h) => Ok(serde_json::json!({
                "success": true,
                "repo_id": repo_id,
                "status": h.status,
                "ahead": h.ahead,
                "behind": h.behind,
                "checked_at": h.checked_at.to_rfc3339()
            })),
            None => Ok(serde_json::json!({ "success": false, "error": "No health data found" })),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    struct TempStorageBackend {
        dir: tempfile::TempDir,
    }

    impl TempStorageBackend {
        fn new() -> Self {
            Self {
                dir: tempfile::tempdir().unwrap(),
            }
        }
    }

    impl StorageBackend for TempStorageBackend {
        fn db_path(&self) -> anyhow::Result<PathBuf> {
            Ok(self.dir.path().join("registry.db"))
        }
        fn workspace_dir(&self) -> anyhow::Result<PathBuf> {
            let ws = self.dir.path().join("workspace");
            std::fs::create_dir_all(&ws)?;
            Ok(ws)
        }
        fn index_path(&self) -> anyhow::Result<PathBuf> {
            Ok(self.dir.path().join("search_index"))
        }
        fn backup_dir(&self) -> anyhow::Result<PathBuf> {
            Ok(self.dir.path().join("backups"))
        }
    }

    #[test]
    fn test_app_context_with_temp_storage() {
        let storage = Arc::new(TempStorageBackend::new());
        let ctx = AppContext::with_storage(storage).unwrap();
        let conn = ctx.conn().unwrap();
        let version: String =
            conn.query_row("SELECT sqlite_version()", [], |row| row.get(0)).unwrap();
        assert!(!version.is_empty());
    }
}
