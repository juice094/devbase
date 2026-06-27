// SPDX-License-Identifier: MIT
// Copyright (c) 2026 juice094
//! MCP tools for Agent Context management (P1: Claude Projects inspired sessions).

use crate::mcp::McpTool;
use crate::storage::AppContext;
use anyhow::Context;
use serde_json::json;

#[derive(Clone)]
pub struct DevkitSessionSaveTool;

impl McpTool for DevkitSessionSaveTool {
    fn name(&self) -> &'static str {
        "devkit_session_save"
    }

    fn schema(&self) -> serde_json::Value {
        json!({
            "description": r#"Save or update an AI agent session context with optional memories.

Use this when the user wants to:
- Create a new persistent project context
- Update an existing session's name, intent, or append memories
- Checkpoint current conversation state for later resumption

Parameters:
- context_id: Unique session identifier (e.g., "project-alpha", "sprint-29").
- name: Human-readable session name.
- intent: Optional high-level goal or project description.
- memories: Optional array of {type, content} objects to append. Types: decision, constraint, note, discovery, error."#,
            "inputSchema": {
                "type": "object",
                "properties": {
                    "context_id": { "type": "string", "description": "Unique session ID" },
                    "name": { "type": "string", "description": "Human-readable name" },
                    "intent": { "type": "string", "description": "High-level goal / project description" },
                    "memories": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "type": { "type": "string", "description": "Memory type: decision, constraint, note, discovery, error" },
                                "content": { "type": "string", "description": "Memory content" }
                            },
                            "required": ["type", "content"]
                        }
                    }
                },
                "required": ["context_id", "name"]
            }
        })
    }

    async fn invoke(
        &self,
        args: serde_json::Value,
        ctx: &mut AppContext,
    ) -> anyhow::Result<serde_json::Value> {
        let context_id = args.get("context_id").and_then(|v| v.as_str()).unwrap_or("");
        let name = args.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let intent = args.get("intent").and_then(|v| v.as_str());
        if context_id.is_empty() {
            anyhow::bail!("Missing required argument: context_id");
        }
        if name.is_empty() {
            anyhow::bail!("Missing required argument: name");
        }

        let mut conn = ctx.conn_mut()?;
        crate::registry::agent_context::upsert_context(&mut conn, context_id, name, intent)?;

        let mut memory_count = 0;
        if let Some(memories) = args.get("memories").and_then(|v| v.as_array()) {
            for mem in memories {
                let ty = mem.get("type").and_then(|v| v.as_str()).unwrap_or("note");
                let content = mem.get("content").and_then(|v| v.as_str()).unwrap_or("");
                if !content.is_empty() {
                    crate::registry::agent_context::insert_memory(
                        &mut conn, context_id, ty, content, None, None,
                    )?;
                    memory_count += 1;
                }
            }
        }

        Ok(json!({
            "success": true,
            "context_id": context_id,
            "name": name,
            "memories_added": memory_count
        }))
    }
}

#[derive(Clone)]
pub struct DevkitSessionListTool;

impl McpTool for DevkitSessionListTool {
    fn name(&self) -> &'static str {
        "devkit_session_list"
    }

    fn schema(&self) -> serde_json::Value {
        json!({
            "description": r#"List persisted AI agent sessions (contexts).

Use this when the user wants to:
- See all active or archived sessions
- Find a session to resume
- Audit past project contexts"#,
            "inputSchema": {
                "type": "object",
                "properties": {
                    "status_filter": {
                        "type": "string",
                        "enum": ["active", "archived"],
                        "description": "Filter by status. Omit for all."
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum results",
                        "default": 50
                    }
                }
            }
        })
    }

    async fn invoke(
        &self,
        args: serde_json::Value,
        ctx: &mut AppContext,
    ) -> anyhow::Result<serde_json::Value> {
        let status_filter = args.get("status_filter").and_then(|v| v.as_str());
        let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(50) as usize;

        let conn = ctx.conn()?;
        let contexts = crate::registry::agent_context::list_contexts(&conn)?;
        let results: Vec<serde_json::Value> = contexts
            .into_iter()
            .filter(|c| status_filter.is_none_or(|f| c.status == f))
            .take(limit)
            .map(|c| {
                json!({
                    "context_id": c.id,
                    "name": c.name,
                    "intent": c.intent,
                    "status": c.status,
                    "updated_at": c.updated_at.to_rfc3339(),
                })
            })
            .collect();

        Ok(json!({
            "success": true,
            "count": results.len(),
            "contexts": results
        }))
    }
}

