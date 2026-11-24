param(
    [string]$Output = "dist/coti.exe"
)
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Push-Location $root
try {
    cargo build -p md-app --release --locked
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    New-Item -ItemType Directory -Force -Path (Split-Path -Parent $Output) | Out-Null
    Copy-Item -Force "target/release/coti.exe" $Output
    $size = (Get-Item $Output).Length
    Write-Host "built $Output ($([math]::Round($size / 1MB, 2)) MB)"
} finally {
    Pop-Location
}
