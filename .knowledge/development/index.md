---
type: ConceptIndex
title: devbase 开发规范索引
description: 构建、测试、代码风格、提交规范等开发约定。
timestamp: 2026-06-25T11:15:50Z
tags: [development, guidelines, index]
---

# devbase 开发规范索引

## 快速链接

| 主题 | 文档 |
|------|------|
| 构建与测试 | [build-and-test.md](./build-and-test.md) |
| 代码风格与提交规范 | [code-style.md](./code-style.md) |

## 提交前必须通过

```powershell
cargo test --all-targets
cargo clippy --all-targets -D warnings
cargo fmt --check
```

仓库已配置 `.githooks/pre-commit` 执行 `cargo fmt --check` 与 `cargo clippy --all-targets -- -D warnings`。