#[derive(Clone)]
pub struct DevkitSessionResumeTool;

impl McpTool for DevkitSessionResumeTool {
    fn name(&self) -> &'static str {
        "devkit_session_resume"
    }

    fn schema(&self) -> serde_json::Value {
        json!({
            "description": r#"Resume a persisted AI agent session, returning its metadata and memories.

Use this when the user wants to:
- Restore a previous project context
- Continue work from a checkpointed session
- Review all decisions and constraints stored in a session"#,
            "inputSchema": {
                "type": "object",
                "properties": {
                    "context_id": {
                        "type": "string",
                        "description": "Session ID to resume"
                    },
                    "include_memories": {
                        "type": "boolean",
                        "description": "Include associated memories",
                        "default": true
                    },
                    "memory_types": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Optional filter for memory types (e.g. [\"decision\", \"constraint\"])"
                    }
                },
                "required": ["context_id"]
            }
        })
    }

    async fn invoke(
        &self,
        args: serde_json::Value,
        ctx: &mut AppContext,
    ) -> anyhow::Result<serde_json::Value> {
        let context_id = args.get("context_id").and_then(|v| v.as_str()).unwrap_or("");
        if context_id.is_empty() {
            anyhow::bail!("Missing required argument: context_id");
        }
        let include_memories =
            args.get("include_memories").and_then(|v| v.as_bool()).unwrap_or(true);
        let memory_types: Option<Vec<String>> = args
            .get("memory_types")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect());

        let conn = ctx.conn()?;
        match crate::registry::agent_context::get_context_with_memories(&conn, context_id)? {
            Some((ctx_info, mut memories)) => {
                if let Some(types) = memory_types {
                    let type_set: std::collections::HashSet<String> = types.into_iter().collect();
                    memories.retain(|m| type_set.contains(&m.memory_type));
                }
                let memory_json: Vec<serde_json::Value> = if include_memories {
                    memories
                        .into_iter()
                        .map(|m| {
                            json!({
                                "id": m.id,
                                "type": m.memory_type,
                                "content": m.content,
                                "created_at": m.created_at.to_rfc3339(),
                            })
                        })
                        .collect()
                } else {
                    vec![]
                };

                let linked =
                    crate::registry::agent_context::list_linked_entities(&conn, context_id)?;
                let linked_json: Vec<serde_json::Value> = linked
                    .into_iter()
                    .map(|(eid, ltype, _cat)| {
                        json!({
                            "entity_id": eid,
                            "link_type": ltype,
                        })
                    })
                    .collect();

                Ok(json!({
                    "success": true,
                    "context": {
                        "context_id": ctx_info.id,
                        "name": ctx_info.name,
                        "intent": ctx_info.intent,
                        "status": ctx_info.status,
                        "created_at": ctx_info.created_at.to_rfc3339(),
                        "updated_at": ctx_info.updated_at.to_rfc3339(),
                    },
                    "memories": memory_json,
                    "memory_count": memory_json.len(),
                    "linked_entities": linked_json,
                    "linked_count": linked_json.len(),
                }))
            }
            None => anyhow::bail!("Session '{}' not found", context_id),
        }
    }
}

#[derive(Clone)]
pub struct DevkitSessionAttachTool;

