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
    workspace = "wmlscript"
    mode = "release"
}

$manifest | ConvertTo-Json -Depth 4 | Set-Content -Encoding utf8 (Join-Path $releaseDir "build-manifest.json")

