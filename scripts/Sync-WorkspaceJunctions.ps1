# Sync-WorkspaceJunctions.ps1
# 自动维护 Obsidian Vault ↔ workspace / dev 的 NTFS Junction
# 用于 OpenClaw heartbeat 或手动执行
#
# ⚠️ 不在 workspace 内部创建 junction → dev/
#   原因: 会把 62GB 代码仓库暴露进 workspace，破坏 syncthing/备份/搜索

$ErrorActionPreference = "Stop"

$Junctions = @(
    @{
        Path   = "$env:USERPROFILE\Documents\Obsidian Vault\80-Gray"
        Target = "$env:USERPROFILE\.kimi_openclaw\workspace"
        Name   = "Obsidian/80-Gray → workspace"
    },
    @{
        Path   = "$env:USERPROFILE\Documents\Obsidian Vault\90-Code"
        Target = "$env:USERPROFILE\dev"
        Name   = "Obsidian/90-Code → dev"
    }
)

$Issues = @()
$Fixed = @()

foreach ($J in $Junctions) {
    $exists = Test-Path $J.Path
    $isJunction = (Get-Item $J.Path -Force -ErrorAction SilentlyContinue).Attributes -band [System.IO.FileAttributes]::ReparsePoint

    if (-not $exists) {
        # 缺失 → 创建
        try {
            New-Item -Path $J.Path -ItemType Junction -Target $J.Target -Force | Out-Null
            $Fixed += "CREATED: $($J.Name)"
        } catch {
            $Issues += "FAILED: $($J.Name) — $_"
        }
    } elseif (-not $isJunction) {
        # 存在但不是 Junction → 报告异常
        $Issues += "NOT-JUNCTION: $($J.Name) — exists as regular directory"
    } else {
        # 存在且是 Junction → 验证可访问性
        $reachable = Test-Path "$($J.Path)\*"
        if (-not $reachable) {
            $Issues += "BROKEN: $($J.Name) — junction exists but target unreachable"
        }
    }
}

# 输出
if ($Fixed.Count -gt 0) {
    Write-Host "=== FIXED ===" -ForegroundColor Green
    $Fixed | ForEach-Object { Write-Host "  $_" }
}

if ($Issues.Count -gt 0) {
    Write-Host "=== ISSUES ===" -ForegroundColor Yellow
    $Issues | ForEach-Object { Write-Host "  $_" }
}

if ($Fixed.Count -eq 0 -and $Issues.Count -eq 0) {
    Write-Host "All junctions healthy." -ForegroundColor Green
}

# 返回状态码供 heartbeat 判断
if ($Issues.Count -gt 0) { exit 1 } else { exit 0 }
