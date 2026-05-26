// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (c) 2026 juice094

use crate::version_vector::VersionVector;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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
