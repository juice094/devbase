#!/usr/bin/env pwsh
# SPDX-License-Identifier: MIT
# Copyright (c) 2026 juice094
# devbase Architecture Invariant CI Checks
# Run from repo root: scripts/invariant-checks/run-checks.ps1

$ErrorActionPreference = "Stop"
$script:Failed = 0
$script:Passed = 0
$script:Warnings = 0

function Write-CheckHeader($name) {
    Write-Host "`n==> $name" -ForegroundColor Cyan
}

function Report($status, $message) {
    if ($status -eq "PASS") {
        $script:Passed++
        Write-Host "  [PASS] $message" -ForegroundColor Green
    } elseif ($status -eq "WARN") {
        $script:Warnings++
        Write-Host "  [WARN] $message" -ForegroundColor Yellow
    } else {
        $script:Failed++
        Write-Host "  [FAIL] $message" -ForegroundColor Red
    }
}

# --- Helper: compute line ranges of #[cfg(test)] blocks in a file ---
function Get-TestLineRanges($filePath) {
    $ranges = @()
    if (-not (Test-Path $filePath)) { return $ranges }
    $lines = Get-Content $filePath
    $inTest = $false
    $testStart = -1
    $depth = 0
    for ($i = 0; $i -lt $lines.Count; $i++) {
        $line = $lines[$i]
        if (-not $inTest -and $line -match '^\s*#\[cfg\(test\)\]\s*$') {
            $inTest = $true
            $testStart = $i
            $depth = 0
            continue
        }
        if ($inTest) {
            $open = ([regex]::Matches($line, '\{')).Count
            $close = ([regex]::Matches($line, '\}')).Count
            $depth += $open - $close
            if ($depth -le 0 -and ($i -gt $testStart)) {
                $ranges += @{Start = $testStart; End = $i}
                $inTest = $false
            }
        }
    }
    if ($inTest) {
        $ranges += @{Start = $testStart; End = $lines.Count - 1}
    }
    return $ranges
}

function Is-LineInTestRange($lineNum, $ranges) {
    foreach ($r in $ranges) {
        if ($lineNum -ge $r.Start -and $lineNum -le $r.End) {
            return $true
        }
    }
    return $false
}

# --- G5: RF-6 — Detect NEW unwrap/expect/panic in production code ---
Write-CheckHeader "G5: RF-6 new unwrap/expect/panic detection"

$diffFiles = cmd /c "git diff --name-only origin/main 2>nul"
if (-not $diffFiles) {
    Report "PASS" "No changes since origin/main"
} else {
    $newViolations = @()
    foreach ($file in $diffFiles -split "`n") {
        if ($file -notmatch '\.rs$') { continue }
        if ($file -match 'tests?/|tests\.rs$|_test\.rs$|benches/|examples/') { continue }

        # Get test line ranges for the file
        $testRanges = Get-TestLineRanges $file

        $diff = cmd /c "git diff -U0 origin/main -- `"$file`" 2>nul"
        $lines = $diff -split "`n"
        $currentLine = -1
        for ($i = 0; $i -lt $lines.Count; $i++) {
            $line = $lines[$i]
            # Parse hunk header to get base line number
            if ($line -match '^@@ -(\d+)(?:,\d+)? \+(\d+)(?:,\d+)? @@') {
                $currentLine = [int]$Matches[2]
                continue
            }
            if ($line -match '^@@') { continue }
            if ($line -match '^(diff|index|---|\+\+\+)') { $currentLine = -1; continue }
            if ($currentLine -lt 0) { continue }

            if ($line.StartsWith('+') -and -not $line.StartsWith('+++')) {
                $added = $line.Substring(1)
                if ($added -match '^\s*//') { $currentLine++; continue }

                # Check if this line is inside a cfg(test) block
                $lineNum = $currentLine - 1  # convert to 0-based index
                if ($testRanges.Count -gt 0 -and (Is-LineInTestRange $lineNum $testRanges)) {
                    $currentLine++
                    continue
                }

                if ($added -match '(?<!\w)unwrap\(\)') {
                    $newViolations += "$file`:L$currentLine`: $added"
                }
                if ($added -match '(?<!\w)expect\s*\(') {
                    $newViolations += "$file`:L$currentLine`: $added"
                }
                if ($added -match '(?<!\w)panic!\s*\(') {
                    $newViolations += "$file`:L$currentLine`: $added"
                }
                $currentLine++
            } elseif ($line.StartsWith('-')) {
                # Deleted line, don't advance line number in new file
            } else {
                $currentLine++
            }
        }
    }

    if ($newViolations.Count -eq 0) {
        Report "PASS" "No new production unwrap/expect/panic in diff since origin/main"
    } else {
        Report "FAIL" "Found $($newViolations.Count) new production unwrap/expect/panic:"
        foreach ($v in $newViolations | Select-Object -First 10) {
            Write-Host "      $v" -ForegroundColor DarkYellow
        }
        if ($newViolations.Count -gt 10) {
            Write-Host "      ... and $($newViolations.Count - 10) more" -ForegroundColor DarkYellow
        }
    }
}

