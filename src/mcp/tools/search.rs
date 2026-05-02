use crate::mcp::McpTool;
use crate::repository::knowledge::KnowledgeRepository;
use crate::storage::AppContext;
use anyhow::Context;

/// Parse a JSON array of numbers into a Vec<f32>.
fn parse_f32_array(value: &serde_json::Value, field: &str) -> anyhow::Result<Vec<f32>> {
    let arr = value
        .get(field)
        .and_then(|v| v.as_array())
        .with_context(|| format!("{} must be an array of numbers", field))?;
    arr.iter()
        .map(|v| {
            v.as_f64()
                .map(|f| f as f32)
                .with_context(|| format!("{} contains non-numeric value", field))
        })
        .collect::<Result<Vec<f32>, _>>()
}
#[derive(Clone)]
pub struct DevkitSemanticSearchTool;

impl McpTool for DevkitSemanticSearchTool {
    fn name(&self) -> &'static str {
        "devkit_semantic_search"
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "description": r#"Search for code symbols semantically similar to a query embedding vector. Supports both externally-generated embeddings and on-the-fly local embedding generation.

Use this when the user wants to:
- Find code related to a concept (e.g., "authentication", "error handling", "config parsing")
- Discover functions by what they do, not what they're named
- Explore unfamiliar codebases using natural language

Do NOT use this for:
- Exact keyword searches (use devkit_natural_language_query or devkit_query instead)
- Finding symbol definitions by exact name (use devkit_code_symbols instead)
- When no embeddings have been stored for the repository (use devkit_embedding_store first)

Parameters:
- repo_id: Registered repository ID to search within.
- query_embedding: Query vector as an array of f32 numbers. Must match the dimension of stored embeddings.
- query_text: Natural language query text. If query_embedding is omitted, devbase will generate it locally.
- limit: Maximum results (default: 10, max: 50).

Returns: JSON array of matching symbols with file_path, name, line_start, and similarity_score (0.0-1.0)."#,
            "inputSchema": {
                "type": "object",
                "properties": {
                    "repo_id": { "type": "string" },
                    "query_embedding": {
                        "type": "array",
                        "items": { "type": "number" },
                        "description": "Query embedding vector as an array of f32 numbers"
                    },
                    "query_text": { "type": "string", "description": "Natural language query text (alternative to query_embedding)" },
                    "limit": { "type": "integer", "default": 10 }
                },
                "required": ["repo_id"]
            }
        })
    }

    async fn invoke(
        &self,
        args: serde_json::Value,
        ctx: &mut AppContext,
    ) -> anyhow::Result<serde_json::Value> {
        let repo_id = args.get("repo_id").and_then(|v| v.as_str()).context("repo_id required")?;
        let query_emb = match args.get("query_embedding").and_then(|v| v.as_array()) {
            Some(arr) => arr.iter().filter_map(|v| v.as_f64().map(|f| f as f32)).collect::<Vec<f32>>(),
            None => {
                let query_text = args.get("query_text").and_then(|v| v.as_str())
                    .context("query_embedding or query_text required")?;
                match crate::embedding::generate_query_embedding(query_text) {
                    Ok(emb) => emb,
                    Err(e) => return Err(anyhow::anyhow!("Embedding generation failed: {}", e)),
                }
            }
        };
        let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(10).min(50) as usize;

        let repo_id = repo_id.to_string();

        let pool = ctx.pool();
        tokio::task::spawn_blocking(move || {
            let conn = pool.get()?;
            let results = crate::registry::WorkspaceRegistry::semantic_search_symbols(
                &conn, &repo_id, &query_emb, limit,
            )?;

            let symbols: Vec<serde_json::Value> = results
                .into_iter()
                .map(|(_repo, name, path, line, sim)| {
                    serde_json::json!({
                        "name": name,
                        "file_path": path,
                        "line_start": line,
                        "similarity_score": sim,
                    })
                })
                .collect();

            Ok::<_, anyhow::Error>(serde_json::json!({
                "success": true,
                "repo_id": repo_id,
                "count": symbols.len(),
                "symbols": symbols,
            }))
        })
        .await
        .map_err(|e| anyhow::anyhow!("spawn_blocking failed: {}", e))?
    }
}
#[derive(Clone)]
pub struct DevkitEmbeddingStoreTool;