impl McpTool for DevkitSessionAttachTool {
    fn name(&self) -> &'static str {
        "devkit_session_attach"
    }

    fn schema(&self) -> serde_json::Value {
        json!({
            "description": r#"Attach an entity (repo, vault note, skill, etc.) to an agent session.

Use this when the user wants to:
- Link a repository to a project session
- Associate a skill or vault note with the current context
- Build a project workspace by connecting relevant resources"#,
            "inputSchema": {
                "type": "object",
                "properties": {
                    "context_id": { "type": "string", "description": "Session ID" },
                    "entity_id": { "type": "string", "description": "Entity ID (repo_id, vault path, skill_id, etc.)" },
                    "link_type": {
                        "type": "string",
                        "enum": ["linked_repo", "linked_vault", "linked_skill", "linked_paper", "linked"],
                        "default": "linked",
                        "description": "Type of relationship"
                    }
                },
                "required": ["context_id", "entity_id"]
            }
        })
    }

    async fn invoke(
        &self,
        args: serde_json::Value,
        ctx: &mut AppContext,
    ) -> anyhow::Result<serde_json::Value> {
        let context_id = args.get("context_id").and_then(|v| v.as_str()).unwrap_or("");
        let entity_id = args.get("entity_id").and_then(|v| v.as_str()).unwrap_or("");
        let link_type = args.get("link_type").and_then(|v| v.as_str()).unwrap_or("linked");
        if context_id.is_empty() {
            anyhow::bail!("Missing required argument: context_id");
        }
        if entity_id.is_empty() {
            anyhow::bail!("Missing required argument: entity_id");
        }

        let mut conn = ctx.conn_mut()?;
        crate::registry::agent_context::attach_entity(&mut conn, context_id, entity_id, link_type)?;
        Ok(json!({
            "success": true,
            "context_id": context_id,
            "entity_id": entity_id,
            "link_type": link_type,
        }))
    }
}

#[derive(Clone)]
pub struct DevkitSessionDetachTool;

impl McpTool for DevkitSessionDetachTool {
    fn name(&self) -> &'static str {
        "devkit_session_detach"
    }

    fn schema(&self) -> serde_json::Value {
        json!({
            "description": r#"Detach an entity from an agent session.

Use this when the user wants to:
- Remove a stale repository link
- Unlink a skill that is no longer relevant to the project"#,
            "inputSchema": {
                "type": "object",
                "properties": {
                    "context_id": { "type": "string", "description": "Session ID" },
                    "entity_id": { "type": "string", "description": "Entity ID to remove" },
                    "link_type": {
                        "type": "string",
                        "description": "Specific link type to remove. Omit to remove all links to this entity."
                    }
                },
                "required": ["context_id", "entity_id"]
            }
        })
    }

    async fn invoke(
        &self,
        args: serde_json::Value,
        ctx: &mut AppContext,
    ) -> anyhow::Result<serde_json::Value> {
        let context_id = args.get("context_id").and_then(|v| v.as_str()).unwrap_or("");
        let entity_id = args.get("entity_id").and_then(|v| v.as_str()).unwrap_or("");
        let link_type = args.get("link_type").and_then(|v| v.as_str());
        if context_id.is_empty() {
            anyhow::bail!("Missing required argument: context_id");
        }
        if entity_id.is_empty() {
            anyhow::bail!("Missing required argument: entity_id");
        }

        let mut conn = ctx.conn_mut()?;
        let removed = crate::registry::agent_context::detach_entity(
            &mut conn, context_id, entity_id, link_type,
        )?;
        Ok(json!({
            "success": true,
            "removed": removed,
            "context_id": context_id,
            "entity_id": entity_id,
        }))
    }
}

#[derive(Clone)]
pub struct DevkitSessionActivateTool;

impl McpTool for DevkitSessionActivateTool {
    fn name(&self) -> &'static str {
        "devkit_session_activate"
    }

    fn schema(&self) -> serde_json::Value {
        json!({
            "description": r#"Activate a session so that subsequent skill executions automatically receive its memories and linked entities.

Use this when the user wants to:
- Set a default project context for the current workspace
- Make all future skill runs context-aware without manual memory passing
- Switch between projects"#,
            "inputSchema": {
                "type": "object",
                "properties": {
                    "context_id": { "type": "string", "description": "Session ID to activate" }
                },
                "required": ["context_id"]
            }
        })
    }

    async fn invoke(
        &self,
        args: serde_json::Value,
        _ctx: &mut AppContext,
    ) -> anyhow::Result<serde_json::Value> {
        let context_id = args.get("context_id").and_then(|v| v.as_str()).unwrap_or("");
        if context_id.is_empty() {
            anyhow::bail!("Missing required argument: context_id");
        }

        let state_file =
            crate::registry::WorkspaceRegistry::workspace_dir()?.join(".active_context");
        std::fs::write(&state_file, context_id)?;

        Ok(json!({
            "success": true,
            "context_id": context_id,
            "state_file": state_file.to_string_lossy().to_string(),
            "tip": format!("Set DEVBASE_ACTIVE_CONTEXT={} in your environment to make this persistent across shell sessions.", context_id),
        }))
    }
}

