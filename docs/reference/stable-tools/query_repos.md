# devkit_query_repos

> **Tier**: Stable (frozen at v0.20.1)
> **Source**: `src/mcp/tools/repo.rs` — `DevkitQueryReposTool`

Query registered repositories using structured filters. This is the primary read-only discovery tool for the local workspace.

## Purpose

- List all tracked repositories
- Filter by programming language (e.g., "rust", "python", "go")
- Filter by tag (e.g., "managed", "third-party", "active")
- Filter by Git status (dirty, ahead, behind, diverged, up_to_date)
- Get paginated repo listings with metadata

## When NOT to use

- Natural language queries → use `devkit_natural_language_query`
- Full-text search across repo contents → use `devkit_index` + search tools
- Detailed health diagnostics → use `devkit_health`
- Writing or modifying repos → use `devkit_sync` or `devkit_scan`

## Input Schema

```json
{
  "type": "object",
  "properties": {
    "language": { "type": "string", "description": "Filter by programming language", "default": "" },
    "tag": { "type": "string", "description": "Filter by tag", "default": "" },
    "status": { "type": "string", "enum": ["dirty", "ahead", "behind", "diverged", "up_to_date", ""], "description": "Filter by Git status", "default": "" },
    "limit": { "type": "integer", "description": "Max results", "default": 50 }
  }
}
```

| Parameter | Type    | Required | Default | Description                          |
|-----------|---------|----------|---------|--------------------------------------|
| `language`| string  | No       | `""`    | Programming language filter          |
| `tag`     | string  | No       | `""`    | Tag filter (case-insensitive)        |
| `status`  | string  | No       | `""`    | Git status enum or empty for all     |
| `limit`   | integer | No       | `50`    | Maximum number of results            |

### Status values

| Status       | Meaning                                    |
|--------------|-------------------------------------------|
| `dirty`      | Uncommitted changes in working tree       |
| `ahead`      | Local commits not pushed, no remote ahead |
| `behind`     | Remote commits not pulled, no local ahead |
| `diverged`   | Both ahead and behind                     |
| `up_to_date` | Clean and synchronized                    |

## Output Schema

```json
{
  "success": true,
  "count": 3,
  "repos": [
    {
      "id": "devbase",
      "path": "~/dev/devbase",
      "language": "rust",
      "tags": ["managed", "active"],
      "status": { "dirty": false, "ahead": 0, "behind": 0 },
      "stars": 42
    }
  ]
}
```

| Field            | Type     | Description                              |
|------------------|----------|------------------------------------------|
| `id`             | string   | Repository identifier                    |
| `path`           | string   | Local path (home masked as `~`)          |
| `language`       | string?  | Primary programming language             |
| `tags`           | string[] | Associated tags                          |
| `status.dirty`   | boolean  | Whether working tree has changes         |
| `status.ahead`   | integer  | Commits ahead of upstream                |
| `status.behind`  | integer  | Commits behind upstream                  |
| `stars`          | integer  | GitHub stars cache                       |

## Errors

| Error                        | Cause                                    |
|------------------------------|------------------------------------------|
| Database connection failed   | SQLite locked or corrupted               |
| Filter parse error           | Invalid `status` enum value              |

## Changelog

| Version | Change                                    |
|---------|------------------------------------------|
| v0.14.2 | Promoted to Stable tier                  |
| v0.20.1 | Invocation test `test_tools_call_devkit_query_repos` added |
