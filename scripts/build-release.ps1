param(
    [string]$OutputRoot = "releases"
)

$ErrorActionPreference = "Stop"

New-Item -ItemType Directory -Force -Path $OutputRoot | Out-Null

cargo build --workspace --release

$stamp = Get-Date -Format "yyyyMMdd-HHmmss"
$releaseDir = Join-Path $OutputRoot $stamp
New-Item -ItemType Directory -Force -Path $releaseDir | Out-Null

$manifest = [ordered]@{
    built_at = (Get-Date).ToString("o")
    workspace = "wmscript"
    mode = "release"
}

$manifest | ConvertTo-Json -Depth 4 | Set-Content -Encoding utf8 (Join-Path $releaseDir "build-manifest.json")

$targetReleaseDir = $null
if ($env:CARGO_BUILD_TARGET_DIR) {
    $candidate = Join-Path $env:CARGO_BUILD_TARGET_DIR "release"
    if (Test-Path $candidate) {
        $targetReleaseDir = $candidate
    }
}

if (-not $targetReleaseDir) {
    $candidate = Join-Path "target" "release"
    if (Test-Path $candidate) {
        $targetReleaseDir = $candidate
    }
}

if (-not $targetReleaseDir) {
    throw "release artifacts not found. expected target/release or CARGO_BUILD_TARGET_DIR/release"
}

$binaries = Get-ChildItem -Path $targetReleaseDir -Filter "*.exe" -File -ErrorAction Stop
if (-not $binaries) {
    throw "no .exe files found under $targetReleaseDir"
}

Copy-Item -Path (Join-Path $targetReleaseDir "*.exe") -Destination $releaseDir -Force
Write-Host "release artifacts copied to $releaseDir"