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
        crate::registry::relation::save_relation(&conn, &from, &to, &rel_type, confidence)?;

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
                let rows = crate::registry::relation::find_related_entities(
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
                let rows =
                    crate::registry::relation::list_relations(&conn, &entity_id, relation_type)?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_relation_store_and_query_roundtrip() {
        let tmp = tempfile::tempdir().unwrap();
        unsafe {
            std::env::set_var("DEVBASE_DATA_DIR", tmp.path());
        }
        let mut ctx = crate::storage::AppContext::with_defaults().unwrap();

        // Pre-seed entities to satisfy FK constraint
        let conn = ctx.conn().unwrap();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT OR IGNORE INTO entity_types (name, schema_json, description, created_at) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params!["test", "{}", "test type", &now],
        ).unwrap();
        crate::registry::upsert_entity(
            &conn,
            "entity-a",
            "test",
            "Entity A",
            None,
            &serde_json::json!({}),
        )
        .unwrap();
        crate::registry::upsert_entity(
            &conn,
            "entity-b",
            "test",
            "Entity B",
            None,
            &serde_json::json!({}),
        )
        .unwrap();
        drop(conn);

        let store_tool = DevkitRelationStoreTool;
        let store_result = store_tool
            .invoke(
                serde_json::json!({
                    "from_entity_id": "entity-a",
                    "to_entity_id": "entity-b",
                    "relation_type": "depends_on",
                    "confidence": 0.95
                }),
                &mut ctx,
            )
            .await
            .unwrap();
        assert_eq!(store_result.get("success").and_then(|v| v.as_bool()), Some(true));

        let query_tool = DevkitRelationQueryTool;
        let query_result = query_tool
            .invoke(
                serde_json::json!({
                    "entity_id": "entity-a",
                    "direction": "outgoing"
                }),
                &mut ctx,
            )
            .await
            .unwrap();
        assert_eq!(query_result.get("success").and_then(|v| v.as_bool()), Some(true));
        let count = query_result.get("count").and_then(|v| v.as_u64()).unwrap();
        assert_eq!(count, 1);
        let relations = query_result.get("relations").and_then(|v| v.as_array()).unwrap();
        assert_eq!(relations[0].get("to_entity_id").and_then(|v| v.as_str()), Some("entity-b"));
        assert_eq!(relations[0].get("relation_type").and_then(|v| v.as_str()), Some("depends_on"));
    }

    #[tokio::test]
    async fn test_relation_store_missing_required_fields() {
        let tmp = tempfile::tempdir().unwrap();
        unsafe {
            std::env::set_var("DEVBASE_DATA_DIR", tmp.path());
        }
        let mut ctx = crate::storage::AppContext::with_defaults().unwrap();

        let tool = DevkitRelationStoreTool;
        let result = tool
            .invoke(
                serde_json::json!({"from_entity_id": "", "to_entity_id": "b", "relation_type": ""}),
                &mut ctx,
            )
            .await
            .unwrap();
        assert_eq!(result.get("success").and_then(|v| v.as_bool()), Some(false));
    }

    #[tokio::test]
    async fn test_relation_query_bidirectional() {
        let tmp = tempfile::tempdir().unwrap();
        unsafe {
            std::env::set_var("DEVBASE_DATA_DIR", tmp.path());
        }
        let mut ctx = crate::storage::AppContext::with_defaults().unwrap();

        let conn = ctx.conn().unwrap();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT OR IGNORE INTO entity_types (name, schema_json, description, created_at) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params!["test", "{}", "test type", &now],
        ).unwrap();
        crate::registry::upsert_entity(&conn, "src", "test", "Src", None, &serde_json::json!({}))
            .unwrap();
        crate::registry::upsert_entity(&conn, "dst", "test", "Dst", None, &serde_json::json!({}))
            .unwrap();
        drop(conn);

        let store_tool = DevkitRelationStoreTool;
        store_tool
            .invoke(
                serde_json::json!({
                    "from_entity_id": "src",
                    "to_entity_id": "dst",
                    "relation_type": "calls"
                }),
                &mut ctx,
            )
            .await
            .unwrap();

        let query_tool = DevkitRelationQueryTool;
        let result = query_tool
            .invoke(
                serde_json::json!({
                    "entity_id": "dst",
                    "direction": "bidirectional"
                }),
                &mut ctx,
            )
            .await
            .unwrap();
        assert_eq!(result.get("success").and_then(|v| v.as_bool()), Some(true));
        let count = result.get("count").and_then(|v| v.as_u64()).unwrap();
        assert_eq!(count, 1);
    }
}
