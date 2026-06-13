## Summary

<!-- 必填：用一句话说明这个 PR 做了什么，解决了什么问题 -->

## Motivation / Context

<!-- 为什么需要这个改动？关联的 issue、讨论或用户场景是什么？ -->

## Type of Change

- [ ] Bug fix (non-breaking)
- [ ] New feature
- [ ] Breaking change
- [ ] Documentation only
- [ ] Performance improvement
- [ ] Refactoring (no behavior change)
- [ ] Test-only change
- [ ] Build / CI / tooling

## Testing

<!-- 如何验证这个改动？列出你运行的命令和结果 -->

```bash
# 本地验证命令示例
cargo test --all-targets
cargo clippy --all-targets -D warnings
cargo fmt --check
scripts/invariant-checks/run-checks.ps1   # Windows
```

- [ ] `cargo test --all-targets` passes locally
- [ ] `cargo clippy --all-targets -D warnings` passes
- [ ] `cargo fmt --check` passes
- [ ] New code has no production `unwrap`/`expect`/`panic` (test code exempt)
- [ ] Schema changes include migration in `src/registry/migrate.rs` **and** `src/registry/test_helpers.rs`
- [ ] New MCP tools include tests in `src/mcp/tests.rs`
- [ ] README / AGENTS.md / docs/README.md updated if user-facing behavior changed
- [ ] `scripts/invariant-checks/run-checks.ps1` passes (Windows)

## Breaking Changes / Migration Notes

<!-- 如果这是 Breaking change，说明用户/下游需要如何迁移。无则填 "None" -->

## Related Issues

<!-- Fixes #123, Closes #456, Related to #789 -->

## Additional Notes

<!-- 其他需要审阅者知道的信息：设计取舍、已知限制、截图等 -->
