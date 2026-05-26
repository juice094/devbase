// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (c) 2026 juice094
//! Unified SQLite registry operations for devbase.
//!
//! Submodules cover entity management, health tracking, code metrics,
//! call graph queries, code symbol indexing, dead-code analysis,
//! workspace snapshots, and entity relations.

pub mod call_graph;
pub mod code_symbols;
pub mod dead_code;
pub mod entity;
pub mod health;
pub mod metrics;
pub mod relation;
pub mod workspace;
