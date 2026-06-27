// SPDX-License-Identifier: MIT
// Copyright (c) 2026 juice094
//! MCP tools for memory intelligence — typed knowledge-graph relationships,
//! deduplication, quality scoring, decay, and merging.
//!
//! Phase 1: Memory Graph Foundation (v0.21.0)

use crate::clients::MemoryClient;
use crate::mcp::McpTool;
use anyhow::Context;

// ── devkit_memory_link ──

#[derive(Clone)]
pub struct DevkitMemoryLinkTool;

impl McpTool for DevkitMemoryLinkTool {
    fn name(&self) -> &'static str {
        "devkit_memory_link"
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "description": r#"Create a typed relationship edge between two memories (knowledge graph).

Use this when the user wants to:
- Mark a newer memory as superseding an older one (SUPERSEDES)
- Record that a bug fix was caused by a specific error (CAUSED_BY)
- Link related concepts across memory entries (RELATES_TO)
- Document that two memories contradict each other (CONTRADICTS)
- Note that an implementation detail refines an architectural decision (REFINES)
- Record that one memory depends on another for full context (DEPENDS_ON)

Supported relation types:
- SUPERSEDES: newer memory replaces an older one
- DEPENDS_ON: this memory depends on another for context
- CAUSED_BY: error/fix was caused by a previous event
- RELATES_TO: general conceptual link
- CONTRADICTS: two memories disagree
- GENERALIZES: abstract memory generalizes a specific one
- REFINES: specific memory refines a general one
- IMPLEMENTS: concrete implementation of an abstract decision

Do NOT use this for:
- Linking entities other than memories (use devkit_relation_store instead)
- Storing the original memory content (use devkit_session_save instead)

Parameters:
- from_memory_id: Source memory ID (required)
- to_memory_id: Target memory ID (required)
- relation_type: One of SUPERSEDES, DEPENDS_ON, CAUSED_BY, RELATES_TO, CONTRADICTS, GENERALIZES, REFINES, IMPLEMENTS (required)
- confidence: Confidence score 0.0–1.0 (default 1.0)
- evidence: Optional reason/evidence for the relationship

Returns: The created relation ID and details. Idempotent: re-linking the same pair with the same type overwrites."#,
            "inputSchema": {
                "type": "object",
                "properties": {
                    "from_memory_id": { "type": "integer", "description": "Source memory ID" },
                    "to_memory_id": { "type": "integer", "description": "Target memory ID" },
                    "relation_type": {
                        "type": "string",
                        "description": "Relationship type",
                        "enum": ["SUPERSEDES", "DEPENDS_ON", "CAUSED_BY", "RELATES_TO", "CONTRADICTS", "GENERALIZES", "REFINES", "IMPLEMENTS"]
                    },
                    "confidence": { "type": "number", "description": "Confidence score 0.0–1.0 (default 1.0)", "minimum": 0.0, "maximum": 1.0 },
                    "evidence": { "type": "string", "description": "Optional reason/evidence for the relationship" }
                },
                "required": ["from_memory_id", "to_memory_id", "relation_type"]
            }
        })
    }

    async fn invoke(
        &self,
        args: serde_json::Value,
        ctx: &mut crate::storage::AppContext,
    ) -> anyhow::Result<serde_json::Value> {
        let from_id = args
            .get("from_memory_id")
            .and_then(|v| v.as_i64())
            .context("from_memory_id (integer) is required")?;
        let to_id = args
            .get("to_memory_id")
            .and_then(|v| v.as_i64())
            .context("to_memory_id (integer) is required")?;
        let relation_type = args
            .get("relation_type")
            .and_then(|v| v.as_str())
            .context("relation_type is required")?
            .trim();
        let confidence = args.get("confidence").and_then(|v| v.as_f64()).unwrap_or(1.0);
        let evidence = args.get("evidence").and_then(|v| v.as_str());

        if relation_type.is_empty() {
            anyhow::bail!("relation_type must not be empty");
        }
        if from_id == to_id {
            anyhow::bail!("self-relations are not allowed");
        }

        let valid_types = [
            "SUPERSEDES",
            "DEPENDS_ON",
            "CAUSED_BY",
            "RELATES_TO",
            "CONTRADICTS",
            "GENERALIZES",
            "REFINES",
            "IMPLEMENTS",
        ];
        if !valid_types.contains(&relation_type) {
            anyhow::bail!(
                "Invalid relation_type '{}'. Must be one of: {}",
                relation_type,
                valid_types.join(", ")
            );
        }

        ctx.link_memories(from_id, to_id, relation_type, confidence, evidence)
    }
}

// ── devkit_memory_related ──

#[derive(Clone)]
pub struct DevkitMemoryRelatedTool;

