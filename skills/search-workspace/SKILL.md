---
name: search-workspace
version: "1.0.0"
description: Search across all registered repositories using hybrid (vector + keyword) search
author: devbase-team
tags: [search, hybrid, cross-repo]
skill_type: builtin
inputs:
  - name: query
    type: string
    description: Search query text
    required: true
  - name: limit
    type: integer
    description: Maximum results
    default: "10"
  - name: repo_id
    type: string
    description: Optional specific repo to search (omit for cross-repo)
outputs:
  - name: results
    type: json
    description: Matching symbols with scores
---
# Search Workspace

Performs hybrid search combining semantic vector similarity with keyword matching
via Reciprocal Rank Fusion (RRF).

## Usage

```bash
devbase skill run search-workspace --arg query="authentication middleware"
devbase skill run search-workspace --arg query="auth" --arg repo_id=devbase --arg limit=5
```
