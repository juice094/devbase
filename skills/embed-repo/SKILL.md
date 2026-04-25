---
name: embed-repo
version: "1.0.0"
description: Generate semantic embeddings for a repository's code symbols
author: devbase-team
tags: [embedding, semantic-search, indexing]
skill_type: builtin
inputs:
  - name: repo_id
    type: string
    description: Target repository ID in devbase
    required: true
  - name: device
    type: string
    description: Device for embedding model (cpu, cuda, auto)
    default: "auto"
outputs:
  - name: status
    type: string
    description: Completion status
---
# Embed Repository

This skill invokes the local embedding provider (`tools/embedding-provider/local.py`)
to generate 384-dimensional vectors for all code symbols in the specified repository.

## Usage

```bash
devbase skill run embed-repo --arg repo_id=devbase --arg device=cuda
```

## Implementation

The entry script locates `local.py` relative to the devbase workspace and runs it
with the provided `--repo-id` and `--device` arguments.