#[derive(Clone)]
pub struct DevkitSessionSearchTool;

impl McpTool for DevkitSessionSearchTool {
    fn name(&self) -> &'static str {
        "devkit_session_search"
    }

    fn schema(&self) -> serde_json::Value {
        json!({
            "description": r#"Search memories by keyword across all sessions or within a specific session.

Use this when the user wants to:
- Find a past decision or constraint mentioned in memories
- Recall what was discussed in a previous project session
- Audit all sessions for a specific topic"#,
            "inputSchema": {
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Keyword to search for" },
                    "context_id": {
                        "type": "string",
                        "description": "Restrict search to a specific session. Omit for global search."
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum results",
                        "default": 20
                    }
                },
                "required": ["query"]
            }
        })
    }

    async fn invoke(
        &self,
        args: serde_json::Value,
        ctx: &mut AppContext,
    ) -> anyhow::Result<serde_json::Value> {
        let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("");
        let context_id = args.get("context_id").and_then(|v| v.as_str());
        let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(20) as usize;
        if query.is_empty() {
            anyhow::bail!("Missing required argument: query");
        }

        let conn = ctx.conn()?;
        let memories =
            crate::registry::agent_context::search_memories(&conn, context_id, query, limit)?;
        let results: Vec<serde_json::Value> = memories
            .into_iter()
            .map(|m| {
                json!({
                    "id": m.id,
                    "context_id": m.context_id,
                    "type": m.memory_type,
                    "content": m.content,
                    "created_at": m.created_at.to_rfc3339(),
                })
            })
            .collect();

        Ok(json!({
            "success": true,
            "query": query,
            "count": results.len(),
            "memories": results,
        }))
    }
}

#[derive(Clone)]
pub struct DevkitSessionCaptureTool;

impl McpTool for DevkitSessionCaptureTool {
    fn name(&self) -> &'static str {
        "devkit_session_capture"
    }

    fn schema(&self) -> serde_json::Value {
        json!({
            "description": r#"Capture a decision, constraint, or observation into the active session's memory.

Use this when the AI (or user) wants to:
- Record an architectural decision made during the conversation
- Save a constraint discovered while debugging
- Checkpoint a key insight before moving to another topic

This is a lightweight append-only operation. No validation is performed on content."#,
            "inputSchema": {
                "type": "object",
                "properties": {
                    "context_id": {
                        "type": "string",
                        "description": "Session ID. Omit to use the currently activated session (via devkit_session_activate)."
                    },
                    "type": {
                        "type": "string",
                        "enum": ["decision", "constraint", "note", "discovery", "error", "action"],
                        "default": "note",
                        "description": "Memory classification"
                    },
                    "content": { "type": "string", "description": "Memory content" }
                },
                "required": ["content"]
            }
        })
    }

    async fn invoke(
        &self,
        args: serde_json::Value,
        ctx: &mut AppContext,
    ) -> anyhow::Result<serde_json::Value> {
        let content = args.get("content").and_then(|v| v.as_str()).unwrap_or("");
        if content.is_empty() {
            anyhow::bail!("Missing required argument: content");
        }
        let memory_type = args.get("type").and_then(|v| v.as_str()).unwrap_or("note");

        let context_id = match args.get("context_id").and_then(|v| v.as_str()) {
            Some(cid) if !cid.is_empty() => cid.to_string(),
            _ => {
                // Fallback: read activated session from state file
                let state_file =
                    crate::registry::WorkspaceRegistry::workspace_dir()?.join(".active_context");
                std::fs::read_to_string(state_file)
                    .ok()
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .ok_or_else(|| anyhow::anyhow!("No active session. Use context_id argument or devkit_session_activate first."))?
            }
        };

        let mut conn = ctx.conn_mut()?;
        let id = crate::registry::agent_context::insert_memory(
            &mut conn,
            &context_id,
            memory_type,
            content,
            None,
            None,
        )?;

        Ok(json!({
            "success": true,
            "memory_id": id,
            "context_id": context_id,
            "type": memory_type,
        }))
    }
}

