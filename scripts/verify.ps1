# One command: clean clone -> build -> full test matrix.
#
# Proves the repo works from nothing but git, cargo, node, and python --
# not "works on my machine". By default this clones a fresh copy into a
# temp directory and builds/tests THAT, so nothing in your existing working
# tree (uncommitted changes, stale build artifacts, local-only fixes) can
# make the result look better than what a judge cloning the repo actually
# gets.
#
# Usage:
#   .\scripts\verify.ps1                    # clean clone of the current branch's HEAD commit
#   .\scripts\verify.ps1 -Ref <branch>       # clean clone of a specific branch/tag/commit
#   .\scripts\verify.ps1 -InPlace            # skip cloning, test the current working tree instead
#   .\scripts\verify.ps1 -SkipPython         # skip the channel-simulator Python matrix
#
# Exits 0 iff every step passed. On failure, the temp clone (if any) is left
# in place and its path is printed, so you can inspect what broke.

param(
    [string]$Ref = "",
    [switch]$InPlace,
    [switch]$SkipPython
)

$ErrorActionPreference = "Continue"
Set-Location (Join-Path $PSScriptRoot "..")
$RepoRoot = (Get-Location).Path
$RepoUrl = (git remote get-url origin 2>$null)
if (-not $RepoUrl) { $RepoUrl = $RepoRoot }
if (-not $Ref) { $Ref = (git rev-parse HEAD) }

$StepsPassed = New-Object System.Collections.Generic.List[string]
$StepsFailed = New-Object System.Collections.Generic.List[string]

function Run-Step {
    param([string]$Name, [string]$Exe, [string[]]$ExeArgs)
    Write-Host ""
    Write-Host "=== $Name ==="
    Write-Host "+ $Exe $($ExeArgs -join ' ')"
    & $Exe @ExeArgs
    if ($LASTEXITCODE -eq 0) {
        $StepsPassed.Add($Name)
    } else {
        $StepsFailed.Add($Name)
        Write-Host ">>> FAILED: $Name" -ForegroundColor Red
    }
}

if ($InPlace) {
    $WorkDir = $RepoRoot
    Write-Host "Testing in place: $WorkDir (not a clean-clone proof)"
} else {
    $WorkDir = Join-Path ([System.IO.Path]::GetTempPath()) ("stegstr-verify-" + [System.Guid]::NewGuid().ToString("N").Substring(0,8))
    Write-Host "Clean clone of $RepoUrl @ $Ref into: $WorkDir"
    git clone --quiet $RepoUrl $WorkDir
    if ($LASTEXITCODE -ne 0) { Write-Host "clone failed" -ForegroundColor Red; exit 1 }
    git -C $WorkDir checkout --quiet $Ref
    if ($LASTEXITCODE -ne 0) { Write-Host "checkout of ref '$Ref' failed" -ForegroundColor Red; exit 1 }
}

Set-Location $WorkDir

Run-Step "Rust: release build (stegstr-cli)" "cargo" @("build", "--release", "--bin", "stegstr-cli", "--manifest-path", "src-tauri/Cargo.toml")
Run-Step "Rust: cargo test --release" "cargo" @("test", "--release", "--manifest-path", "src-tauri/Cargo.toml")
Run-Step "Rust: cargo clippy -- -D warnings" "cargo" @("clippy", "--release", "--all-targets", "--manifest-path", "src-tauri/Cargo.toml", "--", "-D", "warnings")

if (Get-Command npm -ErrorAction SilentlyContinue) {
    Run-Step "Frontend: npm install" "npm" @("install", "--no-audit", "--no-fund")
    Run-Step "Frontend: npm test" "npm" @("test")
} else {
    Write-Host ""
    Write-Host "=== Frontend tests skipped: npm not found ==="
    $StepsFailed.Add("Frontend: npm not found")
}

if (-not $SkipPython) {
    $Py = $null
    if (Get-Command python -ErrorAction SilentlyContinue) { $Py = "python" }
    elseif (Get-Command python3 -ErrorAction SilentlyContinue) { $Py = "python3" }

    if ($Py) {
        Push-Location channel_simulator
        Run-Step "Python: pip install -r requirements.txt" $Py @("-m", "pip", "install", "-q", "-r", "requirements.txt")
        Run-Step "Python: generate realistic covers" $Py @("gen_realistic_covers.py")
        Run-Step "Python: generate extended covers" $Py @("gen_extended_covers.py")
        Run-Step "Python: run_matrix_rust_cli.py (45/45 expected)" $Py @("run_matrix_rust_cli.py")
        Run-Step "Python: run_matrix_realistic.py (prototype matrix)" $Py @("run_matrix_realistic.py")
        Pop-Location
    } else {
        Write-Host ""
        Write-Host "=== Python matrix skipped: no python/python3 found ==="
        $StepsFailed.Add("Python: interpreter not found")
    }
} else {
    Write-Host ""
    Write-Host "=== Python matrix skipped: -SkipPython ==="
}

Write-Host ""
Write-Host "==================== SUMMARY ===================="
foreach ($s in $StepsPassed) { Write-Host "PASS  $s" -ForegroundColor Green }
foreach ($s in $StepsFailed) { Write-Host "FAIL  $s" -ForegroundColor Red }
Write-Host "$($StepsPassed.Count) passed, $($StepsFailed.Count) failed"

if (-not $InPlace) {
    if ($StepsFailed.Count -eq 0) {
        Set-Location $RepoRoot
        Remove-Item -Recurse -Force $WorkDir -ErrorAction SilentlyContinue
    } else {
        Write-Host "Clean clone left at: $WorkDir (for inspection)"
    }
}

if ($StepsFailed.Count -eq 0) { exit 0 } else { exit 1 }
