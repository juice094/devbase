# devkit_session_recall

> **Tier**: Stable (frozen at v0.21.0)
> **Source**: `src/mcp/tools/session.rs` — `DevkitSessionRecallTool`

Semantic memory recall for an active agent session. Finds relevant past memories by meaning rather than exact keyword.

## Purpose

- Surface decisions, constraints, or discoveries related to the current task
- Inject top-k relevant memories into prompt context
- Recall what was discussed in a previous project session

## When NOT to use

- Keyword-based memory search → use `devkit_session_search`
- Listing all sessions → use `devkit_session_list`
- Saving a new memory → use `devkit_session_capture`
- When embeddings have not been stored for memories → use `devkit_session_index` first

## Input Schema

```json
{
  "type": "object",
  "properties": {
    "context_id": { "type": "string", "description": "Session ID (optional)" },
    "query_embedding": {
      "type": "array",
      "items": { "type": "number" },
      "description": "Query vector as f32 array (externally generated)"
    },
    "limit": { "type": "integer", "default": 5 }
  },
  "required": ["query_embedding"]
}
```

| Parameter       | Type       | Required | Default | Description                                |
|-----------------|------------|----------|---------|--------------------------------------------|
| `context_id`    | string     | No       | —       | Session ID. Falls back to `DEVBASE_ACTIVE_CONTEXT` env var or `.active_context` state file |
| `query_embedding`| number[]  | Yes      | —       | Externally-generated f32 embedding vector  |
| `limit`         | integer    | No       | 5       | Max results (capped at 20)                 |

## Important: Embedding source

devbase does **NOT** generate embeddings. The caller must provide a pre-computed vector from an external provider (Ollama, OpenAI, etc.). Use the same model that was used to index the memories via `devkit_session_index`.

## Output Schema

```json
{
  "success": true,
  "context_id": "project-alpha",
  "count": 3,
  "memories": [
    {
      "id": 42,
      "type": "decision",
      "content": "Use SQLite WAL mode for concurrent reads",
      "created_at": "2026-05-10T14:32:00Z",
      "embedding_model": "nomic-embed-text",
      "score": 0.91
    }
  ]
}
```

| Field             | Type    | Description                              |
|-------------------|---------|------------------------------------------|
| `id`              | integer | Memory row ID                            |
| `type`            | string  | Memory classification: decision, constraint, note, discovery, error, action |
| `content`         | string  | Full memory text                         |
| `created_at`      | string  | ISO 8601 timestamp                       |
| `embedding_model` | string  | Model used when memory was indexed       |
| `score`           | number  | Cosine similarity (0.0–1.0)              |

## Errors

| Error                        | Cause                                               |
|------------------------------|-----------------------------------------------------|
| `query_embedding required`   | Missing or empty embedding array                    |
| `query_embedding must not be empty` | Array contains no valid f32 values          |
| No active session            | `context_id` omitted and no active session set      |
| Memory not found             | `memory_id` in `devkit_session_index` does not exist|

## Changelog

| Version | Change                                    |
|---------|------------------------------------------|
| v0.21.0 | Schema frozen as Stable                  |
