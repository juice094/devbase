// SPDX-License-Identifier: MIT
// Copyright (c) 2026 juice094
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Session-level cache for environment tool versions.
/// Avoids spawning subprocesses on every health check.
#[derive(Debug, Clone, Default)]
pub struct EnvVersionCache {
    pub rustc: Option<String>,
    pub cargo: Option<String>,
    pub node: Option<String>,
    pub go: Option<String>,
    pub cmake: Option<String>,
    pub python: Option<String>,
    pub bun: Option<String>,
    pub zig: Option<String>,
    pub java: Option<String>,
    pub fetched_at: Option<Instant>,
}

impl EnvVersionCache {
    const TTL: Duration = Duration::from_secs(30);

    pub fn is_fresh(&self) -> bool {
        self.fetched_at.map(|t| t.elapsed() < Self::TTL).unwrap_or(false)
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }
}

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

    /// Tantivy 代码符号搜索索引目录。
    fn symbol_index_path(&self) -> anyhow::Result<PathBuf>;

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

    fn symbol_index_path(&self) -> anyhow::Result<PathBuf> {
        let dir = self.data_base()?;
        std::fs::create_dir_all(&dir)?;
        Ok(dir.join("symbol_index"))
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
    env_cache: std::sync::Mutex<EnvVersionCache>,
}

impl AppContext {
    /// 使用默认存储后端和已加载配置创建上下文。
    pub fn with_defaults() -> anyhow::Result<Self> {
        let storage: Arc<dyn StorageBackend> = Arc::new(DefaultStorageBackend);
        let path = storage.db_path()?;
        // 先执行 init_db_with 确保数据库已初始化并迁移
        let mut conn = crate::registry::WorkspaceRegistry::init_db_with(&*storage)?;
        if let Err(e) = repair_tantivy_consistency(&mut conn) {
            tracing::warn!("Startup Tantivy consistency check failed: {}", e);
        }
        if let Err(e) = crate::search::sync_index_to_db(&conn) {
            tracing::warn!("Startup Tantivy/SQLite orphan sync failed: {}", e);
        }
        drop(conn);
        let pool = Self::build_pool(&path)?;
        let config = crate::config::Config::load()?;
        let i18n = crate::i18n::from_language(&config.general.language);
        Ok(Self {
            storage,
            config,
            i18n,
            pool,
            env_cache: std::sync::Mutex::new(EnvVersionCache::default()),
        })
    }

    /// 使用自定义存储后端创建上下文（主要用于测试）。
    pub fn with_storage(storage: Arc<dyn StorageBackend>) -> anyhow::Result<Self> {
        let path = storage.db_path()?;
        let mut conn = crate::registry::WorkspaceRegistry::init_db_with(&*storage)?;
        if let Err(e) = repair_tantivy_consistency(&mut conn) {
            tracing::warn!("Startup Tantivy consistency check failed: {}", e);
        }
        if let Err(e) = crate::search::sync_index_to_db(&conn) {
            tracing::warn!("Startup Tantivy/SQLite orphan sync failed: {}", e);
        }
        drop(conn);
        let pool = Self::build_pool(&path)?;
        let config = crate::config::Config::load()?;
        let i18n = crate::i18n::from_language(&config.general.language);
        Ok(Self {
            storage,
            config,
            i18n,
            pool,
            env_cache: std::sync::Mutex::new(EnvVersionCache::default()),
        })
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

    /// 获取环境版本缓存的只读快照。
    pub fn env_cache(&self) -> anyhow::Result<EnvVersionCache> {
        let guard = self
            .env_cache
            .lock()
            .map_err(|e| anyhow::anyhow!("env_cache poisoned: {}", e))?;
        Ok(guard.clone())
    }

    /// 更新环境版本缓存。
    pub fn set_env_cache(&self, cache: EnvVersionCache) -> anyhow::Result<()> {
        let mut guard = self
            .env_cache
            .lock()
            .map_err(|e| anyhow::anyhow!("env_cache poisoned: {}", e))?;
        *guard = cache;
        Ok(())
    }
}

/// Result of a startup consistency scan.
#[allow(dead_code)]
pub(crate) struct RepairResult {
    /// Tantivy documents whose repo no longer exists in SQLite.
    pub orphans: usize,
    /// SQLite entities that are missing from the Tantivy index.
    pub missing_from_index: usize,
}

/// Startup consistency scan: detect Tantivy documents whose repo no longer exists in SQLite.
/// Also detects SQLite repos that are missing from the Tantivy index.
/// Inserts orphan records into `orphan_tantivy_docs` for lazy cleanup during next index.
pub(crate) fn repair_tantivy_consistency(
    conn: &mut rusqlite::Connection,
) -> anyhow::Result<RepairResult> {
    let backend = crate::storage::DefaultStorageBackend {};
    let index_path = match backend.index_path() {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!("Failed to resolve index path: {}", e);
            return Ok(RepairResult {
                orphans: 0,
                missing_from_index: 0,
            });
        }
    };
    repair_tantivy_consistency_at(&index_path, conn)
}

