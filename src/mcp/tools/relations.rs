use crate::mcp::McpTool;

#[derive(Clone)]
pub struct DevkitRelationStoreTool;

impl McpTool for DevkitRelationStoreTool {
    fn name(&self) -> &'static str {
        "devkit_relation_store"
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "description": r#"Store or update a directional relation between two entities in the devbase registry.

Use this when the user wants to:
- Record a conceptual link between two code symbols, repos, or knowledge entities
- Tag a dependency, similarity, or causal relationship
- Build a knowledge graph incrementally

Parameters:
- from_entity_id: Source entity ID (e.g., repo ID, symbol ID, or entity UUID)
- to_entity_id: Target entity ID
- relation_type: Relationship label (e.g., 'depends_on', 'similar_to', 'calls', 'extends')
- confidence: Optional confidence score 0.0–1.0 (default 1.0)

Returns: success boolean and relation details."#,
            "inputSchema": {
                "type": "object",
                "properties": {
                    "from_entity_id": { "type": "string" },
                    "to_entity_id": { "type": "string" },
                    "relation_type": { "type": "string" },
                    "confidence": { "type": "number" }
                },
                "required": ["from_entity_id", "to_entity_id", "relation_type"]
            }
        })
    }

    async fn invoke(
        &self,
        args: serde_json::Value,
        ctx: &mut crate::storage::AppContext,
    ) -> anyhow::Result<serde_json::Value> {
        let from = args.get("from_entity_id").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let to = args.get("to_entity_id").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let rel_type = args.get("relation_type").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let confidence = args.get("confidence").and_then(|v| v.as_f64()).unwrap_or(1.0);

        if from.is_empty() || to.is_empty() || rel_type.is_empty() {
            return Ok(serde_json::json!({
                "success": false,
                "error": "from_entity_id, to_entity_id, and relation_type are required"
            }));
        }

        let conn = ctx.conn()?;
        crate::registry::WorkspaceRegistry::save_relation(&conn, &from, &to, &rel_type, confidence)?;

        Ok(serde_json::json!({
            "success": true,
            "from_entity_id": from,
            "to_entity_id": to,
            "relation_type": rel_type,
            "confidence": confidence
        }))
    }
}

#[derive(Clone)]
pub struct DevkitRelationQueryTool;

impl McpTool for DevkitRelationQueryTool {
    fn name(&self) -> &'static str {
        "devkit_relation_query"
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "description": r#"Query relations (knowledge graph edges) from the devbase registry.

Use this when the user wants to:
- Find all entities related to a given symbol, repo, or concept
- Explore outgoing links from an entity
- Filter by relationship type (e.g., only 'depends_on')

Parameters:
- entity_id: The entity to query around
- relation_type: Optional filter by relationship label (omit for all types)
- direction: 'outgoing' | 'bidirectional' (default: 'outgoing')

Returns: JSON array of relations with to_entity_id, relation_type, confidence, and created_at."#,
            "inputSchema": {
                "type": "object",
                "properties": {
                    "entity_id": { "type": "string" },
                    "relation_type": { "type": "string" },
                    "direction": { "type": "string", "enum": ["outgoing", "bidirectional"] }
                },
                "required": ["entity_id"]
            }
        })
    }

    async fn invoke(
        &self,
        args: serde_json::Value,
        ctx: &mut crate::storage::AppContext,
    ) -> anyhow::Result<serde_json::Value> {
        let entity_id = args.get("entity_id").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let relation_type = args.get("relation_type").and_then(|v| v.as_str());
        let direction = args.get("direction").and_then(|v| v.as_str()).unwrap_or("outgoing");

        if entity_id.is_empty() {
            return Ok(serde_json::json!({
                "success": false,
                "error": "entity_id is required"
            }));
        }

        let conn = ctx.conn()?;
        let results = match direction {
            "bidirectional" => {
                let rows = crate::registry::WorkspaceRegistry::find_related_entities(
                    &conn,
                    &entity_id,
                    relation_type,
                )?;
                rows.into_iter()
                    .map(|(from, to, rt, conf, created)| {
                        serde_json::json!({
                            "from_entity_id": from,
                            "to_entity_id": to,
                            "relation_type": rt,
                            "confidence": conf,
                            "created_at": created
                        })
                    })
                    .collect::<Vec<_>>()
            }
            _ => {
                let rows = crate::registry::WorkspaceRegistry::list_relations(
                    &conn,
                    &entity_id,
                    relation_type,
                )?;
                rows.into_iter()
                    .map(|(to, rt, conf, created)| {
                        serde_json::json!({
                            "to_entity_id": to,
                            "relation_type": rt,
                            "confidence": conf,
                            "created_at": created
                        })
                    })
                    .collect::<Vec<_>>()
            }
        };

        Ok(serde_json::json!({
            "success": true,
            "entity_id": entity_id,
            "direction": direction,
            "count": results.len(),
            "relations": results
        }))
    }
}
