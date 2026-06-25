---
type: HowToGuide
title: devbase 构建、运行与测试指南
description: 环境要求、常用命令、测试策略。
timestamp: 2026-06-25T11:15:50Z
tags: [development, build, test, howto]
---

# devbase 构建、运行与测试指南

## 环境要求

- **Rust 1.95.0+**
- 主要开发/CI 平台：**Windows**（Linux/macOS 社区支持）
- 可选：`sccache` 可显著加速 tree-sitter grammar 的 C 编译

## 常用命令

```powershell
# 构建
cargo build --release

# 本地快速体验
cargo run -- scan . --register
cargo run -- tui
cargo run -- mcp

# 测试（与 CI 一致）
cargo test --all-targets
cargo test --workspace -- --test-threads=4

# 静态检查
cargo clippy --all-targets -D warnings
cargo fmt --check

# 审计
cargo audit

# 架构不变量检查（CI 的 invariant job）
scripts/invariant-checks/run-checks.ps1
```

## 测试策略

- **单元测试**：分布在 `src/**/tests.rs` 与 `#[cfg(test)]` 块中。
- **集成测试**：`tests/cli.rs`，使用 `assert_cmd` + `tempfile`。
- **Crate 测试**：每个 `crates/*/src/*.rs` 自带测试。
- **Bench**：`criterion` 驱动的 `benches/`。
- **测试隔离**：
  - 所有 IO 测试使用 `TempDir` 与 `StorageBackend` 注入。
  - `.cargo/config.toml` 默认 `RUST_TEST_THREADS=1`；CI 使用 `--test-threads=4`。
  - `git2` 测试必须显式 `Signature::now("Test", "test@example.com")` 与 `repo.set_head("refs/heads/main")`。

## Feature 说明

```toml
default = ["tui", "mcp", "lang-rust", "lang-python", "lang-js-ts", "lang-go"]
```

- `tui`：终端仪表盘
- `mcp`：MCP Server
- `lang-*`：tree-sitter 语言支持
- `embedding`：本地 embedding（默认不包含）
- `greptimedb`：可选 GreptimeDB 写入
- `watch`：目录监控
