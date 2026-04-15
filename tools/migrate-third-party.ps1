<#
.SYNOPSIS
    Batch migrate confirmed third-party ZIP snapshots to official Git clones.
    Projects are cloned into C:\Users\<user>\dev\third_party and registered to devbase.
#>

$targetRoot = "C:\Users\<user>\dev\third_party"
New-Item -ItemType Directory -Force -Path $targetRoot | Out-Null

$repos = @(
    @{ name = "openclaw";         url = "https://github.com/openclaw/openclaw.git";              branch = "main" },
    @{ name = "lazygit";          url = "https://github.com/jesseduffield/lazygit.git";          branch = "master" },
    @{ name = "gitui";            url = "https://github.com/gitui-org/gitui.git";                branch = "master" },
    @{ name = "ollama";           url = "https://github.com/ollama/ollama.git";                  branch = "main" },
    @{ name = "dify";             url = "https://github.com/langgenius/dify.git";                branch = "main" },
    @{ name = "codex";            url = "https://github.com/openai/codex.git";                   branch = "main" },
    @{ name = "kimi-cli";         url = "https://github.com/MoonshotAI/kimi-cli.git";            branch = "main" },
    @{ name = "iroh";             url = "https://github.com/n0-computer/iroh.git";               branch = "main" },
    @{ name = "tailscale";        url = "https://github.com/tailscale/tailscale.git";            branch = "main" },
    @{ name = "vllm";             url = "https://github.com/vllm-project/vllm.git";              branch = "main" },
    @{ name = "coze-studio";      url = "https://github.com/coze-dev/coze-studio.git";           branch = "main" },
    @{ name = "nanobot";          url = "https://github.com/HKUDS/nanobot.git";                  branch = "main" },
    @{ name = "claude-code-rust"; url = "https://github.com/lorryjovens-hub/claude-code-rust.git"; branch = "main" },
    @{ name = "zeroclaw";         url = "https://github.com/zeroclaw-labs/zeroclaw.git";         branch = "master" }
)

$devbaseExe = "C:\Users\<user>\target\debug\devbase.exe"
if (-not (Test-Path $devbaseExe)) {
    Write-Host "devbase executable not found at $devbaseExe" -ForegroundColor Red
    Write-Host "Please run 'cargo build' in C:\Users\<user>\Desktop\devbase first." -ForegroundColor Yellow
    exit 1
}

foreach ($r in $repos) {
    $dest = Join-Path $targetRoot $r.name
    Write-Host "`n>>> $($r.name) :: $($r.url)" -ForegroundColor Cyan

    if (Test-Path (Join-Path $dest ".git")) {
        Write-Host "    Already cloned. Registering to devbase..." -ForegroundColor Green
    } else {
        Write-Host "    Cloning to $dest ..." -ForegroundColor Yellow
        git clone --recursive $r.url $dest
        if (-not $?) {
            Write-Host "    [ERROR] Clone failed for $($r.name)" -ForegroundColor Red
            continue
        }
    }

    # Register to devbase
    & $devbaseExe scan $dest --register | Out-Null

    # Tag as third_party reference
    & $devbaseExe tag $r.name "third-party,reference" | Out-Null

    Write-Host "    [OK] Registered and tagged." -ForegroundColor Green
}

Write-Host "`n========================================" -ForegroundColor Cyan
Write-Host "Migration complete. Registered repos:" -ForegroundColor Cyan
& $devbaseExe health --detail