impl McpTool for DevkitEmbeddingStoreTool {
    fn name(&self) -> &'static str {
        "devkit_embedding_store"
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "description": r#"Store an externally-generated embedding vector for a code symbol. This is the storage-side of the "outboard brain" architecture: an external MCP Server or Skill (Ollama, llama.cpp, ONNX, remote API) generates embeddings, and devbase persists them in SQLite for similarity search.

Use this when:
- An external embedding provider has generated a vector for a symbol and wants to store it
- Indexing a repository with custom embedding models not supported natively
- Updating embeddings after code changes

Parameters:
- repo_id: Registered repository ID.
- symbol_name: Name of the code symbol (must match an entry in code_symbols table).
- embedding: Embedding vector as an array of f32 numbers.

Returns: success flag and count of stored embeddings."#,
            "inputSchema": {
                "type": "object",
                "properties": {
                    "repo_id": { "type": "string" },
                    "symbol_name": { "type": "string" },
                    "embedding": {
                        "type": "array",
                        "items": { "type": "number" },
                        "description": "Embedding vector as an array of f32 numbers"
                    }
                },
                "required": ["repo_id", "symbol_name", "embedding"]
            }
        })
    }

    async fn invoke(
        &self,
        args: serde_json::Value,
        ctx: &mut AppContext,
    ) -> anyhow::Result<serde_json::Value> {
        let repo_id = args.get("repo_id").and_then(|v| v.as_str()).context("repo_id required")?;
        let symbol_name = args
            .get("symbol_name")
            .and_then(|v| v.as_str())
            .context("symbol_name required")?;
        let embedding = parse_f32_array(&args, "embedding")?;

        let repo_id = repo_id.to_string();
        let symbol_name = symbol_name.to_string();

        let pool = ctx.pool();
        tokio::task::spawn_blocking(move || {
            let conn = pool.get()?;
            let pairs = vec![(symbol_name.clone(), embedding)];
            let count = KnowledgeRepository::new(&conn).save_embeddings(&repo_id, &pairs)?;

            Ok::<_, anyhow::Error>(serde_json::json!({
                "success": true,
                "repo_id": repo_id,
                "symbol_name": symbol_name,
                "stored": count,
            }))
        })
        .await
        .map_err(|e| anyhow::anyhow!("spawn_blocking failed: {}", e))?
    }
}
#[derive(Clone)]
pub struct DevkitEmbeddingSearchTool;

impl McpTool for DevkitEmbeddingSearchTool {
    fn name(&self) -> &'static str {
        "devkit_embedding_search"
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "description": r#"Search for code symbols using an externally-provided query embedding vector. Alias for devkit_semantic_search with the same vector-based interface. Use whichever name is more intuitive for your workflow.

Parameters:
- repo_id: Registered repository ID to search within.
- query_embedding: Query vector as an array of f32 numbers.
- limit: Maximum results (default: 10, max: 50).

Returns: JSON array of matching symbols with file_path, name, line_start, and similarity_score (0.0-1.0)."#,
            "inputSchema": {
                "type": "object",
                "properties": {
                    "repo_id": { "type": "string" },
                    "query_embedding": {
                        "type": "array",
                        "items": { "type": "number" },
                        "description": "Query embedding vector as an array of f32 numbers"
                    },
                    "limit": { "type": "integer", "default": 10 }
                },
                "required": ["repo_id", "query_embedding"]
            }
        })
    }

    async fn invoke(
        &self,
        args: serde_json::Value,
        ctx: &mut AppContext,
    ) -> anyhow::Result<serde_json::Value> {
        // Delegate to DevkitSemanticSearchTool — same logic
        DevkitSemanticSearchTool.invoke(args, ctx).await
    }
}
#[derive(Clone)]
pub struct DevkitHybridSearchTool;

