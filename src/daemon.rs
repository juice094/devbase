use std::time::Duration;
use tokio::time::sleep;
use tracing::{info, warn};

pub struct Daemon {
    pub interval: Duration,
}

impl Daemon {
    pub fn new(interval_seconds: u64) -> Self {
        Self {
            interval: Duration::from_secs(interval_seconds),
        }
    }

    pub async fn run(&self) -> anyhow::Result<()> {
        info!("devbase daemon started, interval={:?}", self.interval);
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
        match tokio::task::spawn_blocking(|| {
            let conn = crate::registry::WorkspaceRegistry::init_db()?;
            let repos = crate::registry::WorkspaceRegistry::list_repos(&conn)?;
            for repo in repos {
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
            Ok::<_, anyhow::Error>(())
        })
        .await
        {
            Ok(Ok(())) => {}
            Ok(Err(e)) => warn!("Health check failed: {}", e),
            Err(e) => warn!("Health check task panicked: {}", e),
        }

        // 2. re-index recently modified repos
        match tokio::task::spawn_blocking(|| crate::knowledge_engine::run_index("")).await {
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
        match tokio::task::spawn_blocking(|| {
            let conn = crate::registry::WorkspaceRegistry::init_db()?;
            crate::digest::generate_daily_digest(&conn)
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
