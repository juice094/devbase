## Summary

<!-- One-line description of what this PR does -->

## Type of Change

- [ ] Bug fix (non-breaking)
- [ ] New feature
- [ ] Breaking change
- [ ] Documentation
- [ ] Performance improvement
- [ ] Refactoring (no behavior change)

## Checklist

- [ ] `cargo test --all-targets` passes locally
- [ ] `cargo clippy --all-targets -D warnings` passes
- [ ] `cargo fmt --check` passes
- [ ] New code has no production `unwrap`/`expect`/`panic` (test code exempt)
- [ ] Schema changes include migration in `src/registry/migrate.rs`
- [ ] New MCP tools include tests in `src/mcp/tests.rs`
- [ ] README / AGENTS.md updated if user-facing behavior changed

## Related Issues

<!-- Link to related issues: Fixes #123, Closes #456 -->
