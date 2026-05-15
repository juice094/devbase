#!/usr/bin/env bash
# devbase 本地 CI 预检脚本（WSL / Git Bash / Linux）
# 功能对等 GitHub Actions 的 Required Checks

set -euo pipefail

echo "=== devbase Local CI ==="

echo ""
echo "[1/6] cargo fmt --check"
cargo fmt --check

echo ""
echo "[2/6] cargo clippy --all-targets -- -D warnings"
cargo clippy --all-targets -- -D warnings

echo ""
echo "[3/6] cargo check"
cargo check

echo ""
echo "[4/6] cargo test --workspace -- --test-threads=4"
cargo test --workspace -- --test-threads=4

echo ""
echo "[5/6] cargo check --features greptimedb"
cargo check --features greptimedb

echo ""
echo "[6/6] cargo audit"
cargo audit || echo "warning: audit found issues (non-blocking)"

echo ""
echo "=== All local checks passed ==="
