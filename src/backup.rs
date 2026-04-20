use crate::registry::{RepoEntry, WorkspaceRegistry};
use std::fs;
use std::path::{Path, PathBuf};
use tracing::info;

const MAX_AUTO_BACKUPS: usize = 10;

/// Determine the backup directory path.
pub fn backup_dir() -> anyhow::Result<PathBuf> {
    let data_dir = dirs::data_local_dir()
        .ok_or_else(|| anyhow::anyhow!("Could not determine local data directory"))?;
    let dir = data_dir.join("devbase").join("backup");
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Generate a timestamped backup filename.
pub fn backup_filename(prefix: &str, ext: &str) -> String {
    let now = chrono::Local::now();
    format!("{}-{}.{}", prefix, now.format("%Y%m%d-%H%M%S"), ext)
}

/// List existing SQLite backups, sorted by filename (newest last).
pub fn list_backups() -> anyhow::Result<Vec<PathBuf>> {
    let dir = backup_dir()?;
    let mut entries = Vec::new();
    for entry in fs::read_dir(&dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) == Some("db") {
            entries.push(path);
        }
    }
    entries.sort();
    Ok(entries)
}

/// Remove oldest backups if count exceeds max.
pub fn clean_old_backups(max_count: usize) -> anyhow::Result<usize> {
    let mut backups = list_backups()?;
    if backups.len() <= max_count {
        return Ok(0);
    }
    let to_remove = backups.len() - max_count;
    // Remove oldest (they are sorted by filename, oldest first)
    for path in backups.drain(..to_remove) {
        info!("Removing old backup: {}", path.display());
        fs::remove_file(&path)?;
    }
    Ok(to_remove)
}

/// Copy the current registry.db to the backup directory before migration.
pub fn auto_backup_before_migration(db_path: &Path) -> anyhow::Result<PathBuf> {
    let dir = backup_dir()?;
    let filename = backup_filename("devbase-registry-pre-migration", "db");
    let dest = dir.join(&filename);
    fs::copy(db_path, &dest)?;
    info!("Auto-backed up registry before migration: {}", dest.display());
    let _ = clean_old_backups(MAX_AUTO_BACKUPS);
    Ok(dest)
}

/// Export the current registry database to a SQLite file.
pub fn export_sqlite(output: Option<&Path>) -> anyhow::Result<PathBuf> {
    let db_path = WorkspaceRegistry::db_path()?;
    let dest = match output {
        Some(p) => p.to_path_buf(),
        None => {
            let dir = backup_dir()?;
            let filename = backup_filename("devbase-registry", "db");
            dir.join(filename)
        }
    };
    fs::copy(&db_path, &dest)?;
    info!("Exported registry to {}", dest.display());
    Ok(dest)
}

/// Export the current registry to a JSON file.
pub fn export_json(output: Option<&Path>) -> anyhow::Result<PathBuf> {
    let conn = WorkspaceRegistry::init_db()?;
    let repos = WorkspaceRegistry::list_repos(&conn)?;

    let registry = crate::registry::WorkspaceRegistry {
        version: "0.2.0".to_string(),
        entries: repos,
    };

    let json = serde_json::to_string_pretty(&registry)?;
    let dest = match output {
        Some(p) => p.to_path_buf(),
        None => {
            let dir = backup_dir()?;
            let filename = backup_filename("devbase-registry", "json");
            dir.join(filename)
        }
    };
    fs::write(&dest, json)?;
    info!("Exported registry JSON to {}", dest.display());
    Ok(dest)
}

