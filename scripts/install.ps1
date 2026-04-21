# devbase Quick Install Script (Windows)
# Usage: irm https://raw.githubusercontent.com/juice094/devbase/main/scripts/install.ps1 | iex

$ErrorActionPreference = "Stop"

function Write-Info($msg) { Write-Host "[devbase] $msg" -ForegroundColor Cyan }
function Write-Ok($msg)   { Write-Host "[devbase] $msg" -ForegroundColor Green }
function Write-Warn($msg) { Write-Host "[devbase] $msg" -ForegroundColor Yellow }

# 1. Check Rust / cargo
$cargo = Get-Command cargo -ErrorAction SilentlyContinue
if (-not $cargo) {
    Write-Warn "Rust (cargo) not found."
    Write-Host "Please install Rust first: https://rustup.rs/"
    Write-Host "Then re-run this script."
    exit 1
}
Write-Ok "Found cargo at $($cargo.Source)"

# 2. Determine install method
$installFromSource = $true
$repoUrl = "https://github.com/juice094/devbase.git"
$installDir = "$env:USERPROFILE\.devbase\src"
$binDir = "$env:USERPROFILE\.devbase\bin"

# 3. Clone or update source
if (Test-Path "$installDir\.git") {
    Write-Info "Updating existing source..."
    Set-Location $installDir
    git pull --quiet
} else {
    Write-Info "Cloning devbase repository..."
    New-Item -ItemType Directory -Path $installDir -Force | Out-Null
    git clone --depth 1 $repoUrl $installDir --quiet
}

# 4. Build release binary
Write-Info "Building devbase (release mode)..."
Set-Location $installDir
cargo build --release 2>&1 | ForEach-Object {
    if ($_ -match "error|warning:") { Write-Host $_ }
}

# 5. Install binary to bin dir
New-Item -ItemType Directory -Path $binDir -Force | Out-Null
$srcBin = "$installDir\target\release\devbase.exe"
$dstBin = "$binDir\devbase.exe"
Copy-Item -Path $srcBin -Destination $dstBin -Force
Write-Ok "Installed binary to $dstBin"

# 6. Add to PATH if not present
$currentPath = [Environment]::GetEnvironmentVariable("Path", "User")
if ($currentPath -notlike "*$binDir*") {
    [Environment]::SetEnvironmentVariable("Path", "$currentPath;$binDir", "User")
    Write-Ok "Added $binDir to user PATH"
    Write-Warn "Please restart your terminal for PATH changes to take effect."
} else {
    Write-Ok "bin directory already in PATH"
}

# 7. Verify
& $dstBin --version
Write-Ok "devbase installation complete!"
Write-Host ""
Write-Host "Quick start:"
Write-Host "  devbase scan .          # scan for repos"
Write-Host "  devbase tui             # launch TUI"
Write-Host "  devbase mcp             # start MCP server (stdio)"
