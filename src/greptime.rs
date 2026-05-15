// SPDX-License-Identifier: MIT
// Copyright (c) 2026 juice094
//! GreptimeDB integration layer (feature-gated).
//!
//! Provides optional async ingestion of time-series data:
//! - repo health (ahead/behind/status)
//! - code metrics (LOC/language breakdown)
//! - GitHub stars history
//!
//! When the `greptimedb` feature is disabled this module compiles to no-ops.

use crate::config::GreptimeConfig;

/// Shared GreptimeDB client handle.
pub struct GreptimeClient {
    #[cfg(feature = "greptimedb")]
    inner: Option<greptimedb_ingester::database::Database>,
    #[cfg(not(feature = "greptimedb"))]
    _placeholder: (),
}

impl GreptimeClient {
    /// Create a new client from configuration.
    /// Returns a no-op client if disabled or if the feature is not compiled.
    pub fn new(config: &GreptimeConfig) -> Self {
        #[cfg(feature = "greptimedb")]
        {
            if !config.enabled {
                return Self { inner: None };
            }
            let client = greptimedb_ingester::client::Client::with_urls(&[&config.endpoint]);
            let db =
                greptimedb_ingester::database::Database::new_with_dbname(&config.dbname, client);
            Self { inner: Some(db) }
        }
        #[cfg(not(feature = "greptimedb"))]
        {
            let _ = config;
            Self { _placeholder: () }
        }
    }

    /// Write a health entry. No-op when feature is disabled.
    pub async fn write_health(
        &self,
        repo_id: &str,
        entry: &crate::registry::HealthEntry,
    ) -> anyhow::Result<()> {
        #[cfg(feature = "greptimedb")]
        {
            if let Some(db) = &self.inner {
                use greptimedb_ingester::ColumnDataType;
                use greptimedb_ingester::api::v1::{
                    Row, RowInsertRequest, RowInsertRequests, Rows,
                };
                use greptimedb_ingester::helpers::schema::{field, tag, timestamp};
                use greptimedb_ingester::helpers::values::{
                    i64_value, string_value, timestamp_millisecond_value,
                };

                let schema = vec![
                    tag("repo_id", ColumnDataType::String),
                    timestamp("checked_at", ColumnDataType::TimestampMillisecond),
                    field("status", ColumnDataType::String),
                    field("ahead", ColumnDataType::Int64),
                    field("behind", ColumnDataType::Int64),
                ];

                let checked_at_ms = entry.checked_at.timestamp_millis();
                let rows = vec![Row {
                    values: vec![
                        string_value(repo_id.to_string()),
                        timestamp_millisecond_value(checked_at_ms),
                        string_value(entry.status.clone()),
                        i64_value(entry.ahead as i64),
                        i64_value(entry.behind as i64),
                    ],
                }];

                let req = RowInsertRequests {
                    inserts: vec![RowInsertRequest {
                        table_name: "health_metrics".to_string(),
                        rows: Some(Rows { schema, rows }),
                    }],
                };

                if let Err(e) = db.insert(req).await {
                    tracing::warn!("GreptimeDB write_health failed for {}: {}", repo_id, e);
                }
            }
        }
        let _ = repo_id;
        let _ = entry;
        Ok(())
    }

    /// Write code metrics. No-op when feature is disabled.
    pub async fn write_metrics(
        &self,
        _repo_id: &str,
        _metrics: &crate::registry::CodeMetrics,
    ) -> anyhow::Result<()> {
        #[cfg(feature = "greptimedb")]
        {
            // Phase C: convert CodeMetrics to GreptimeDB row batch.
        }
        Ok(())
    }

    /// Write stars snapshot. No-op when feature is disabled.
    pub async fn write_stars(&self, _repo_id: &str, _stars: u64) -> anyhow::Result<()> {
        #[cfg(feature = "greptimedb")]
        {
            // Phase C: convert stars to GreptimeDB row batch.
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::HealthEntry;
    use chrono::Utc;

    #[tokio::test]
    async fn test_write_health_noop_when_disabled() {
        let config = GreptimeConfig::default(); // enabled = false
        let client = GreptimeClient::new(&config);
        let entry = HealthEntry {
            status: "ok".to_string(),
            ahead: 0,
            behind: 0,
            checked_at: Utc::now(),
        };
        assert!(client.write_health("repo1", &entry).await.is_ok());
    }

    #[tokio::test]
    async fn test_write_stars_noop_when_disabled() {
        let config = GreptimeConfig::default();
        let client = GreptimeClient::new(&config);
        assert!(client.write_stars("repo1", 42).await.is_ok());
    }
}
