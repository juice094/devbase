//! Repository layer — encapsulates all persistence logic for the AI-Native Storage Engine.
//!
//! Design principles:
//! - Synchronous (rusqlite::Connection is not Send).
//! - Returns raw domain structs; MCP tools handle JSON serialization.
//! - No direct HTTP calls; external APIs stay in mcp/tools.

pub mod dependency;
pub mod health;
pub mod knowledge;
pub mod repo;
pub mod search;
pub mod symbol;
pub mod workspace;

/// Base trait shared by all repositories.
pub trait Repository {
    fn conn(&self) -> &rusqlite::Connection;
}
