use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    #[serde(default = "default_llm_enabled")]
    pub enabled: bool,
    #[serde(default = "default_llm_provider")]
    pub provider: String,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub base_url: Option<String>,
    #[serde(default = "default_llm_max_tokens")]
    pub max_tokens: u32,
    #[serde(default = "default_llm_timeout_seconds")]
    pub timeout_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncConfig {
    #[serde(default = "default_sync_timeout_seconds")]
    pub timeout_seconds: u64,
    #[serde(default = "default_sync_concurrency")]
    pub concurrency: usize,
}

impl Default for SyncConfig {
    fn default() -> Self {
        Self {
            timeout_seconds: default_sync_timeout_seconds(),
            concurrency: default_sync_concurrency(),
        }
    }
}

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
    #[serde(default)]
    pub github: GithubConfig,
    #[serde(default)]
    pub llm: LlmConfig,
    #[serde(default)]
    pub sync: SyncConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GithubConfig {
    #[serde(default)]
    pub token: Option<String>,
    #[serde(default = "default_github_timeout_seconds")]
    pub timeout_seconds: u64,
}

impl Default for GithubConfig {
    fn default() -> Self {
        Self {
            token: None,
            timeout_seconds: default_github_timeout_seconds(),
        }
    }
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
            github: GithubConfig::default(),
            llm: LlmConfig::default(),
            sync: SyncConfig::default(),
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

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            enabled: default_llm_enabled(),
            provider: default_llm_provider(),
            api_key: None,
            model: None,
            base_url: None,
            max_tokens: default_llm_max_tokens(),
            timeout_seconds: default_llm_timeout_seconds(),
        }
    }
}

fn default_llm_enabled() -> bool { false }
fn default_llm_provider() -> String { "ollama".to_string() }
fn default_llm_max_tokens() -> u32 { 200 }
fn default_llm_timeout_seconds() -> u64 { 30 }
fn default_sync_timeout_seconds() -> u64 { 60 }
fn default_sync_concurrency() -> usize { 8 }
fn default_github_timeout_seconds() -> u64 { 5 }

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
            let config = Self::default();
            let _ = config.save_default();
            return Ok(config);
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

    /// Write a default config file with inline comments for first-time users.
    pub fn save_default(&self) -> anyhow::Result<()> {
        let path = Self::config_path()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = r#"# devbase configuration file
# Generated automatically on first run. Modify as needed.

[general]
# UI language: "auto", "en", or "zh"
language = "auto"

[daemon]
# Background maintenance interval in seconds
interval_seconds = 3600
incremental = true
health_stale_hours = 24

[cache]
# How long to cache health/stars data before re-fetching (seconds)
ttl_seconds = 300

[watch]
max_files = 512

[digest]
window_hours = 24

[github]
# Uncomment and set your GitHub Personal Access Token to avoid API rate limits
# token = "ghp_xxxxxxxxxxxxxxxxxxxx"
timeout_seconds = 5

[llm]
enabled = false
provider = "ollama"
# api_key = ""
# model = ""
# base_url = ""
max_tokens = 200
timeout_seconds = 30

[sync]
# Max concurrent sync operations
timeout_seconds = 60
concurrency = 8
"#;
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
