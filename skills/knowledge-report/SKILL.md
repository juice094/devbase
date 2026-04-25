---
name: knowledge-report
version: "1.0.0"
description: Generate a structured knowledge report for a repository
author: devbase-team
tags: [report, knowledge, documentation]
skill_type: builtin
inputs:
  - name: repo_id
    type: string
    description: Target repository ID
    required: true
outputs:
  - name: report
    type: markdown
    description: Generated knowledge report in markdown
---
# Knowledge Report

Generates a comprehensive report about a repository including:
- Registered metadata (language, tags, tier)
- Code metrics (LOC, file count)
- Top-level module structure
- Recent health status

## Usage

```bash
devbase skill run knowledge-report --arg repo_id=devbase
```