impl McpTool for DevkitMemoryRelatedTool {
    fn name(&self) -> &'static str {
        "devkit_memory_related"
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "description": r#"Query memories related to a given memory by traversing typed relationship edges.

Use this when the user wants to:
- Find what a memory supersedes or is superseded by
- Trace causal chains (CAUSED_BY edges)
- Discover related concepts (RELATES_TO edges)
- Find refinements or generalizations of a decision

Do NOT use this for:
- Full-text keyword search across all memories (use devkit_session_search instead)
- Semantic similarity search (use devkit_session_recall with embedding instead)
- Entity-to-entity relations (use devkit_relation_query instead)

Parameters:
- memory_id: The root memory ID to query from (required)
- direction: "outgoing" (from this memory), "incoming" (to this memory), or "both" (default: "both")
- relation_type: Optional filter by relation type
- limit: Maximum results (default 50, max 500)

Returns: Array of memory relation objects with IDs, types, confidence, and evidence."#,
            "inputSchema": {
                "type": "object",
                "properties": {
                    "memory_id": { "type": "integer", "description": "Root memory ID to query from" },
                    "direction": {
                        "type": "string",
                        "description": "outgoing | incoming | both (default: both)",
                        "enum": ["outgoing", "incoming", "both"]
                    },
                    "relation_type": {
                        "type": "string",
                        "description": "Optional filter by relation type",
                        "enum": ["SUPERSEDES", "DEPENDS_ON", "CAUSED_BY", "RELATES_TO", "CONTRADICTS", "GENERALIZES", "REFINES", "IMPLEMENTS"]
                    },
                    "limit": { "type": "integer", "description": "Maximum results (default 50, max 500)" }
                },
                "required": ["memory_id"]
            }
        })
    }

    async fn invoke(
        &self,
        args: serde_json::Value,
        ctx: &mut crate::storage::AppContext,
    ) -> anyhow::Result<serde_json::Value> {
        let memory_id = args
            .get("memory_id")
            .and_then(|v| v.as_i64())
            .context("memory_id (integer) is required")?;
        let direction = args.get("direction").and_then(|v| v.as_str()).unwrap_or("both");
        let relation_type = args.get("relation_type").and_then(|v| v.as_str());
        let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(50).min(500) as usize;

        ctx.query_memory_relations(memory_id, direction, relation_type, limit)
    }
}

// ── devkit_memory_graph ──

#[derive(Clone)]
pub struct DevkitMemoryGraphTool;

impl McpTool for DevkitMemoryGraphTool {
    fn name(&self) -> &'static str {
        "devkit_memory_graph"
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "description": r#"Build and return a BFS-traversed memory sub-graph rooted at a given memory.

Use this when the user wants to:
- Visualize how memories connect in the knowledge graph
- Trace decision chains (IMPLEMENTS → REFINES chain)
- Understand the context around a specific memory
- Export the memory graph for visualization

Do NOT use this for:
- Simple relation queries (use devkit_memory_related instead)
- Searching for memories by content (use devkit_session_search instead)

Parameters:
- root_memory_id: The starting memory ID for BFS traversal (required)
- depth: Maximum traversal depth 1–3 (default 2, max 5)

Returns: Graph with nodes (memory_id, depth, type, preview, importance) and edges (relation_id, from, to, type, confidence)."#,
            "inputSchema": {
                "type": "object",
                "properties": {
                    "root_memory_id": { "type": "integer", "description": "Starting memory ID for BFS" },
                    "depth": { "type": "integer", "description": "Traversal depth 1-3 (default 2, max 5)" }
                },
                "required": ["root_memory_id"]
            }
        })
    }

    async fn invoke(
        &self,
        args: serde_json::Value,
        ctx: &mut crate::storage::AppContext,
    ) -> anyhow::Result<serde_json::Value> {
        let root_id = args
            .get("root_memory_id")
            .and_then(|v| v.as_i64())
            .context("root_memory_id (integer) is required")?;
        let depth = args.get("depth").and_then(|v| v.as_u64()).unwrap_or(2).min(5) as u32;

        ctx.build_memory_graph(root_id, depth)
    }
}

// ── devkit_memory_dedup ──

#[derive(Clone)]
pub struct DevkitMemoryDedupTool;