#[derive(Clone)]
pub struct DevkitSessionWorkflowsTool;

impl McpTool for DevkitSessionWorkflowsTool {
    fn name(&self) -> &'static str {
        "devkit_session_workflows"
    }

    fn schema(&self) -> serde_json::Value {
        json!({
            "description": r#"List workflow executions associated with an agent session.

Use this when the user wants to:
- Review what automated workflows were run in a project context
- Audit the execution history of a session
- Check workflow status for a specific project"#,
            "inputSchema": {
                "type": "object",
                "properties": {
                    "context_id": { "type": "string", "description": "Session ID" },
                    "limit": { "type": "integer", "description": "Maximum results", "default": 20 }
                },
                "required": ["context_id"]
            }
        })
    }

    async fn invoke(
        &self,
        args: serde_json::Value,
        ctx: &mut AppContext,
    ) -> anyhow::Result<serde_json::Value> {
        let context_id = args.get("context_id").and_then(|v| v.as_str()).unwrap_or("");
        let limit = args.get("limit").and_then(|v| v.as_i64()).unwrap_or(20);
        if context_id.is_empty() {
            anyhow::bail!("Missing required argument: context_id");
        }

        let conn = ctx.conn()?;
        let executions =
            crate::workflow::state::list_executions_by_context(&conn, context_id, limit)?;
        let results: Vec<serde_json::Value> = executions
            .into_iter()
            .map(|(id, wf_id, status, current_step, started_at, duration_ms)| {
                json!({
                    "execution_id": id,
                    "workflow_id": wf_id,
                    "status": status,
                    "current_step": current_step,
                    "started_at": started_at,
                    "duration_ms": duration_ms,
                })
            })
            .collect();

        Ok(json!({
            "success": true,
            "context_id": context_id,
            "count": results.len(),
            "executions": results,
        }))
    }
}

#[derive(Clone)]
pub struct DevkitSessionRecallTool;