impl McpTool for DevkitHybridSearchTool {
    fn name(&self) -> &'static str {
        "devkit_hybrid_search"
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "description": r#"Hybrid code symbol search combining vector embeddings and keyword matching via Reciprocal Rank Fusion (RRF). This is the recommended default search tool when looking for code concepts.

Behavior:
- If query_embedding is provided: fuses vector similarity (70%) + keyword BM25-like matching (30%) via RRF.
- If query_embedding is omitted: falls back to pure keyword search on symbol names and signatures.
- If no embeddings exist for the repo: gracefully degrades to keyword search.

Use this when the user wants to:
- Find code related to a concept ("authentication", "error handling")
- Search with either a natural language description or an embedding vector
- Get robust results even when the embedding provider is offline

Parameters:
- repo_id: Registered repository ID to search within.
- query_text: Text query for keyword matching (always used).
- query_embedding: Optional f32 vector for semantic search.
- limit: Maximum results (default: 10, max: 50).

Returns: JSON array of symbols with file_path, name, line_start, and similarity_score."#,
            "inputSchema": {
                "type": "object",
                "properties": {
                    "repo_id": { "type": "string" },
                    "query_text": { "type": "string", "description": "Keyword or natural language query" },
                    "query_embedding": {
                        "type": "array",
                        "items": { "type": "number" },
                        "description": "Optional query embedding vector"
                    },
                    "limit": { "type": "integer", "default": 10 }
                },
                "required": ["repo_id", "query_text"]
            }
        })
    }

    async fn invoke(
        &self,
        args: serde_json::Value,
        ctx: &mut AppContext,
    ) -> anyhow::Result<serde_json::Value> {
        let repo_id = args.get("repo_id").and_then(|v| v.as_str()).context("repo_id required")?;
        let query_text =
            args.get("query_text").and_then(|v| v.as_str()).context("query_text required")?;
        let query_embedding = args.get("query_embedding").and_then(|v| v.as_array()).map(|arr| {
            arr.iter().filter_map(|v| v.as_f64().map(|f| f as f32)).collect::<Vec<f32>>()
        });
        let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(10).min(50) as usize;

        let query_embedding = match query_embedding {
            Some(e) => Some(e),
            None => {
                match crate::embedding::generate_query_embedding(query_text) {
                    Ok(emb) => Some(emb),
                    Err(e) => {
                        tracing::warn!("Embedding generation failed, falling back to keyword: {}", e);
                        None
                    }
                }
            }
        };

        let repo_id = repo_id.to_string();
        let query_text = query_text.to_string();

        let pool = ctx.pool();
        tokio::task::spawn_blocking(move || {
            let conn = pool.get()?;
            let results = crate::registry::WorkspaceRegistry::hybrid_search_symbols(
                &conn,
                &repo_id,
                &query_text,
                query_embedding.as_deref(),
                limit,
            )?;

            let symbols: Vec<serde_json::Value> = results
                .into_iter()
                .map(|(_repo, name, path, line, sim)| {
                    serde_json::json!({
                        "name": name,
                        "file_path": path,
                        "line_start": line,
                        "similarity_score": sim,
                    })
                })
                .collect();

            Ok::<_, anyhow::Error>(serde_json::json!({
                "success": true,
                "repo_id": repo_id,
                "query_text": query_text,
                "count": symbols.len(),
                "symbols": symbols,
            }))
        })
        .await
        .map_err(|e| anyhow::anyhow!("spawn_blocking failed: {}", e))?
    }
}
#[derive(Clone)]
pub struct DevkitRelatedSymbolsTool;

impl McpTool for DevkitRelatedSymbolsTool {
    fn name(&self) -> &'static str {
        "devkit_related_symbols"
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "description": r#"Find symbols explicitly linked to a given symbol via conceptual relationships (similar signature, co-located in same file). This goes beyond the call graph to discover "related concepts".

Use this when the user wants to:
- Find functions with similar signatures (e.g., "other functions that also take a token parameter")
- Discover utilities in the same file that might be relevant
- Explore conceptual neighbors beyond direct callers/callees

Parameters:
- repo_id: Registered repository ID.
- symbol_name: Name of the source symbol.
- limit: Maximum related symbols (default: 10, max: 50).

Returns: JSON array of related symbols with target_symbol, link_type, and strength (0.0-1.0)."#,
            "inputSchema": {
                "type": "object",
                "properties": {
                    "repo_id": { "type": "string" },
                    "symbol_name": { "type": "string" },
                    "limit": { "type": "integer", "default": 10 }
                },
                "required": ["repo_id", "symbol_name"]
            }
        })
    }

    async fn invoke(
        &self,
        args: serde_json::Value,
        ctx: &mut AppContext,
    ) -> anyhow::Result<serde_json::Value> {
        let repo_id = args.get("repo_id").and_then(|v| v.as_str()).context("repo_id required")?;
        let symbol_name = args
            .get("symbol_name")
            .and_then(|v| v.as_str())
            .context("symbol_name required")?;
        let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(10).min(50) as usize;

        let repo_id = repo_id.to_string();
        let symbol_name = symbol_name.to_string();

        let pool = ctx.pool();
        tokio::task::spawn_blocking(move || {
            let conn = pool.get()?;
            let results = crate::registry::WorkspaceRegistry::find_related_symbols(
                &conn,
                &repo_id,
                &symbol_name,
                limit,
            )?;

            let links: Vec<serde_json::Value> = results
                .into_iter()
                .map(|(_src_repo, _src_sym, target_repo, target_symbol, link_type, strength)| {
                    serde_json::json!({
                        "target_repo": target_repo,
                        "target_symbol": target_symbol,
                        "link_type": link_type,
                        "strength": strength,
                    })
                })
                .collect();

            Ok::<_, anyhow::Error>(serde_json::json!({
                "success": true,
                "repo_id": repo_id,
                "symbol_name": symbol_name,
                "count": links.len(),
                "links": links,
            }))
        })
        .await
        .map_err(|e| anyhow::anyhow!("spawn_blocking failed: {}", e))?
    }
}
#[derive(Clone)]
pub struct DevkitCrossRepoSearchTool;