/// Repair Tantivy consistency at an explicit index path, bypassing global storage backend.
pub(crate) fn repair_tantivy_consistency_at(
    index_path: &std::path::Path,
    conn: &mut rusqlite::Connection,
) -> anyhow::Result<RepairResult> {
    let tantivy_ids: std::collections::HashSet<String> =
        match crate::search::list_indexed_repo_ids_at(index_path) {
            Ok(ids) => ids.into_iter().collect(),
            Err(e) => {
                tracing::warn!("Failed to list Tantivy repo IDs: {}", e);
                return Ok(RepairResult {
                    orphans: 0,
                    missing_from_index: 0,
                });
            }
        };

    let sqlite_ids: std::collections::HashSet<String> = {
        let mut stmt = conn.prepare("SELECT id FROM entities WHERE entity_type = ?1")?;
        let rows =
            stmt.query_map([crate::registry::ENTITY_TYPE_REPO], |row| row.get::<_, String>(0))?;
        rows.filter_map(Result::ok).collect()
    };

    // Clear stale orphans: repos that are now present in SQLite but still marked orphan
    {
        let mut stmt = conn.prepare("SELECT repo_id FROM orphan_tantivy_docs")?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        for repo_id in rows.filter_map(Result::ok) {
            if sqlite_ids.contains(&repo_id) || !tantivy_ids.contains(&repo_id) {
                conn.execute("DELETE FROM orphan_tantivy_docs WHERE repo_id = ?1", [&repo_id])?;
            }
        }
    }

    // Record new orphans: Tantivy has doc but SQLite has no entity
    let mut orphaned = 0usize;
    for repo_id in &tantivy_ids {
        if !sqlite_ids.contains(repo_id) {
            conn.execute(
                "INSERT OR IGNORE INTO orphan_tantivy_docs (repo_id) VALUES (?1)",
                [repo_id],
            )?;
            orphaned += 1;
        }
    }

    if orphaned > 0 {
        tracing::info!("Detected {} orphan Tantivy document(s)", orphaned);
    }

    // Reverse check: SQLite entities missing from Tantivy
    let mut missing = 0usize;
    for repo_id in &sqlite_ids {
        if !tantivy_ids.contains(repo_id) {
            tracing::warn!(
                "repo {} exists in SQLite but missing from Tantivy index; needs re-index",
                repo_id
            );
            missing += 1;
        }
    }

    Ok(RepairResult {
        orphans: orphaned,
        missing_from_index: missing,
    })
}

/// Test-only storage backend that uses an independent temporary directory.
/// Eliminates `DEVBASE_DATA_DIR` environment-variable races during parallel tests.
#[cfg(test)]
pub(crate) struct TempStorageBackend {
    dir: tempfile::TempDir,
}