impl McpTool for DevkitMemoryDedupTool {
    fn name(&self) -> &'static str {
        "devkit_memory_dedup"
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "description": r#"Detect duplicate or near-duplicate memories within a context using vector similarity.

Use this when the user wants to:
- Find redundant memories that should be merged
- Clean up a context before archiving or exporting
- Validate that a new memory doesn't already exist

Do NOT use this for:
- Semantic search for related concepts (use devkit_session_recall instead)
- Finding contradictory memories (use devkit_memory_related with CONTRADICTS type instead)

Parameters:
- context_id: The session/context ID to scan (required)
- threshold: Cosine similarity threshold 0.0–1.0 (default 0.85). Higher = stricter dedup.

Returns: Pairs of potentially duplicate memories with similarity scores. Only checks memories that have embeddings."#,
            "inputSchema": {
                "type": "object",
                "properties": {
                    "context_id": { "type": "string", "description": "Session/context ID to scan" },
                    "threshold": { "type": "number", "description": "Similarity threshold 0.0–1.0 (default 0.85)", "minimum": 0.0, "maximum": 1.0 }
                },
                "required": ["context_id"]
            }
        })
    }

    async fn invoke(
        &self,
        args: serde_json::Value,
        ctx: &mut crate::storage::AppContext,
    ) -> anyhow::Result<serde_json::Value> {
        let context_id = args
            .get("context_id")
            .and_then(|v| v.as_str())
            .context("context_id is required")?
            .trim()
            .to_string();
        let threshold = args.get("threshold").and_then(|v| v.as_f64()).unwrap_or(0.85) as f32;

        if context_id.is_empty() {
            anyhow::bail!("context_id must not be empty");
        }

        ctx.dedup_memories(&context_id, threshold)
    }
}

// ── devkit_memory_quality ──

#[derive(Clone)]
pub struct DevkitMemoryQualityTool;

impl McpTool for DevkitMemoryQualityTool {
    fn name(&self) -> &'static str {
        "devkit_memory_quality"
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "description": r#"Set or update the quality score of a memory.

Use this when the user wants to:
- Mark a memory as high-quality (well-written, accurate, reusable)
- Flag a memory as low-quality (outdated, vague, incorrect)
- Adjust quality scores as part of memory curation

Do NOT use this for:
- Adjusting importance (use devkit_memory_link and adjust confidence instead, or devkit_memory_decay to let the system compute importance)
- Deleting memories (use devkit_memory_decay or devkit_memory_merge with supersede strategy)

Parameters:
- memory_id: The memory ID to score (required)
- score: Quality score 0.0–1.0 (required). 0.8+ = high quality, 0.3–0.7 = average, <0.3 = low quality.

Returns: success boolean and the updated score."#,
            "inputSchema": {
                "type": "object",
                "properties": {
                    "memory_id": { "type": "integer", "description": "Memory ID to score" },
                    "score": { "type": "number", "description": "Quality score 0.0–1.0", "minimum": 0.0, "maximum": 1.0 }
                },
                "required": ["memory_id", "score"]
            }
        })
    }

    async fn invoke(
        &self,
        args: serde_json::Value,
        ctx: &mut crate::storage::AppContext,
    ) -> anyhow::Result<serde_json::Value> {
        let memory_id = args
            .get("memory_id")
            .and_then(|v| v.as_i64())
            .context("memory_id (integer) is required")?;
        let score = args
            .get("score")
            .and_then(|v| v.as_f64())
            .context("score (0.0–1.0) is required")?;

        if !(0.0..=1.0).contains(&score) {
            anyhow::bail!("score must be between 0.0 and 1.0");
        }

        let conn = ctx.conn()?;
        crate::registry::agent_context::update_memory_quality(&conn, memory_id, score)?;
        Ok(serde_json::json!({ "success": true, "memory_id": memory_id, "quality_score": score }))
    }
}

// ── devkit_memory_decay ──

#[derive(Clone)]
pub struct DevkitMemoryDecayTool;

impl McpTool for DevkitMemoryDecayTool {
    fn name(&self) -> &'static str {
        "devkit_memory_decay"
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "description": r#"Apply memory decay policy to a context's memories.

Archives low-importance, stale memories based on:
- importance weight (lower = more likely to decay)
- decay_factor (higher = faster decay rate)
- days since last access (longer = more stale)

Formula: archived if importance × (1.0 - days_stale × decay_factor) < 0.1
Memories with decay_factor = 0 are never decayed. Memories with importance ≥ 0.5 are never decayed.

Use this when the user wants to:
- Clean up old, unused session memories
- Run periodic maintenance on the memory store
- Free up context space for more relevant memories

Do NOT use this for:
- Deleting specific memories (use devkit_memory_merge with merge strategy)
- Finding duplicates (use devkit_memory_dedup instead)

Parameters:
- context_id: The session/context ID to apply decay to (required)

Returns: Number and IDs of archived memories."#,
            "inputSchema": {
                "type": "object",
                "properties": {
                    "context_id": { "type": "string", "description": "Session/context ID" }
                },
                "required": ["context_id"]
            }
        })
    }

    async fn invoke(
        &self,
        args: serde_json::Value,
        ctx: &mut crate::storage::AppContext,
    ) -> anyhow::Result<serde_json::Value> {
        let context_id = args
            .get("context_id")
            .and_then(|v| v.as_str())
            .context("context_id is required")?
            .trim()
            .to_string();

        if context_id.is_empty() {
            anyhow::bail!("context_id must not be empty");
        }

        ctx.apply_memory_decay(&context_id)
    }
}