impl McpTool for DevkitSessionRecallTool {
    fn name(&self) -> &'static str {
        "devkit_session_recall"
    }

    fn schema(&self) -> serde_json::Value {
        json!({
            "description": r#"Semantic memory recall for an active agent session. Supports both vector and text-based queries.

Use this when the user wants to:
- Find relevant past memories by meaning rather than exact keyword
- Surface decisions, constraints, or discoveries related to the current task
- Inject top-k relevant memories into the prompt context

Two query modes (choose one):
1. Vector mode: provide query_embedding (f32 array, externally generated or from devkit_embedding_search).
2. Text mode: provide query_text (plain string). devbase auto-generates the embedding internally when the 'embedding' feature is enabled, or falls back to keyword search.

Do NOT use this for:
- Keyword-only search over all memories (use devkit_session_search instead)
- Querying entity relations (use devkit_relation_query instead)

Parameters:
- context_id: Session ID (optional; falls back to DEVBASE_ACTIVE_CONTEXT).
- query_embedding: f32 array from an external embedding provider (optional if query_text is set).
- query_text: Plain text query string (optional if query_embedding is set). Auto-embedded when embedding feature enabled.
- limit: Maximum memories to return (default 5, max 20).
- max_tokens: Optional total token budget for returned content. Truncates longer memories.

Returns: memories sorted by relevance score (vector cosine similarity or keyword match score 0.0-1.0)."#,
            "inputSchema": {
                "type": "object",
                "properties": {
                    "context_id": { "type": "string", "description": "Session ID (optional; falls back to active context)" },
                    "query_embedding": {
                        "type": "array",
                        "items": { "type": "number" },
                        "description": "Query vector as f32 array (optional if query_text is set)"
                    },
                    "query_text": {
                        "type": "string",
                        "description": "Plain text query (optional if query_embedding is set). Internally embedded when feature enabled, or falls back to keyword search."
                    },
                    "limit": { "type": "integer", "default": 5, "description": "Maximum memories to return (default 5, max 20)" },
                    "max_tokens": { "type": "integer", "description": "Optional total token budget. Longer memories are truncated." }
                }
            }
        })
    }

    async fn invoke(
        &self,
        args: serde_json::Value,
        ctx: &mut AppContext,
    ) -> anyhow::Result<serde_json::Value> {
        let context_id = match args.get("context_id").and_then(|v| v.as_str()) {
            Some(cid) => cid.to_string(),
            None => crate::registry::agent_context::resolve_active_context().ok_or_else(|| {
                anyhow::anyhow!(
                    "No active session. Use context_id argument or devkit_session_activate first."
                )
            })?,
        };
        let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(5).min(20) as usize;
        let max_tokens = args.get("max_tokens").and_then(|v| v.as_u64());

        let conn = ctx.conn_mut()?;
        crate::registry::agent_context::register_vector_functions(&conn)?;

        // Determine query mode: vector vs text
        let results: Vec<(crate::registry::agent_context::AgentMemory, f64)> =
            if let Some(query_emb) = args.get("query_embedding").and_then(|v| v.as_array()) {
                // Vector mode
                let emb: Vec<f32> = query_emb
                    .iter()
                    .filter_map(|v| v.as_f64().map(|f| f as f32))
                    .collect();
                if emb.is_empty() {
                    anyhow::bail!("query_embedding must not be empty if provided");
                }
                crate::registry::agent_context::search_memories_semantic(
                    &conn, &context_id, &emb, limit,
                )?
            } else if let Some(query_text) = args.get("query_text").and_then(|v| v.as_str()) {
                if query_text.trim().is_empty() {
                    anyhow::bail!("query_text must not be empty if provided");
                }
                // Text mode: auto-embedding with keyword fallback
                crate::registry::agent_context::search_memories_by_text(
                    &conn, &context_id, query_text, limit,
                )?
            } else {
                anyhow::bail!("One of query_embedding or query_text is required");
            };

        let memories: Vec<serde_json::Value> = results
            .into_iter()
            .map(|(mem, score)| {
                let content = if let Some(max_tok) = max_tokens {
                    truncate_to_tokens(&mem.content, max_tok as usize)
                } else {
                    mem.content.clone()
                };
                json!({
                    "id": mem.id,
                    "type": mem.memory_type,
                    "content": content,
                    "created_at": mem.created_at.to_rfc3339(),
                    "embedding_model": mem.embedding_model,
                    "importance": mem.importance,
                    "quality_score": mem.quality_score,
                    "score": score,
                })
            })
            .collect();

        Ok(json!({
            "success": true,
            "context_id": context_id,
            "count": memories.len(),
            "memories": memories,
        }))
    }
}

#[derive(Clone)]
pub struct DevkitSessionIndexTool;

impl McpTool for DevkitSessionIndexTool {
    fn name(&self) -> &'static str {
        "devkit_session_index"
    }

    fn schema(&self) -> serde_json::Value {
        json!({
            "description": r#"Store an externally-generated embedding for an existing memory.

This is the storage-side of the memory semantic index: an external provider (Ollama, OpenAI, etc.) generates the embedding, and devbase persists it in SQLite for similarity search.

Parameters:
- memory_id: The integer ID of the memory to index.
- embedding: f32 vector from external provider.
- embedding_model: Name of the model used (e.g., "nomic-embed-text").

Returns: success flag."#,
            "inputSchema": {
                "type": "object",
                "properties": {
                    "memory_id": { "type": "integer", "description": "Memory row ID" },
                    "embedding": {
                        "type": "array",
                        "items": { "type": "number" },
                        "description": "Embedding vector as f32 array"
                    },
                    "embedding_model": { "type": "string", "description": "Model name used for generation" }
                },
                "required": ["memory_id", "embedding", "embedding_model"]
            }
        })
    }

    async fn invoke(
        &self,
        args: serde_json::Value,
        ctx: &mut AppContext,
    ) -> anyhow::Result<serde_json::Value> {
        let memory_id = args
            .get("memory_id")
            .and_then(|v| v.as_i64())
            .ok_or_else(|| anyhow::anyhow!("memory_id required"))?;
        let embedding = args
            .get("embedding")
            .and_then(|v| v.as_array())
            .ok_or_else(|| anyhow::anyhow!("embedding required"))?
            .iter()
            .filter_map(|v| v.as_f64().map(|f| f as f32))
            .collect::<Vec<f32>>();
        let embedding_model =
            args.get("embedding_model").and_then(|v| v.as_str()).unwrap_or("unknown");

        let embedding_blob = crate::registry::agent_context::embedding_to_blob(&embedding);
        let now = chrono::Utc::now().to_rfc3339();

        let conn = ctx.conn_mut()?;
        let rows = conn.execute(
            "UPDATE agent_memories SET embedding = ?1, embedding_model = ?2, indexed_at = ?3 WHERE id = ?4",
            rusqlite::params![embedding_blob, embedding_model, now, memory_id],
        )?;

        if rows == 0 {
            anyhow::bail!("Memory {} not found", memory_id);
        }

        Ok(json!({
            "success": true,
            "memory_id": memory_id,
            "embedding_model": embedding_model,
        }))
    }
}

