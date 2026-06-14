# devkit_vault_search

> **Tier**: Stable (frozen at v0.21.0)
> **Source**: `src/mcp/tools/vault.rs` — `DevkitVaultSearchTool`

Search the devbase Vault (Markdown notes) by keywords across note titles, tags, and full content using Tantivy BM25 full-text search.

## Purpose

- Find notes related to a topic, architecture decision, or project
- Discover linked concepts via tags or wikilinks
- Locate a note when you only remember fragments of its content
- Check if a topic has been documented before writing a new note

## When NOT to use

- Reading the full content of a known note → use `devkit_vault_read`
- Writing or updating notes → use `devkit_vault_write`
- Finding backlinks to a specific note → use `devkit_vault_backlinks`
- Searching across code repositories → use `devkit_query_repos`

## Input Schema

```json
{
  "type": "object",
  "properties": {
    "query": { "type": "string", "description": "Search keywords" }
  },
  "required": ["query"]
}
```

| Parameter | Type   | Required | Default | Description                    |
|-----------|--------|----------|---------|--------------------------------|
| `query`   | string | Yes      | —       | Space-separated keywords (AND) |

## Matching behavior

- Primary path: Tantivy BM25 full-text search across note title, tags, and full Markdown body content.
- Fallback path: if the Tantivy index is empty/unavailable, case-insensitive substring scan across the same fields.
- All keywords must match (AND logic).
- Run `devbase vault reindex` or `devkit_index` to build/update the search index.

## Output Schema

```json
{
  "success": true,
  "count": 2,
  "query": "mcp integration",
  "notes": [
    {
      "id": "mcp-integration-guide",
      "title": "MCP Integration Guide",
      "path": "references/mcp-integration.md",
      "tags": ["mcp", "integration", "architecture"]
    }
  ]
}
```

| Field   | Type     | Description                              |
|---------|----------|------------------------------------------|
| `id`    | string   | Note identifier (usually filename stem)  |
| `title` | string   | Parsed from YAML frontmatter             |
| `path`  | string   | Vault-relative file path                 |
| `tags`  | string[] | Parsed from YAML frontmatter             |

## Errors

| Error              | Cause                                    |
|--------------------|------------------------------------------|
| `query required`   | Missing or empty `query` argument        |
| Vault unreadable   | Vault directory missing or permission denied |

## Changelog

| Version | Change                                    |
|---------|------------------------------------------|
| Unreleased | Primary search path switched to Tantivy BM25; substring scan retained as fallback |
| v0.21.0 | Schema frozen as Stable                  |
