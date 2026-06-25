---
name: skillopt-sleep-kimi
version: "1.0.0"
description: >
  Use when the user wants their Kimi CLI agent to self-improve from past usage,
  asks about a nightly/offline "sleep" or "dream" cycle, memory/skill consolidation,
  or says things like "make my agent better the more I use it", "review my past sessions",
  "learn my preferences", "consolidate what you learned", "run the sleep cycle",
  or wants to schedule offline self-optimization.
author: devbase-team
tags: [skillopt, self-improvement, memory, kimicli, sleep-cycle]
skill_type: custom
inputs:
  - name: scope
    type: string
    description: Scope of the sleep cycle (current-project | all)
    default: "current-project"
    required: false
  - name: backend
    type: string
    description: Backend for offline replay (mock | local | remote)
    default: "mock"
    required: false
outputs:
  - name: report
    type: markdown
    description: Summary of harvested sessions, mined tasks, and staged proposals
---

# SkillOpt-Sleep for Kimi CLI: offline self-evolution

This skill gives the user's Kimi CLI agent a **sleep cycle**. While the user is
offline (or on demand), it reviews real past sessions, re-runs recurring tasks,
and consolidates what it learns into **memory** and **skills** — but only keeps
changes that pass a held-out validation gate, and only after the user adopts them.

## When to activate

Trigger when the user wants any of:

- "make my agent learn from how I use it" / "get better the more I use it"
- a nightly/scheduled or on-demand **offline self-improvement / dream / sleep** run
- to **review past sessions/trajectories** and distill recurring tasks
- to **consolidate** feedback into devbase Vault notes or managed skills
- to run `status`, `harvest`, `dry-run`, `run`, or `adopt` for SkillOpt-Sleep

## The cycle

1. **Harvest** — use `devkit_session_recall` and `devkit_vault_history` to gather
   recent sessions and note changes (read-only).
2. **Mine** — turn session digests into recurring `TaskRecord`s with outcomes and
   checkable references where possible.
3. **Replay** — re-run mined tasks offline under the current skill and memory.
4. **Consolidate** — reflect on failures and propose bounded edits to Vault notes
   or skills.
5. **Gate** — accept edits only when a held-out validation score improves.
6. **Stage** — write the proposal under
   `<project>/.skillopt-sleep/staging/<date>/`; nothing live changes.
7. **Adopt** — only after explicit user approval, copy staged files over live
   files with backups.

## How to drive it

Because Kimi CLI does not have a plugin script model like Claude Code, the cycle
is driven through devbase MCP tools and shell commands:

```bash
# 1. Check current status
python -m skillopt_sleep status --project "$(pwd)"

# 2. Safe deterministic preview (no API spend)
python -m skillopt_sleep dry-run --project "$(pwd)" --backend mock

# 3. Run full cycle and stage a proposal
python -m skillopt_sleep run --project "$(pwd)" --backend local

# 4. Adopt staged proposal after review
python -m skillopt_sleep adopt --project "$(pwd)"
```

For context gathering, also use:

- `devkit_session_recall` — find relevant past sessions.
- `devkit_vault_search` — locate existing memory/skills related to the mined tasks.
- `devkit_knowledge_report` — summarize the current project state before proposing edits.

## Hard rules

- **Never** hand-edit the user's `AGENTS.md`, Vault notes, or skills as part of this skill.
  Only the `adopt` action changes live files, and it backs them up first.
- Harvest is read-only. `mock` replay has no side effects.
- Always show the user the **held-out baseline → candidate** score and the
  exact proposed edits before suggesting adoption. Evidence before adoption.
- If asked whether it really helps, run the deterministic demo:
  ```bash
  python -m skillopt_sleep.experiments.run_experiment --persona researcher --assert-improves
  ```

## Validate / demo

```bash
# deterministic proof (no API): held-out score rises, gate blocks regressions
python -m skillopt_sleep.experiments.run_experiment --persona researcher --assert-improves
python -m skillopt_sleep.experiments.run_experiment --persona programmer  --assert-improves
```

See the SkillOpt-Sleep guide for recorded output and
`docs/superpowers/specs/2026-06-07-skillopt-sleep-claude-code-plugin-design.md`
for the full design.
