# Stable Tools Reference

Tools in this directory have **frozen schemas** as of devbase v0.20.1.
Breaking changes require a major version bump and a deprecation cycle.

| Tool | Purpose | File | Test Coverage |
|------|---------|------|---------------|
| [`devkit_health`](health.md) | Check Git health (dirty/ahead/behind) of all registered repos | `repo.rs` | `test_tools_call_devkit_health` |
| [`devkit_query_repos`](query_repos.md) | Query registered repos with language/tag/status filters | `repo.rs` | `test_tools_call_devkit_query_repos` |
| [`devkit_vault_search`](vault_search.md) | Keyword search across Vault notes (titles, tags, content) | `vault.rs` | `test_tools_call_devkit_vault_search` |
| [`devkit_vault_read`](vault_read.md) | Read full content of a Vault note including frontmatter | `vault.rs` | `test_tools_call_devkit_vault_read` |
| [`devkit_project_context`](project_context.md) | Unified project snapshot (repo + vault + symbols + relations + limits + skills) | `context.rs` | `test_tools_call_devkit_project_context` |

## Schema stability guarantee

- Input JSON Schema: frozen — no required fields added/removed without deprecation
- Output JSON structure: frozen — fields may be added but never removed or retyped
- Semantic behavior: frozen — matching logic, fallback behavior, and error modes are stable

## Changelog

| Version | Change |
|---------|--------|
| v0.20.1 | 5 Stable tools verified with dedicated invocation tests |
| v0.20.0 | `project_context` enriched with `known_limits` and `skills` |
| v0.14.2 | 5 tools promoted to Stable tier |
