use anyhow::Context;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Counter {
    pub id: u64,
    pub value: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct VersionVector {
    pub counters: Vec<Counter>,
}

impl VersionVector {
    /// Increment the counter for `local_id`, creating it if absent.
    pub fn update(mut self, local_id: u64) -> Self {
        for c in &mut self.counters {
            if c.id == local_id {
                c.value += 1;
                return self;
            }
        }
        self.counters.push(Counter {
            id: local_id,
            value: 1,
        });
        self
    }

    /// Merge with another vector, taking the maximum value for each id.
    pub fn merge(mut self, other: &VersionVector) -> Self {
        for o in &other.counters {
            let mut found = false;
            for c in &mut self.counters {
                if c.id == o.id {
                    c.value = c.value.max(o.value);
                    found = true;
                    break;
                }
            }
            if !found {
                self.counters.push(o.clone());
            }
        }
        self
    }

    /// Compare two version vectors.
    ///
    /// - Greater  => self dominates other (all >= and at least one >)
    /// - Less     => other dominates self
    /// - Equal    => identical or concurrent conflict (incomparable)
    pub fn compare(&self, other: &VersionVector) -> Ordering {
        let mut self_map = std::collections::HashMap::new();
        for c in &self.counters {
            self_map.insert(c.id, c.value);
        }
        let mut other_map = std::collections::HashMap::new();
        for c in &other.counters {
            other_map.insert(c.id, c.value);
        }

        let all_ids: std::collections::HashSet<u64> = self_map
            .keys()
            .chain(other_map.keys())
            .copied()
            .collect();

        let mut has_greater = false;
        let mut has_less = false;
        for id in all_ids {
            let sv = self_map.get(&id).copied().unwrap_or(0);
            let ov = other_map.get(&id).copied().unwrap_or(0);
            if sv > ov {
                has_greater = true;
            } else if sv < ov {
                has_less = true;
            }
        }

        match (has_greater, has_less) {
            (true, false) => Ordering::Greater,
            (false, true) => Ordering::Less,
            _ => {
                // Both false => equal; both true => conflict => Equal per spec
                Ordering::Equal
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileInfo {
    pub name: String,
    pub size: u64,
    pub mod_time: DateTime<Utc>,
    pub version: VersionVector,
    pub blocks_hash: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncIndex {
    pub path: PathBuf,
    pub files: Vec<FileInfo>,
}

/// Lightweight directory scanner inspired by Syncthing's local index.
pub fn scan_directory(path: &Path) -> anyhow::Result<SyncIndex> {
    let mut files = Vec::new();

    for entry in walkdir::WalkDir::new(path)
        .into_iter()
        .filter_entry(|e| {
            // Skip .git directories
            if let Some(name) = e.file_name().to_str() {
                name != ".git"
            } else {
                true
            }
        })
    {
        let entry = entry.with_context(|| "walkdir entry error")?;
        if !entry.file_type().is_file() {
            continue;
        }

        let meta = entry.metadata().with_context(|| {
            format!("failed to read metadata for {:?}", entry.path())
        })?;
        let size = meta.len();
        let mod_time = meta
            .modified()
            .unwrap_or_else(|_| std::time::SystemTime::UNIX_EPOCH);
        let mod_time: DateTime<Utc> = mod_time.into();

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