#[derive(Clone)]
pub struct DevkitSessionExportTool;

impl McpTool for DevkitSessionExportTool {
    fn name(&self) -> &'static str {
        "devkit_session_export"
    }

    fn schema(&self) -> serde_json::Value {
        json!({
            "description": r#"Export an agent session (context + memories + links) to Markdown or JSON. Useful for sharing session state with ClaudeCode or other AI assistants, or for archival.

Parameters:
- context_id: Session ID to export.
- format: "markdown" (default) or "json".

Returns: exported content string."#,
            "inputSchema": {
                "type": "object",
                "properties": {
                    "context_id": { "type": "string", "description": "Session ID to export" },
                    "format": { "type": "string", "enum": ["markdown", "json"], "default": "markdown", "description": "Export format: markdown (default) or json" }
                },
                "required": ["context_id"]
            }
        })
    }

    async fn invoke(
        &self,
        args: serde_json::Value,
        ctx: &mut AppContext,
    ) -> anyhow::Result<serde_json::Value> {
        let context_id = args.get("context_id").and_then(|v| v.as_str()).unwrap_or("");
        let format = args.get("format").and_then(|v| v.as_str()).unwrap_or("markdown");
        if context_id.is_empty() {
            anyhow::bail!("Missing required argument: context_id");
        }

        let conn = ctx.conn()?;
        let (ctx_data, memories) =
            match crate::registry::agent_context::get_context_with_memories(&conn, context_id)? {
                Some(data) => data,
                None => anyhow::bail!("Context '{}' not found", context_id),
            };
        let linked = crate::registry::agent_context::list_linked_entities(&conn, context_id)?;

        let content = if format == "json" {
            serde_json::to_string_pretty(&json!({
                "context": {
                    "id": ctx_data.id,
                    "name": ctx_data.name,
                    "intent": ctx_data.intent,
                    "status": ctx_data.status,
                },
                "memories": memories.iter().map(|m| json!({
                    "type": m.memory_type,
                    "content": m.content,
                    "created_at": m.created_at.to_rfc3339(),
                })).collect::<Vec<_>>(),
                "linked_entities": linked.iter().map(|(eid, ltype, _)| json!({
                    "entity_id": eid,
                    "link_type": ltype,
                })).collect::<Vec<_>>(),
            }))? + "\n"
        } else {
            let mut md = format!("# Session: {}\n\n", ctx_data.name);
            if let Some(ref intent) = ctx_data.intent {
                md.push_str(&format!("**Intent:** {}\n\n", intent));
            }
            md.push_str(&format!("**Status:** {}\n\n", ctx_data.status));
            if !linked.is_empty() {
                md.push_str("## Linked Entities\n");
                for (eid, ltype, _) in &linked {
                    md.push_str(&format!("- `{}` ({})\n", eid, ltype));
                }
                md.push('\n');
            }
            if !memories.is_empty() {
                md.push_str("## Memories\n");
                for m in &memories {
                    md.push_str(&format!(
                        "### [{}] {}\n{}\n\n",
                        m.memory_type,
                        m.created_at.format("%Y-%m-%d %H:%M"),
                        m.content
                    ));
                }
            }
            md
        };

        Ok(json!({
            "success": true,
            "context_id": context_id,
            "format": format,
            "content": content,
        }))
    }
}

#[derive(Clone)]
pub struct DevkitSessionImportTool;

