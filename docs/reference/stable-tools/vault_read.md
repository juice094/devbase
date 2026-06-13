# devkit_vault_read

> **Tier**: Stable (frozen at v0.20.1)
> **Source**: `src/mcp/tools/vault.rs` — `DevkitVaultReadTool`

Read the complete Markdown content of a Vault note, including its YAML frontmatter and body.

## Purpose

- Read a specific note after finding it via `devkit_vault_search`
- Retrieve project documentation, architecture decisions, or design notes
- Extract frontmatter metadata (tags, repo links, ai_context)
- Render note content for the user or for downstream processing

## When NOT to use

- Searching for notes → use `devkit_vault_search`
- Writing or updating notes → use `devkit_vault_write`
- Finding backlinks → use `devkit_vault_backlinks`
- Reading code files → use filesystem tools or `devkit_project_context`

## Input Schema

```json
{
  "type": "object",
  "properties": {
    "path": { "type": "string", "description": "File path or note id" }
  },
  "required": ["path"]
}
```

| Parameter | Type   | Required | Default | Description                    |
|-----------|--------|----------|---------|--------------------------------|
| `path`    | string | Yes      | —       | Vault-relative path or note id |

## Output Schema

```json
{
  "success": true,
  "path": "references/mcp-integration.md",
  "frontmatter": {
    "id": "mcp-integration",
    "title": "MCP Integration Guide",
    "tags": ["mcp", "integration"],
    "repo": "devbase",
    "created": "2026-04-20",
    "updated": "2026-06-13"
  },
  "content": "# MCP Integration Guide\n\n..."
}
```

| Field         | Type    | Description                              |
|---------------|---------|------------------------------------------|
| `path`        | string  | Requested path                           |
| `frontmatter` | object? | Parsed YAML frontmatter                  |
| `content`     | string  | Markdown body (may be empty)             |

## Errors

| Error              | Cause                                    |
|--------------------|------------------------------------------|
| `path required`    | Missing or empty `path` argument         |
| Vault unreadable   | Vault directory missing or permission denied |
| Note not found     | No note matches the given path/id        |

## Changelog

| Version | Change                                    |
|---------|------------------------------------------|
| v0.14.2 | Promoted to Stable tier                  |
| v0.20.1 | Invocation test `test_tools_call_devkit_vault_read` added |
