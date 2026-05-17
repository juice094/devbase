# devkit_project_brief

> **Tier**: Stable (frozen at v0.21.0)
> **Source**: `src/mcp/tools/brief.rs` — `DevkitProjectBriefTool`

Generate a Markdown project brief optimized for LLM context injection.

## Purpose

- Summarize a repository's architecture, symbols, and recent activity
- Produce a concise context document for LLM prompts
- Surface known limits, active contexts, and hot files

## When NOT to use

- Searching for specific symbols → use `devkit_code_symbols`
- Reading full source files → use filesystem tools
- Getting Git health status → use `devkit_health`

## Input Schema

```json
{
  "type": "object",
  "properties": {
    "repo_id": { "type": "string" },
    "max_tokens": { "type": "integer", "default": 2000 }
  },
  "required": ["repo_id"]
}
```

| Parameter    | Type    | Required | Default | Description                                 |
|--------------|---------|----------|---------|---------------------------------------------|
| `repo_id`    | string  | Yes      | —       | Registered repository ID                    |
| `max_tokens` | integer | No       | 2000    | Approximate token budget (1 token ~ 4 chars)|

## Output Schema

```json
{
  "success": true,
  "repo_id": "devbase",
  "brief": "# Project Brief: devbase\n\n## Overview\n- **Language**: rust\n- **Tags**: cli, rust, active\n- **Path**: `C:\\Users\\dev\\devbase`\n\n## Architecture\n- `main` (function)\n- `scan` (function)\n..."
}
```

### Brief sections (in order)

1. **Overview** — language, tags, local path
2. **Architecture** — modules (up to 20) and key symbols (up to 15)
3. **Recent Activity** — last 7 commits, hot files (14d change count)
4. **Known Limits & Tech Debt** — open known_limits entries (up to 10)
5. **Active Contexts** — linked agent contexts with memories

### Truncation behavior

If the generated brief exceeds `max_tokens * 4` characters, it is truncated at the nearest section boundary (`\n## `) with an ellipsis note.

## Errors

| Error              | Cause                                           |
|--------------------|-------------------------------------------------|
| `repo_id required` | Missing or empty `repo_id` argument             |
| `repo not found`   | `repo_id` does not exist in the registry        |

## Changelog

| Version | Change                                    |
|---------|------------------------------------------|
| v0.21.0 | Schema frozen as Stable                  |
