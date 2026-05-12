// SPDX-License-Identifier: MIT
// Copyright (c) 2026 juice094
//! MCP tools for Agent Context management (P1: Claude Projects inspired sessions).

use crate::mcp::McpTool;
use crate::storage::AppContext;
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
                        &mut conn, context_id, ty, content,
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
                }))
            }
            None => Ok(json!({
                "success": false,
                "error": format!("Session '{}' not found", context_id)
            })),
        }
    }
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
    }

    #[test]
    fn test_schemas_are_objects() {
        assert!(DevkitSessionSaveTool.schema().is_object());
        assert!(DevkitSessionListTool.schema().is_object());
        assert!(DevkitSessionResumeTool.schema().is_object());
    }
}