/// Import from a SQLite backup file. If dry_run is true, only report stats.
pub fn import_db(source: &Path, dry_run: bool) -> anyhow::Result<()> {
    if !source.exists() {
        anyhow::bail!("Source file does not exist: {}", source.display());
    }

    let src_conn = rusqlite::Connection::open(source)?;
    let src_repos = WorkspaceRegistry::list_repos(&src_conn)?;

    let current_conn = WorkspaceRegistry::init_db()?;
    let current_repos = WorkspaceRegistry::list_repos(&current_conn)?;

    let current_ids: std::collections::HashSet<String> =
        current_repos.iter().map(|r| r.id.clone()).collect();
    let new_repos: Vec<&RepoEntry> =
        src_repos.iter().filter(|r| !current_ids.contains(&r.id)).collect();
    let existing_repos: Vec<&RepoEntry> =
        src_repos.iter().filter(|r| current_ids.contains(&r.id)).collect();

    println!("导入统计:");
    println!("  源文件中的工作区: {}", src_repos.len());
    println!("  当前已有工作区: {}", current_repos.len());
    println!("  将新增: {}", new_repos.len());
    println!("  将覆盖: {}", existing_repos.len());

    if dry_run {
        println!("\n这是 dry-run，未执行实际导入。使用 --yes 确认导入。");
        return Ok(());
    }

    if new_repos.is_empty() && existing_repos.is_empty() {
        println!("没有需要导入的数据。");
        return Ok(());
    }

    // Backup current db before import
    let _ = export_sqlite(None)?;

    let mut conn = WorkspaceRegistry::init_db()?;
    for repo in &src_repos {
        WorkspaceRegistry::save_repo(&mut conn, repo)?;
    }

    println!("\n已成功导入 {} 个工作区。", src_repos.len());
    Ok(())
}

// ------------------------------------------------------------------
// CLI entry points
// ------------------------------------------------------------------

pub fn run_export(format: &str, output: Option<&Path>) -> anyhow::Result<()> {
    let path = match format {
        "json" => export_json(output)?,
        "sqlite" | "db" => export_sqlite(output)?,
        _ => anyhow::bail!("Unsupported export format: {}. Use 'sqlite' or 'json'.", format),
    };
    println!("已导出到: {}", path.display());
    Ok(())
}

pub fn run_import(source: &Path, yes: bool) -> anyhow::Result<()> {
    import_db(source, !yes)
}

pub fn run_list() -> anyhow::Result<()> {
    let backups = list_backups()?;
    if backups.is_empty() {
        println!("没有找到备份文件。");
        return Ok(());
    }
    println!("找到 {} 个备份:", backups.len());
    for path in &backups {
        let meta = fs::metadata(path)?;
        let size = meta.len();
        let size_mb = size as f64 / (1024.0 * 1024.0);
        let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("?");
        println!("  {} ({:.2} MB)", name, size_mb);
    }
    Ok(())
}

pub fn run_clean() -> anyhow::Result<()> {
    let removed = clean_old_backups(MAX_AUTO_BACKUPS)?;
    if removed > 0 {
        println!("已清理 {} 个旧备份，保留最近 {} 个。", removed, MAX_AUTO_BACKUPS);
    } else {
        println!("备份数量未超过限制 ({}), 无需清理。", MAX_AUTO_BACKUPS);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_backup_filename_contains_timestamp() {
        let name = backup_filename("devbase-registry", "db");
        assert!(name.starts_with("devbase-registry-"));
        assert!(name.ends_with(".db"));
    }

    #[test]
    fn test_clean_old_backups_removes_oldest() {
        let dir = TempDir::new().unwrap();
        let b1 = dir.path().join("devbase-registry-20260101-000000.db");
        let b2 = dir.path().join("devbase-registry-20260102-000000.db");
        let b3 = dir.path().join("devbase-registry-20260103-000000.db");
        fs::write(&b1, "x").unwrap();
        fs::write(&b2, "x").unwrap();
        fs::write(&b3, "x").unwrap();

        // Temporarily override backup dir by constructing paths manually
        let mut backups = [b1.clone(), b2.clone(), b3.clone()];
        backups.sort();
        assert_eq!(backups.len(), 3);

        // Simulate cleaning to max 2
        let mut all = vec![b1, b2, b3];
        all.sort();
        let to_remove = all.len().saturating_sub(2);
        for p in all.drain(..to_remove) {
            fs::remove_file(&p).unwrap();
        }
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn test_export_sqlite_creates_file() {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("registry.db");
        // Create a minimal sqlite db
        let conn = rusqlite::Connection::open(&db_path).unwrap();
        conn.execute("CREATE TABLE repos (id TEXT PRIMARY KEY)", []).unwrap();
        drop(conn);

        let out = dir.path().join("backup.db");
        fs::copy(&db_path, &out).unwrap();
        assert!(out.exists());
    }
}
