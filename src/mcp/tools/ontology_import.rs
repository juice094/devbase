use crate::mcp::McpTool;

#[derive(Clone)]
pub struct DevkitOntologyImportTool;

impl McpTool for DevkitOntologyImportTool {
    fn name(&self) -> &'static str {
        "devkit_ontology_import"
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "description": r#"Import ontology entities and relations from an OpenClaw-compatible workspace into devbase.

Reads ontology/entities/*.json and ontology/relations/*.jsonl from the specified workspace path
and maps them into devbase's entities and relations tables.

Entity JSON format: { "entity_id": "...", "type": "...", "name": "...", "aliases": [...], ... }
Relation JSONL format: { "relation_id": "...", "type": "...", "from": "...", "to": "...", ... }

Requires DEVBASE_MCP_ENABLE_DESTRUCTIVE=1 since this modifies the registry."#,
            "inputSchema": {
                "type": "object",
                "properties": {
                    "workspace_path": {
                        "type": "string",
                        "description": "Path to the OpenClaw workspace root (contains ontology/ subdirectory). Defaults to the configured openclaw workspace."
                    }
                },
                "required": []
            }
        })
    }

    async fn invoke(
        &self,
        args: serde_json::Value,
        ctx: &mut crate::storage::AppContext,
    ) -> anyhow::Result<serde_json::Value> {
        crate::mcp::check_destructive_enabled()?;

        let workspace = args
            .get("workspace_path")
            .and_then(|v| v.as_str())
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| {
                dirs::home_dir()
                    .unwrap_or_default()
                    .join(".kimi_openclaw")
                    .join("workspace")
            });

        let stats = {
            let conn = ctx.conn()?;
            crate::registry::import_ontology::import_ontology(&conn, &workspace)?
        };

        Ok(serde_json::json!({
            "workspace": workspace.to_string_lossy().to_string(),
            "entities_added": stats.entities_added,
            "entities_updated": stats.entities_updated,
            "relations_added": stats.relations_added,
            "relations_updated": stats.relations_updated,
            "errors": stats.errors,
            "total_entities": stats.entities_added + stats.entities_updated,
            "total_relations": stats.relations_added + stats.relations_updated,
        }))
    }
}