#[cfg(test)]
impl TempStorageBackend {
    pub fn new() -> Self {
        Self {
            dir: tempfile::tempdir().unwrap(),
        }
    }
}

#[cfg(test)]
impl StorageBackend for TempStorageBackend {
    fn db_path(&self) -> anyhow::Result<PathBuf> {
        Ok(self.dir.path().join("registry.db"))
    }
    fn workspace_dir(&self) -> anyhow::Result<PathBuf> {
        let ws = self.dir.path().join("workspace");
        std::fs::create_dir_all(&ws)?;
        std::fs::create_dir_all(ws.join("vault"))?;
        std::fs::create_dir_all(ws.join("assets"))?;
        Ok(ws)
    }
    fn index_path(&self) -> anyhow::Result<PathBuf> {
        let dir = self.dir.path();
        std::fs::create_dir_all(dir)?;
        Ok(dir.join("search_index"))
    }
    fn symbol_index_path(&self) -> anyhow::Result<PathBuf> {
        let dir = self.dir.path();
        std::fs::create_dir_all(dir)?;
        Ok(dir.join("symbol_index"))
    }
    fn backup_dir(&self) -> anyhow::Result<PathBuf> {
        Ok(self.dir.path().join("backups"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_context_with_temp_storage() {
        let storage = Arc::new(TempStorageBackend::new());
        let ctx = AppContext::with_storage(storage).unwrap();
        let conn = ctx.conn().unwrap();
        let version: String =
            conn.query_row("SELECT sqlite_version()", [], |row| row.get(0)).unwrap();
        assert!(!version.is_empty());
    }

    #[test]
    fn test_repair_tantivy_consistency_detects_orphan() {
        let backend = crate::storage::TempStorageBackend::new();
        let index_path = backend.index_path().unwrap();
        let db_path = backend.db_path().unwrap();

        // Initialize DB with full schema
        let mut conn = crate::registry::WorkspaceRegistry::init_db_at(&db_path).unwrap();

        // Add a Tantivy doc for a repo that does NOT exist in SQLite
        let (index, _reader) = crate::search::init_index_at(&index_path).unwrap();
        let mut writer = crate::search::get_writer(&index).unwrap();
        let schema = index.schema();
        crate::search::add_repo_doc(
            &mut writer,
            &schema,
            "ghost_repo",
            "Ghost",
            "ghost content",
            &[],
        )
        .unwrap();
        crate::search::commit_writer(&mut writer).unwrap();
        drop(writer);
        drop(index);
        // Windows releases Tantivy mmap handles asynchronously.
        // Under parallel test execution this can take >500ms.
        std::thread::sleep(std::time::Duration::from_millis(800));

        // Repair should detect the orphan
        let result = repair_tantivy_consistency_at(&index_path, &mut conn).unwrap();
        assert_eq!(result.orphans, 1);
        assert_eq!(result.missing_from_index, 0);

        let orphan_exists: bool = conn
            .query_row("SELECT 1 FROM orphan_tantivy_docs WHERE repo_id = 'ghost_repo'", [], |_| {
                Ok(true)
            })
            .unwrap_or(false);
        assert!(orphan_exists);

        // Now create the missing entity in SQLite
        conn.execute(
            "INSERT INTO entities (id, entity_type, name, local_path, metadata, created_at, updated_at)
             VALUES ('ghost_repo', 'repo', 'Ghost', '/tmp/ghost', '{}', datetime('now'), datetime('now'))",
            [],
        ).unwrap();

        // Repair should now find zero orphans and clear the record
        let result2 = repair_tantivy_consistency_at(&index_path, &mut conn).unwrap();
        assert_eq!(result2.orphans, 0);
        assert_eq!(result2.missing_from_index, 0);

        let orphan_still_exists: bool = conn
            .query_row("SELECT 1 FROM orphan_tantivy_docs WHERE repo_id = 'ghost_repo'", [], |_| {
                Ok(true)
            })
            .unwrap_or(false);
        assert!(!orphan_still_exists);
    }
}