impl McpTool for DevkitCrossRepoSearchTool {
    fn name(&self) -> &'static str {
        "devkit_cross_repo_search"
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "description": r#"Search for code symbols across multiple repositories filtered by tags. Uses hybrid search (vector + keyword RRF merge) on each matching repo and returns globally deduplicated results.

Use this when the user wants to:
- Find a pattern across all Rust projects (e.g., "error handling" in repos tagged "rust")
- Search across a technology area (e.g., "config parsing" in all CLI tools)
- Discover reusable utilities across the workspace

Do NOT use this for:
- Single-repo searches (use devkit_hybrid_search or devkit_semantic_search)
- Exact symbol lookup (use devkit_code_symbols)

Parameters:
- tags: Array of tag strings. Only repos matching ALL tags are searched. Empty array searches all repos.
- query_text: Natural language or keyword query for the symbol search.
- query_embedding: Optional query vector as f32 array. If provided, enables hybrid vector+keyword search.
- limit: Maximum results (default: 10, max: 50).

Returns: JSON array of symbols with repo_id, file_path, name, line_start, and similarity_score."#,
            "inputSchema": {
                "type": "object",
                "properties": {
                    "tags": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Tags to filter repos (AND semantics). Empty = all repos."
                    },
                    "query_text": { "type": "string", "description": "Keyword or natural language query" },
                    "query_embedding": {
                        "type": "array",
                        "items": { "type": "number" },
                        "description": "Optional query embedding vector"
                    },
                    "limit": { "type": "integer", "default": 10 }
                },
                "required": ["tags", "query_text"]
            }
        })
    }

    async fn invoke(
        &self,
        args: serde_json::Value,
        ctx: &mut AppContext,
    ) -> anyhow::Result<serde_json::Value> {
        let tags = args
            .get("tags")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter().filter_map(|v| v.as_str().map(String::from)).collect::<Vec<String>>()
            })
            .unwrap_or_default();
        let query_text =
            args.get("query_text").and_then(|v| v.as_str()).context("query_text required")?;
        let query_embedding = args.get("query_embedding").and_then(|v| v.as_array()).map(|arr| {
            arr.iter().filter_map(|v| v.as_f64().map(|f| f as f32)).collect::<Vec<f32>>()
        });
        let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(10).min(50) as usize;

        let query_text = query_text.to_string();

        let pool = ctx.pool();
        tokio::task::spawn_blocking(move || {
            let conn = pool.get()?;
            let results = crate::registry::WorkspaceRegistry::cross_repo_search_symbols(
                &conn,
                &tags,
                &query_text,
                query_embedding.as_deref(),
                limit,
            )?;

            let symbols: Vec<serde_json::Value> = results
                .into_iter()
                .map(|(repo, name, path, line, sim)| {
                    serde_json::json!({
                        "repo_id": repo,
                        "name": name,
                        "file_path": path,
                        "line_start": line,
                        "similarity_score": sim,
                    })
                })
                .collect();

            Ok::<_, anyhow::Error>(serde_json::json!({
                "success": true,
                "tags": tags,
                "query_text": query_text,
                "count": symbols.len(),
                "symbols": symbols,
            }))
        })
        .await
        .map_err(|e| anyhow::anyhow!("spawn_blocking failed: {}", e))?
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_f32_array() {
        let value = serde_json::json!({"embedding": [0.1, 0.2, 0.3]});
        let arr = parse_f32_array(&value, "embedding").unwrap();
        assert_eq!(arr, vec![0.1_f32, 0.2_f32, 0.3_f32]);

        let empty = serde_json::json!({"embedding": []});
        assert!(parse_f32_array(&empty, "embedding").unwrap().is_empty());

        let bad = serde_json::json!({"embedding": ["not", "numbers"]});
        assert!(parse_f32_array(&bad, "embedding").is_err());
    }
}