// ── devkit_memory_merge ──

#[derive(Clone)]
pub struct DevkitMemoryMergeTool;

impl McpTool for DevkitMemoryMergeTool {
    fn name(&self) -> &'static str {
        "devkit_memory_merge"
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "description": r#"Merge two similar memories using a specified strategy.

Strategies:
- "supersede": Primary replaces secondary. Secondary is archived and linked via SUPERSEDES edge.
- "merge_content": Primary absorbs secondary's content (appended). Secondary archived, SUPERSEDES link created.
- "keep_both": Both kept active, linked via RELATES_TO edge. No archival.

Use this when the user wants to:
- Consolidate duplicate memories found by devkit_memory_dedup
- Combine related knowledge into a single, richer memory
- Clean up memory store by resolving redundancies

Do NOT use this for:
- Linking unrelated memories (use devkit_memory_link instead)
- Deleting a memory without preserving its knowledge (use devkit_memory_decay for archival-first)

Parameters:
- primary_id: The memory to keep (required)
- secondary_id: The memory to merge into primary or archive (required)
- strategy: "supersede" | "merge_content" | "keep_both" (required)

Returns: success boolean, IDs, and strategy applied."#,
            "inputSchema": {
                "type": "object",
                "properties": {
                    "primary_id": { "type": "integer", "description": "Memory to keep" },
                    "secondary_id": { "type": "integer", "description": "Memory to merge/archive" },
                    "strategy": {
                        "type": "string",
                        "description": "Merge strategy",
                        "enum": ["supersede", "merge_content", "keep_both"]
                    }
                },
                "required": ["primary_id", "secondary_id", "strategy"]
            }
        })
    }

    async fn invoke(
        &self,
        args: serde_json::Value,
        ctx: &mut crate::storage::AppContext,
    ) -> anyhow::Result<serde_json::Value> {
        let primary_id = args
            .get("primary_id")
            .and_then(|v| v.as_i64())
            .context("primary_id (integer) is required")?;
        let secondary_id = args
            .get("secondary_id")
            .and_then(|v| v.as_i64())
            .context("secondary_id (integer) is required")?;
        let strategy = args
            .get("strategy")
            .and_then(|v| v.as_str())
            .context("strategy is required")?
            .trim()
            .to_string();

        if primary_id == secondary_id {
            anyhow::bail!("primary_id and secondary_id must be different");
        }

        let valid_strategies = ["supersede", "merge_content", "keep_both"];
        if !valid_strategies.contains(&strategy.as_str()) {
            anyhow::bail!(
                "Invalid strategy '{}'. Must be one of: {}",
                strategy,
                valid_strategies.join(", ")
            );
        }

        ctx.merge_memories(primary_id, secondary_id, &strategy)
    }
}

// ── devkit_memory_stats ──

#[derive(Clone)]
pub struct DevkitMemoryStatsTool;

impl McpTool for DevkitMemoryStatsTool {
    fn name(&self) -> &'static str {
        "devkit_memory_stats"
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "description": r#"Return memory statistics for a context including counts, token estimates, and quality metrics.

Use this when the user wants to:
- Check the size and health of a memory store
- Decide whether to archive or curate memories
- Monitor memory growth over time
- Report memory statistics in a dashboard

Do NOT use this for:
- Retrieving specific memories (use devkit_session_search or devkit_session_list)
- Modifying memories (use devkit_memory_quality or devkit_memory_merge)

Parameters:
- context_id: The session/context ID (required)

Returns: total_count, archived_count, total_tokens_estimate, avg_quality, avg_importance."#,
            "inputSchema": {
                "type": "object",
                "properties": {
                    "context_id": { "type": "string", "description": "Session/context ID" }
                },
                "required": ["context_id"]
            }
        })
    }

    async fn invoke(
        &self,
        args: serde_json::Value,
        ctx: &mut crate::storage::AppContext,
    ) -> anyhow::Result<serde_json::Value> {
        let context_id = args
            .get("context_id")
            .and_then(|v| v.as_str())
            .context("context_id is required")?
            .trim()
            .to_string();

        if context_id.is_empty() {
            anyhow::bail!("context_id must not be empty");
        }

        ctx.memory_stats(&context_id)
    }
}
