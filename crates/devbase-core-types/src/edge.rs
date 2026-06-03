// SPDX-License-Identifier: MIT
// Copyright (c) 2026 juice094

/// A directed edge in the knowledge graph.
#[derive(Debug, Clone)]
pub struct Edge {
    pub from_id: String,
    pub to_id: String,
    pub edge_type: String,
    pub metadata: Option<String>,
}
