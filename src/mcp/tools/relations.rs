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
        let from = args
            .get("from_entity_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        let to = args
            .get("to_entity_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        let rel_type = args
            .get("relation_type")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        let confidence = args.get("confidence").and_then(|v| v.as_f64()).unwrap_or(1.0);

        if from.is_empty() || to.is_empty() || rel_type.is_empty() {
            return Ok(serde_json::json!({
                "success": false,
                "error": "from_entity_id, to_entity_id, and relation_type are required"
            }));
        }
        if !(0.0..=1.0).contains(&confidence) {
            return Ok(serde_json::json!({
                "success": false,
                "error": "confidence must be between 0.0 and 1.0"
            }));
        }
        if from == to {
            return Ok(serde_json::json!({
                "success": false,
                "error": "self-relations (from_entity_id == to_entity_id) are not allowed"
            }));
        }

        let conn = ctx.conn()?;
        if let Err(e) =
            crate::registry::relation::save_relation(&conn, &from, &to, &rel_type, confidence)
        {
            let msg = e.to_string();
            if msg.contains("foreign key constraint") || msg.contains("FOREIGN KEY") {
                return Ok(serde_json::json!({
                    "success": false,
                    "error": format!("Entity not found in registry. Ensure both '{}' and '{}' exist as registered entities.", from, to)
                }));
            }
            return Ok(serde_json::json!({
                "success": false,
                "error": msg
            }));
        }

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
                    "direction": { "type": "string", "enum": ["outgoing", "incoming", "bidirectional"] }
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
        let entity_id =
            args.get("entity_id").and_then(|v| v.as_str()).unwrap_or("").trim().to_string();
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
            "incoming" => {
                let mut stmt = conn.prepare(
                    "SELECT from_entity_id, relation_type, confidence, created_at FROM relations
                     WHERE to_entity_id = ?1
                     ORDER BY confidence DESC",
                )?;
                let rows = stmt.query_map([&entity_id], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, f64>(2)?,
                        row.get::<_, String>(3)?,
                    ))
                })?;
                let filtered: Vec<_> = if let Some(rt) = relation_type.filter(|s| !s.is_empty()) {
                    rows.filter(|r| r.as_ref().map(|(_, t, _, _)| t == rt).unwrap_or(false))
                        .collect::<Result<Vec<_>, _>>()?
                } else {
                    rows.collect::<Result<Vec<_>, _>>()?
                };
                filtered
                    .into_iter()
                    .map(|(from, rt, conf, created)| {
                        serde_json::json!({
                            "from_entity_id": from,
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

#[derive(Clone)]
pub struct DevkitRelationDeleteTool;

impl McpTool for DevkitRelationDeleteTool {
    fn name(&self) -> &'static str {
        "devkit_relation_delete"
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "description": r#"Delete a relation between two entities from the devbase registry.

Parameters:
- from_entity_id: Source entity ID
- to_entity_id: Target entity ID
- relation_type: Relationship label (optional — if omitted, deletes all relations between the two entities)

Returns: success boolean and count of deleted relations."#,
            "inputSchema": {
                "type": "object",
                "properties": {
                    "from_entity_id": { "type": "string" },
                    "to_entity_id": { "type": "string" },
                    "relation_type": { "type": "string" }
                },
                "required": ["from_entity_id", "to_entity_id"]
            }
        })
    }

    async fn invoke(
        &self,
        args: serde_json::Value,
        ctx: &mut crate::storage::AppContext,
    ) -> anyhow::Result<serde_json::Value> {
        let from = args
            .get("from_entity_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        let to = args
            .get("to_entity_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        let rel_type =
            args.get("relation_type").and_then(|v| v.as_str()).map(|s| s.trim().to_string());

        if from.is_empty() || to.is_empty() {
            return Ok(serde_json::json!({
                "success": false,
                "error": "from_entity_id and to_entity_id are required"
            }));
        }

        let conn = ctx.conn()?;
        let count = match rel_type.as_deref().filter(|s| !s.is_empty()) {
            Some(rt) => conn.execute(
                "DELETE FROM relations WHERE from_entity_id = ?1 AND to_entity_id = ?2 AND relation_type = ?3",
                rusqlite::params![&from, &to, rt],
            )?,
            None => conn.execute(
                "DELETE FROM relations WHERE from_entity_id = ?1 AND to_entity_id = ?2",
                rusqlite::params![&from, &to],
            )?,
        };

        Ok(serde_json::json!({
            "success": true,
            "deleted": count,
            "from_entity_id": from,
            "to_entity_id": to,
            "relation_type": rel_type
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
