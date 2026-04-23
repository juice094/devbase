pub mod asyncgit;
pub mod backup;
pub mod config;
pub mod core;
pub mod daemon;
pub mod digest;
pub mod discovery_engine;
pub mod health;
pub mod i18n;
pub mod knowledge_engine;
pub mod mcp;
pub mod query;
pub mod registry;
pub mod scan;
pub mod search;
pub mod sync;
pub mod sync_protocol;
pub mod syncthing_client;
pub mod tui;
pub mod vault;
pub mod watch;

#[cfg(test)]
pub mod test_utils;
