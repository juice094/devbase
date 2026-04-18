use std::time::Duration;
use tokio::time::sleep;
use tracing::{info, warn};

pub struct Daemon {
    pub interval: Duration,
    pub config: crate::config::Config,
}

impl Daemon {
    pub fn new(interval_seconds: u64, config: crate::config::Config) -> Self {
        Self {
            interval: Duration::from_secs(interval_seconds),
            config,
        }
    }

    pub async fn run(&self) -> anyhow::Result<()> {
        info!("devbase daemon started, interval={:?}", self.interval);
        self.tick_loop().await;
        Ok(())
    }

    async fn tick_loop(&self) {
        loop {
            if let Err(e) = self.tick().await {
                warn!("Daemon tick failed: {}", e);
            }
            sleep(self.interval).await;
        }
    }

    async fn tick(&self) -> anyhow::Result<()> {
        info!("Daemon tick start");

        // 1. health check for stale repos
        let stale_threshold = if self.config.daemon.incremental {
            Some((chrono::Utc::now() - chrono::Duration::hours(self.config.daemon.health_stale_hours)).to_rfc3339())
        } else {
            None
        };
        match tokio::task::spawn_blocking(move || {
            let conn = crate::registry::WorkspaceRegistry::init_db()?;
            let repos = if let Some(threshold) = stale_threshold {
                crate::registry::WorkspaceRegistry::list_repos_stale_health(&conn, &threshold)?
            } else {
                crate::registry::WorkspaceRegistry::list_repos(&conn)?
            };
            for repo in &repos {
                let primary = repo.primary_remote();
                let upstream_url = primary.and_then(|r| r.upstream_url.as_deref());
                let default_branch = primary.and_then(|r| r.default_branch.as_deref());
                let (status, ahead, behind) =
                    crate::health::analyze_repo(repo.local_path.to_string_lossy().as_ref(), upstream_url, default_branch);
                let health = crate::registry::HealthEntry {
                    status: status.clone(),
                    ahead,
                    behind,
                    checked_at: chrono::Utc::now(),
                };
                if let Err(e) = crate::registry::WorkspaceRegistry::save_health(&conn, &repo.id, &health) {
                    tracing::warn!("Failed to save health for {}: {}", repo.id, e);
                }
            }
            Ok::<_, anyhow::Error>(repos.len())
        })
        .await
        {
            Ok(Ok(count)) => info!("Health check: {} stale repos", count),
            Ok(Err(e)) => warn!("Health check failed: {}", e),
            Err(e) => warn!("Health check task panicked: {}", e),
        }

        // 2. re-index recently modified repos
        let index_threshold = if self.config.daemon.incremental {
            Some((chrono::Utc::now() - chrono::Duration::hours(24)).to_rfc3339())
        } else {
            None
        };
        match tokio::task::spawn_blocking(move || {
            let conn = crate::registry::WorkspaceRegistry::init_db()?;
            let repos = if let Some(threshold) = index_threshold {
                crate::registry::WorkspaceRegistry::list_repos_need_index(&conn, &threshold)?
            } else {
                crate::registry::WorkspaceRegistry::list_repos(&conn)?
            };
            let mut count = 0;
            for repo in repos {
                if let Err(e) = crate::knowledge_engine::index_repo(&repo) {
                    tracing::warn!("Failed to index {}: {}", repo.id, e);
                } else {
                    count += 1;
                }
            }
            Ok::<_, anyhow::Error>(count)
        })
        .await
        {
            Ok(Ok(count)) => info!("Re-indexed {} repositories", count),
            Ok(Err(e)) => warn!("Re-index failed: {}", e),
            Err(e) => warn!("Re-index task panicked: {}", e),
        }

        // 3. run discovery engine
        match tokio::task::spawn_blocking(|| {
            let conn = crate::registry::WorkspaceRegistry::init_db()?;
            let repos = crate::registry::WorkspaceRegistry::list_repos(&conn)?;
            let deps = crate::discovery_engine::discover_dependencies(&repos);
            let sims = crate::discovery_engine::discover_similar_projects(&conn)?;
            for d in deps.into_iter().chain(sims) {
                let _ = crate::registry::WorkspaceRegistry::save_relation(
                    &conn,
                    &d.from,
                    &d.to,
                    &d.relation_type,
                    d.confidence,
                );
                let repo_id = if d.from.is_empty() { None } else { Some(d.from.as_str()) };
                let _ = crate::registry::WorkspaceRegistry::save_discovery(
                    &conn,
                    repo_id,
                    &d.relation_type,
                    &d.description,
                    d.confidence,
                );
            }
            Ok::<_, anyhow::Error>(())
        })
        .await
        {
            Ok(Ok(())) => {}
            Ok(Err(e)) => warn!("Discovery failed: {}", e),
            Err(e) => warn!("Discovery task panicked: {}", e),
        }

        // 4. generate digest if it's morning (or every N ticks)
        let digest_config = self.config.digest.clone();
        match tokio::task::spawn_blocking(move || {
            let conn = crate::registry::WorkspaceRegistry::init_db()?;
            let cfg = crate::config::Config {
                digest: digest_config,
                ..Default::default()
            };
            crate::digest::generate_daily_digest(&conn, &cfg)
        })
        .await
        {
            Ok(Ok(text)) => {
                info!("Daily digest generated:\n{}", text);
            }
            Ok(Err(e)) => {
                warn!("Failed to generate digest: {}", e);
            }
            Err(e) => {
                warn!("Digest task panicked: {}", e);
            }
        }

        info!("Daemon tick complete");
        Ok(())
    }
}
