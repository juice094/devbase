# devkit_health

> **Tier**: Stable (frozen at v0.21.0)
> **Source**: `src/mcp/tools/repo.rs` — `DevkitHealthTool`

Check the health status of all registered repositories. Read-only diagnostic tool.

## Purpose

- Get an overview of all tracked repos and their Git status
- Identify repos that are dirty, ahead, behind, or diverged
- Check environment prerequisites (Rust, Go, Node.js, CMake versions)
- Find repos that need attention before a sync

## When NOT to use

- Pulling or pushing changes → use `devkit_sync`
- Searching repos by language or tag → use `devkit_query_repos`
- Scanning new directories → use `devkit_scan`

## Input Schema

```json
{
  "type": "object",
  "properties": {
    "detail": {
      "type": "boolean",
      "description": "Show detailed per-repo status",
      "default": false
    }
  }
}
```

| Parameter | Type    | Required | Default | Description                          |
|-----------|---------|----------|---------|--------------------------------------|
| `detail`  | boolean | No       | `false` | If true, returns per-repo Git status |

## Output Schema

### Summary mode (`detail: false`)

```json
{
  "success": true,
  "summary": {
    "total_repos": 12,
    "dirty_repos": 2,
    "behind_upstream": 3,
    "no_upstream": 1
  },
  "environment": {
    "rustc": "1.85.0",
    "cargo": "1.85.0",
    "node": "22.14.0",
    "go": "go1.24.2",
    "cmake": "3.31.6",
    "python": "3.13.3",
    "bun": "1.2.10",
    "zig": "0.14.0",
    "java": "21.0.6"
  }
}
```

### Detail mode (`detail: true`)

```json
{
  "success": true,
  "summary": { "total_repos": 12, "dirty_repos": 2, "behind_upstream": 3, "no_upstream": 1 },
  "environment": { "rustc": "1.85.0", ... },
  "repos": [
    {
      "id": "devbase",
      "local_path": "C:\\Users\\dev\\devbase",
      "upstream_url": "https://github.com/user/devbase",
      "default_branch": "main",
      "status": "dirty",
      "ahead": 0,
      "behind": 0,
      "workspace_type": "git",
      "data_tier": "private"
    }
  ]
}
```

### Repo status values

| Status       | Meaning                                    |
|--------------|-------------------------------------------|
| `ok`         | Clean, up to date with upstream           |
| `dirty`      | Uncommitted changes in working tree       |
| `ahead`      | Local commits not pushed                  |
| `behind`     | Remote commits not pulled                 |
| `diverged`   | Both ahead and behind                     |
| `no_upstream`| No remote configured                      |
| `error`      | Git repository could not be opened        |
| `detached`   | HEAD is detached                          |

## Errors

| Error                        | Cause                                    |
|------------------------------|------------------------------------------|
| Database connection failed   | SQLite locked or corrupted               |
| Git repository unreadable    | Path no longer exists or permissions     |

## Changelog

| Version | Change                                    |
|---------|------------------------------------------|
| v0.21.0 | Schema frozen as Stable                  |