impl McpTool for DevkitSessionImportTool {
    fn name(&self) -> &'static str {
        "devkit_session_import"
    }

    fn schema(&self) -> serde_json::Value {
        json!({
            "description": r#"Import memories into a session from structured text. Parses a simple format where each memory is on its own line prefixed by [type]. Useful for bulk-importing ClaudeCode conversation excerpts or meeting notes.

Format example:
  [decision] Use SQLite for persistence
  [constraint] Must support Windows paths
  [note] Team agreed on AGPL license

Parameters:
- context_id: Target session ID (created if not exists).
- content: Text block to parse.
- default_type: Memory type for lines without prefix (default "note").

Returns: import summary."#,
            "inputSchema": {
                "type": "object",
                "properties": {
                    "context_id": { "type": "string", "description": "Target session ID (created if not exists)" },
                    "content": { "type": "string", "description": "Text block to parse for memory entries" },
                    "default_type": { "type": "string", "default": "note", "description": "Memory type for lines without a [type] prefix" }
                },
                "required": ["context_id", "content"]
            }
        })
    }

    async fn invoke(
        &self,
        args: serde_json::Value,
        ctx: &mut AppContext,
    ) -> anyhow::Result<serde_json::Value> {
        let context_id = args
            .get("context_id")
            .and_then(|v| v.as_str())
            .context("context_id is required")?;
        let content =
            args.get("content").and_then(|v| v.as_str()).context("content is required")?;
        let default_type = args.get("default_type").and_then(|v| v.as_str()).unwrap_or("note");
        if context_id.is_empty() {
            anyhow::bail!("Missing required argument: context_id");
        }
        if content.is_empty() {
            anyhow::bail!("Missing required argument: content");
        }

        let mut conn = ctx.conn_mut()?;
        // Ensure context exists
        if crate::registry::agent_context::get_context(&conn, context_id)?.is_none() {
            crate::registry::agent_context::upsert_context(
                &mut conn,
                context_id,
                context_id,
                Some("imported"),
            )?;
        }

        let mut imported = 0;
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let (ty, text) = if let Some(pos) = line.find(']') {
                if line.starts_with('[') && pos > 1 {
                    let t = &line[1..pos];
                    let rest = line[pos + 1..].trim();
                    (t, rest)
                } else {
                    (default_type, line)
                }
            } else {
                (default_type, line)
            };
            if !text.is_empty() {
                crate::registry::agent_context::insert_memory(
                    &mut conn, context_id, ty, text, None, None,
                )?;
                imported += 1;
            }
        }

        Ok(json!({
            "success": true,
            "context_id": context_id,
            "imported": imported,
        }))
    }
}

/// Truncate text to fit within an approximate token budget.
/// Rough heuristic: ~2.5 chars/token for ASCII, ~1.5 for CJK.
fn truncate_to_tokens(text: &str, max_tokens: usize) -> String {
    let total_chars = text.chars().count();
    let estimated_tokens = crate::registry::agent_context::estimate_tokens(text) as usize;
    if estimated_tokens <= max_tokens {
        return text.to_string();
    }
    // Truncate proportionally
    let target_chars = (total_chars as f64 * max_tokens as f64 / estimated_tokens as f64) as usize;
    let truncated: String = text.chars().take(target_chars.max(50)).collect();
    format!("{}…[truncated to ~{} tokens]", truncated, max_tokens)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::McpTool;

    #[test]
    fn test_session_tool_names() {
        assert_eq!(DevkitSessionSaveTool.name(), "devkit_session_save");
        assert_eq!(DevkitSessionListTool.name(), "devkit_session_list");
        assert_eq!(DevkitSessionResumeTool.name(), "devkit_session_resume");
        assert_eq!(DevkitSessionAttachTool.name(), "devkit_session_attach");
        assert_eq!(DevkitSessionDetachTool.name(), "devkit_session_detach");
        assert_eq!(DevkitSessionActivateTool.name(), "devkit_session_activate");
        assert_eq!(DevkitSessionSearchTool.name(), "devkit_session_search");
        assert_eq!(DevkitSessionCaptureTool.name(), "devkit_session_capture");
        assert_eq!(DevkitSessionWorkflowsTool.name(), "devkit_session_workflows");
    }

    #[test]
    fn test_schemas_are_objects() {
        assert!(DevkitSessionSaveTool.schema().is_object());
        assert!(DevkitSessionListTool.schema().is_object());
        assert!(DevkitSessionResumeTool.schema().is_object());
    }
}