# --- T11: mcp/tools must not use rusqlite::Connection directly ---
Write-CheckHeader "T11: mcp/tools direct rusqlite::Connection check"

$knownT11Exceptions = @(
    "src/mcp/tools/repo.rs",
    "src/mcp/tools/repo/nl_query.rs",
    "src/mcp/tools/brief.rs",
    "src/mcp/tools/impact.rs"
)

$mcpFiles = Get-ChildItem -Recurse -File -Path src/mcp/tools -Filter "*.rs" -ErrorAction SilentlyContinue
$t11Violations = @()
foreach ($file in $mcpFiles) {
    $relPath = $file.FullName.Replace("$PWD\", "").Replace("\", "/")
    $content = Get-Content $file.FullName -Raw
    if ($content -match 'rusqlite::Connection') {
        if ($knownT11Exceptions -contains $relPath) {
            Report "WARN" "$relPath`: known exception (legacy)"
        } else {
            $t11Violations += $relPath
        }
    }
}

if ($t11Violations.Count -eq 0) {
    Report "PASS" "No new direct rusqlite::Connection usage in mcp/tools"
} else {
    Report "FAIL" "Found new direct rusqlite::Connection usage in mcp/tools:"
    foreach ($v in $t11Violations) {
        Write-Host "      $v" -ForegroundColor DarkYellow
    }
}

# --- T12: tui/render must be pure consumer (no registry/rusqlite writes) ---
Write-CheckHeader "T12: tui/render pure consumer check"

$renderFiles = Get-ChildItem -Recurse -File -Path src/tui/render -Filter "*.rs" -ErrorAction SilentlyContinue
$t12Violations = @()
foreach ($file in $renderFiles) {
    $testRanges = Get-TestLineRanges $file.FullName
    $lines = Get-Content $file.FullName
    for ($i = 0; $i -lt $lines.Count; $i++) {
        if ($testRanges.Count -gt 0 -and (Is-LineInTestRange $i $testRanges)) { continue }
        $line = $lines[$i]
        if ($line -match '^\s*//') { continue }
        if ($line -match '\.execute\s*\(' -or
            $line -match '\.prepare\s*\(' -or
            $line -match 'registry::.*save' -or
            $line -match 'registry::.*insert' -or
            $line -match 'registry::.*update' -or
            $line -match 'registry::.*delete') {
            $t12Violations += "$($file.Name):$($i+1)`: $line"
        }
    }
}

if ($t12Violations.Count -eq 0) {
    Report "PASS" "tui/render is pure consumer (no writes in production code)"
} else {
    Report "FAIL" "Found write operations in tui/render production code:"
    foreach ($v in $t12Violations) {
        Write-Host "      $v" -ForegroundColor DarkYellow
    }
}

# --- Module extraction drill check ---
Write-CheckHeader "Module extraction drill check"

$extractionCandidates = @(
    @{Name="devbase-embedding"; Path="crates/devbase-embedding"},
    @{Name="devbase-registry-health"; Path="crates/devbase-registry-health"}
)

$extractOk = $true
foreach ($mod in $extractionCandidates) {
    if (Test-Path $mod.Path) {
        $hasReadme = Test-Path "$($mod.Path)/README.md"
        $hasCargoToml = Test-Path "$($mod.Path)/Cargo.toml"
        if ($hasReadme -and $hasCargoToml) {
            Write-Host "  $($mod.Name): README + Cargo.toml present" -ForegroundColor Green
        } elseif ($hasCargoToml) {
            Write-Host "  $($mod.Name): Cargo.toml present, README.md MISSING" -ForegroundColor Yellow
            $extractOk = $false
        } else {
            Write-Host "  $($mod.Name): MISSING Cargo.toml" -ForegroundColor Red
            $extractOk = $false
        }
    }
}

if ($extractOk) {
    Report "PASS" "All extracted modules have README + Cargo.toml"
} else {
    Report "WARN" "Some extracted modules missing README.md (not blocking)"
}

# --- Summary ---
Write-Host "`n========================================" -ForegroundColor White
Write-Host "Invariant Checks Complete" -ForegroundColor White
Write-Host "  Passed:   $script:Passed" -ForegroundColor Green
Write-Host "  Warnings: $script:Warnings" -ForegroundColor Yellow
Write-Host "  Failed:   $script:Failed" -ForegroundColor $(if ($script:Failed -gt 0) { "Red" } else { "Green" })
Write-Host "========================================" -ForegroundColor White

exit $script:Failed
