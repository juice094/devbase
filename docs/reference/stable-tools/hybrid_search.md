# devkit_hybrid_search

> **Tier**: Stable (frozen at v0.21.0)
> **Source**: `src/mcp/tools/search.rs` — `DevkitHybridSearchTool`

Hybrid code symbol search combining vector embeddings and keyword matching via Reciprocal Rank Fusion (RRF).

## Purpose

- Find code related to a concept ("authentication", "error handling")
- Search with either natural language or an embedding vector
- Get robust results even when the embedding provider is offline

## When NOT to use

- Exact keyword searches → use `devkit_natural_language_query`
- Finding symbol definitions by exact name → use `devkit_code_symbols`
- When no embeddings exist and no keyword query is available

## Input Schema

```json
{
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
```

| Parameter       | Type       | Required | Default | Description                                |
|-----------------|------------|----------|---------|--------------------------------------------|
| `repo_id`       | string     | Yes      | —       | Registered repository ID                   |
| `query_text`    | string     | Yes      | —       | Keyword or natural language query          |
| `query_embedding`| number[]  | No       | —       | Optional f32 vector for semantic search    |
| `limit`         | integer    | No       | 10      | Max results (capped at 50)                 |

## Behavior

| Scenario                              | Behavior                                          |
|---------------------------------------|---------------------------------------------------|
| `query_embedding` provided            | RRF fusion: vector similarity (70%) + keyword (30%) |
| `query_embedding` omitted             | Falls back to pure keyword search on symbol names/signatures |
| No embeddings exist for repo          | Gracefully degrades to keyword search             |
| Embedding generation fails            | Warns in logs, falls back to keyword search       |

## Output Schema

```json
{
  "success": true,
  "repo_id": "devbase",
  "query_text": "error handling",
  "count": 3,
  "symbols": [
    {
      "name": "handle_error",
      "file_path": "src/errors.rs",
      "line_start": 42,
      "similarity_score": 0.87
    }
  ]
}
```

| Field            | Type    | Description                              |
|------------------|---------|------------------------------------------|
| `name`           | string  | Symbol name                              |
| `file_path`      | string  | Relative file path in the repo           |
| `line_start`     | integer | Line number where symbol begins          |
| `similarity_score`| number | RRF score (0.0–1.0, higher is better)   |

## Errors

| Error              | Cause                                    |
|--------------------|------------------------------------------|
| `repo_id required` | Missing `repo_id`                        |
| `query_text required`| Missing `query_text`                   |
| Database error     | SQLite query failure                     |

## Changelog

| Version | Change                                    |
|---------|------------------------------------------|
| v0.21.0 | Schema frozen as Stable                  |
