use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub general: GeneralConfig,
    #[serde(default)]
    pub daemon: DaemonConfig,
    #[serde(default)]
    pub cache: CacheConfig,
    #[serde(default)]
    pub watch: WatchConfig,
    #[serde(default)]
    pub digest: DigestConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
    #[serde(default = "default_language")]
    pub language: String,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self { language: default_language() }
    }
}

fn default_language() -> String { "auto".to_string() }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonConfig {
    #[serde(default = "default_daemon_interval_seconds")]
    pub interval_seconds: u64,
    #[serde(default = "default_true")]
    pub incremental: bool,
    #[serde(default = "default_health_stale_hours")]
    pub health_stale_hours: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    #[serde(default = "default_cache_ttl_seconds")]
    pub ttl_seconds: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchConfig {
    #[serde(default = "default_watch_max_files")]
    pub max_files: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DigestConfig {
    #[serde(default = "default_digest_window_hours")]
    pub window_hours: i64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            general: GeneralConfig::default(),
            daemon: DaemonConfig::default(),
            cache: CacheConfig::default(),
            watch: WatchConfig::default(),
            digest: DigestConfig::default(),
        }
    }
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            interval_seconds: default_daemon_interval_seconds(),
            incremental: default_true(),
            health_stale_hours: default_health_stale_hours(),
        }
    }
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            ttl_seconds: default_cache_ttl_seconds(),
        }
    }
}

impl Default for WatchConfig {
    fn default() -> Self {
        Self {
            max_files: default_watch_max_files(),
        }
    }
}

impl Default for DigestConfig {
    fn default() -> Self {
        Self {
            window_hours: default_digest_window_hours(),
        }
    }
}

fn default_daemon_interval_seconds() -> u64 { 3600 }
fn default_true() -> bool { true }
fn default_health_stale_hours() -> i64 { 24 }
fn default_cache_ttl_seconds() -> i64 { 300 }
pub fn default_watch_max_files() -> usize { 512 }
fn default_digest_window_hours() -> i64 { 24 }

impl Config {
    pub fn load() -> anyhow::Result<Self> {
        let path = Self::config_path()?;
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(&path)?;
        let config: Self = toml::from_str(&content)?;
        Ok(config)
    }

    pub fn save(&self) -> anyhow::Result<()> {
        let path = Self::config_path()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(self)?;
        std::fs::write(&path, content)?;
        Ok(())
    }

    pub fn config_path() -> anyhow::Result<PathBuf> {
        let dir = dirs::config_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine config directory"))?
            .join("devbase");
        Ok(dir.join("config.toml"))
    }
}
