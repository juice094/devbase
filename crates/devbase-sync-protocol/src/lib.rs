// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (c) 2026 juice094
//! devbase-sync-protocol — Lightweight directory sync protocol with version vectors.
//!
//! **提取日期**: 2026-05-01 (Workspace split)
//! **零内部耦合**: 此 crate 不依赖 devbase 任何内部模块，仅使用 walkdir + serde + chrono + anyhow。
//! **职责**: 扫描目录生成文件索引（`SyncIndex`），支持版本向量（VersionVector）用于冲突检测。
//! **边界**: 输入 `&Path`，输出 `SyncIndex`。不触及网络或数据库。
//!
//! 与 devbase 的关系: 被 devbase `watch` 模块用于文件系统监控，被 syncthing 集成用于本地索引。
//!
//! Design decisions:
//! - Hash = (size, mtime, path) 的 DefaultHasher: 轻量级，避免读取大文件内容。
//! - VersionVector 而非 Lamport clocks: 支持多设备并发写入的偏序比较。
//! - 跳过 `.git` 目录: 避免索引版本控制元数据。

pub mod index;
pub mod version_vector;

// Re-export for backward compatibility
pub use index::{FileInfo, SyncIndex};
pub use version_vector::{Counter, VersionVector};

use anyhow::Context;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::Path;

/// Lightweight directory scanner inspired by Syncthing's local index.
pub fn scan_directory(path: &Path) -> anyhow::Result<SyncIndex> {
    let mut files = Vec::new();

    for entry in walkdir::WalkDir::new(path).into_iter().filter_entry(|e| {
        // Skip .git directories
        if let Some(name) = e.file_name().to_str() {
            name != ".git"
        } else {
            true
        }
    }) {
        let entry = entry.with_context(|| "walkdir entry error")?;
        if !entry.file_type().is_file() {
            continue;
        }

        let meta = entry
            .metadata()
            .with_context(|| format!("failed to read metadata for {:?}", entry.path()))?;
        let size = meta.len();
        let mod_time = meta.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        let mod_time: chrono::DateTime<chrono::Utc> = mod_time.into();

        // Compute a lightweight hash: SHA256 of file content would be ideal,
        // but for a light abstraction we hash the (size, mod_time, path) tuple.
        // This avoids reading large files while still catching most changes.
        let mut hasher = DefaultHasher::new();
        size.hash(&mut hasher);
        mod_time.timestamp().hash(&mut hasher);
        entry.path().hash(&mut hasher);
        let hash_val = hasher.finish();
        let blocks_hash = Some(format!("{:016x}", hash_val));

        let name = entry
            .path()
            .strip_prefix(path)
            .unwrap_or(entry.path())
            .to_string_lossy()
            .replace('\\', "/");

        files.push(FileInfo {
            name,
            size,
            mod_time,
            version: VersionVector::default(),
            blocks_hash,
        });
    }

    // Sort for deterministic comparison
    files.sort_by(|a, b| a.name.cmp(&b.name));

    Ok(SyncIndex {
        path: path.to_path_buf(),
        files,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_counter_default() {
        let c = Counter::default();
        assert_eq!(c.id, 0);
        assert_eq!(c.value, 0);
    }

    #[test]
    fn test_version_vector_update_new() {
        let vv = VersionVector::default().update(1);
        assert_eq!(vv.counters.len(), 1);
        assert_eq!(vv.counters[0].id, 1);
        assert_eq!(vv.counters[0].value, 1);
    }

    #[test]
    fn test_version_vector_update_existing() {
        let vv = VersionVector::default().update(1).update(1);
        assert_eq!(vv.counters.len(), 1);
        assert_eq!(vv.counters[0].value, 2);
    }

    #[test]
    fn test_version_vector_merge() {
        let a = VersionVector::default().update(1);
        let b = VersionVector::default().update(2);
        let merged = a.merge(&b);
        assert_eq!(merged.counters.len(), 2);
        assert!(merged.counters.iter().any(|c| c.id == 1 && c.value == 1));
        assert!(merged.counters.iter().any(|c| c.id == 2 && c.value == 1));
    }

    #[test]
    fn test_version_vector_merge_max() {
        let a = VersionVector::default().update(1).update(1); // id=1, value=2
        let b = VersionVector::default().update(1); // id=1, value=1
        let merged = a.merge(&b);
        assert_eq!(merged.counters.len(), 1);
        assert_eq!(merged.counters[0].value, 2);
    }

    #[test]
    fn test_version_vector_compare_equal() {
        let a = VersionVector::default().update(1);
        let b = VersionVector::default().update(1);
        assert_eq!(a.compare(&b), std::cmp::Ordering::Equal);
    }

    #[test]
    fn test_version_vector_compare_greater() {
        let a = VersionVector::default().update(1).update(1);
        let b = VersionVector::default().update(1);
        assert_eq!(a.compare(&b), std::cmp::Ordering::Greater);
    }

    #[test]
    fn test_version_vector_compare_less() {
        let a = VersionVector::default().update(1);
        let b = VersionVector::default().update(1).update(1);
        assert_eq!(a.compare(&b), std::cmp::Ordering::Less);
    }

    #[test]
    fn test_version_vector_compare_conflict() {
        // Concurrent: a has higher id=1, b has higher id=2
        let a = VersionVector::default().update(1).update(1);
        let b = VersionVector::default().update(2).update(2);
        assert_eq!(a.compare(&b), std::cmp::Ordering::Equal);
    }

    #[test]
    fn test_version_vector_compare_empty() {
        let a = VersionVector::default();
        let b = VersionVector::default().update(1);
        assert_eq!(a.compare(&b), std::cmp::Ordering::Less);
        assert_eq!(b.compare(&a), std::cmp::Ordering::Greater);
    }

    #[test]
    fn test_scan_directory() {
        let tmp = std::env::temp_dir().join(format!("devbase_test_sync_{}", std::process::id()));
        std::fs::create_dir_all(&tmp).unwrap();
        std::fs::write(tmp.join("a.txt"), "hello").unwrap();
        std::fs::write(tmp.join("b.txt"), "world").unwrap();
        std::fs::create_dir_all(tmp.join(".git")).unwrap();
        std::fs::write(tmp.join(".git").join("ignore"), "x").unwrap();

        let idx = scan_directory(&tmp).unwrap();
        assert_eq!(idx.path, tmp);
        assert_eq!(idx.files.len(), 2);
        assert!(idx.files.iter().any(|f| f.name == "a.txt"));
        assert!(idx.files.iter().any(|f| f.name == "b.txt"));
        assert!(!idx.files.iter().any(|f| f.name.contains(".git")));

        std::fs::remove_dir_all(&tmp).unwrap();
    }

    #[test]
    fn test_scan_directory_empty() {
        let tmp =
            std::env::temp_dir().join(format!("devbase_test_sync_empty_{}", std::process::id()));
        std::fs::create_dir_all(&tmp).unwrap();
        let idx = scan_directory(&tmp).unwrap();
        assert!(idx.files.is_empty());
        std::fs::remove_dir_all(&tmp).unwrap();
    }
}
