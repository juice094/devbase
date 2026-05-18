# Stable Tools Reference

Tools in this directory have **frozen schemas** as of devbase v0.21.0.
Breaking changes require a major version bump and a deprecation cycle.

| Tool | Purpose | File |
|------|---------|------|
| [`devkit_health`](health.md) | Check Git health (dirty/ahead/behind) of all registered repos | `repo.rs` |
| [`devkit_project_brief`](project_brief.md) | Generate a Markdown project brief for LLM context injection | `brief.rs` |
| [`devkit_hybrid_search`](hybrid_search.md) | Vector + keyword RRF search for code symbols | `search.rs` |
| [`devkit_vault_search`](vault_search.md) | Keyword search across Vault notes (titles, tags, content) | `vault.rs` |
| [`devkit_session_recall`](session_recall.md) | Semantic memory recall by embedding similarity | `session.rs` |

## Schema stability guarantee

- Input JSON Schema: frozen — no required fields added/removed without deprecation
- Output JSON structure: frozen — fields may be added but never removed or retyped
- Semantic behavior: frozen — matching logic, fallback behavior, and error modes are stable

## Changelog

| Version | Change                                    |
|---------|------------------------------------------|
| v0.21.0 | 5 tools promoted to Stable; schemas frozen |
