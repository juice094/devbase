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
    conn: rusqlite::Connection,
}

impl AppContext {
    /// 使用默认存储后端和已加载配置创建上下文。
    pub fn with_defaults() -> anyhow::Result<Self> {
        let storage: Arc<dyn StorageBackend> = Arc::new(DefaultStorageBackend);
        let conn = crate::registry::WorkspaceRegistry::init_db()?;
        Ok(Self {
            storage,
            config: crate::config::Config::load()?,
            conn,
        })
    }

    /// 使用自定义存储后端创建上下文（主要用于测试）。
    pub fn with_storage(storage: Arc<dyn StorageBackend>) -> anyhow::Result<Self> {
        let conn = crate::registry::WorkspaceRegistry::init_db()?;
        Ok(Self {
            storage,
            config: crate::config::Config::load()?,
            conn,
        })
    }

    /// 获取数据库连接的不可变引用。
    pub fn conn(&self) -> &rusqlite::Connection {
        &self.conn
    }

    /// 获取数据库连接的可变引用。
    pub fn conn_mut(&mut self) -> &mut rusqlite::Connection {
        &mut self.conn
    }
}
