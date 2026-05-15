#!/usr/bin/env pwsh
# devbase 本地 CI 预检脚本（Windows PowerShell）
# 功能对等 GitHub Actions 的 Required Checks

$ErrorActionPreference = "Stop"

Write-Host "=== devbase Local CI ===" -ForegroundColor Cyan

Write-Host "`n[1/6] cargo fmt --check" -ForegroundColor Yellow
& cargo fmt --check
if ($LASTEXITCODE -ne 0) { throw "fmt failed" }

Write-Host "`n[2/6] cargo clippy --all-targets -- -D warnings" -ForegroundColor Yellow
& cargo clippy --all-targets -- -D warnings
if ($LASTEXITCODE -ne 0) { throw "clippy failed" }

Write-Host "`n[3/6] cargo check" -ForegroundColor Yellow
& cargo check
if ($LASTEXITCODE -ne 0) { throw "check failed" }

Write-Host "`n[4/6] cargo test --workspace -- --test-threads=4" -ForegroundColor Yellow
& cargo test --workspace -- --test-threads=4
if ($LASTEXITCODE -ne 0) { throw "test failed" }

Write-Host "`n[5/6] cargo check --features greptimedb" -ForegroundColor Yellow
& cargo check --features greptimedb
if ($LASTEXITCODE -ne 0) { throw "greptimedb feature check failed" }

Write-Host "`n[6/6] cargo audit" -ForegroundColor Yellow
& cargo audit
if ($LASTEXITCODE -ne 0) { Write-Warning "audit found issues (non-blocking)" }

Write-Host "`n=== All local checks passed ===" -ForegroundColor Green
