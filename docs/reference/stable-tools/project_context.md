# devkit_project_context

> **Tier**: Stable (frozen at v0.20.1)
> **Source**: `src/mcp/tools/context.rs` — `DevkitProjectContextTool`

Retrieve a unified context snapshot for a project by aggregating repository metadata, linked Vault notes, code symbols, call graph edges, asset files, relations, workflows, known limits, and available skills.

## Purpose

- Understand a project holistically in a single tool call
- Prepare context before answering questions about a codebase
- Build project briefs or summaries without multiple round trips
- Discover documentation, assets, and related entities for a project

## When NOT to use

- Searching across all repos → use `devkit_query_repos`
- Vault full-text search without a project → use `devkit_vault_search`
- Checking health of multiple repos → use `devkit_health`
- Only one specific fact is needed → use the specific tool to save context space

## Input Schema

```json
{
  "type": "object",
  "properties": {
    "project": {
      "type": "string",
      "description": "Project identifier (repo id, repo name, or vault note id/path)"
    },
    "goal": {
      "type": "string",
      "description": "Optional task description for relevance-ranking symbols and calls"
    }
  },
  "required": ["project"]
}
```

| Parameter | Type   | Required | Default | Description                              |
|-----------|--------|----------|---------|------------------------------------------|
| `project` | string | Yes      | —       | Repo id, repo name, or vault note path   |
| `goal`    | string | No       | —       | Optional goal for relevance ranking      |

## Output Schema

```json
{
  "success": true,
  "project": "devbase",
  "repo": {
    "id": "devbase",
    "path": "~/dev/devbase",
    "language": "rust",
    "tags": ["managed", "active"],
    "stars": 42
  },
  "vault_notes": [
    { "id": "mcp-integration", "title": "MCP Integration", "source": "link" }
  ],
  "modules": [...],
  "symbols": [...],
  "calls": [...],
  "activity": [...],
  "related_symbols": [...],
  "relations": [...],
  "workflows": [...],
  "assets": [...],
  "recent_commits": [...],
  "hot_files": [...],
  "known_limits": [...],
  "skills": [...]
}
```

### Top-level fields

| Field             | Type     | Description                                         |
|-------------------|----------|-----------------------------------------------------|
| `repo`            | object?  | Repository metadata or null                         |
| `vault_notes`     | object[] | Linked and keyword-matched notes                    |
| `modules`         | object[] | High-level module structure                         |
| `symbols`         | object[] | Top code symbols (functions, structs, etc.)         |
| `calls`           | object[] | Call graph edges                                    |
| `activity`        | object[] | Recent OpLog events                                 |
| `related_symbols` | object[] | Conceptual symbol-to-symbol links                   |
| `relations`       | object[] | Knowledge-graph relations from `relations` table    |
| `workflows`       | object[] | Recent workflow executions                          |
| `assets`          | object[] | Project asset files/folders                         |
| `recent_commits`  | string[] | Recent commit messages                              |
| `hot_files`       | string[] | Recently modified files                             |
| `known_limits`    | object[] | Unmitigated known limits                            |
| `skills`          | object[] | Available devbase skills                            |

## Errors

| Error              | Cause                                    |
|--------------------|------------------------------------------|
| `project required` | Missing `project` argument               |
| No repo matched    | Substring did not match any repo         |

## Changelog

| Version | Change                                    |
|---------|------------------------------------------|
| v0.14.2 | Promoted to Stable tier                  |
| v0.20.0 | Enriched with `known_limits` and `skills` |
| v0.20.1 | Invocation test `test_tools_call_devkit_project_context` added |
