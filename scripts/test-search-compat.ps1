param(
    [ValidateSet('FemtoVG', 'Software')][string]$Renderer = 'Software',
    [switch]$GoOnly
)
$ErrorActionPreference = 'Stop'
$repository = Split-Path -Parent $PSScriptRoot
if ($env:VELO_RS09_REFRESH) { throw 'Unset VELO_RS09_REFRESH: verification never regenerates expected outputs.' }
Push-Location $repository
try {
    & go run ./cmd/rs09-tables check
    if ($LASTEXITCODE -ne 0) { throw 'RS-09 Unicode/pinyin tables differ from the Go baseline.' }
    & go test ./internal/app -run '^TestRS09' -count=1
    if ($LASTEXITCODE -ne 0) { throw 'RS-09 Go search/history/home oracle failed.' }
    if (-not $GoOnly) {
        . (Join-Path $PSScriptRoot 'lib/rust-environment.ps1')
        $rs09Environment = Initialize-VeloRustEnvironment
        Push-Location $rs09Environment.RustRoot
        try {
            & cargo test --locked --no-default-features --features "renderer-$($Renderer.ToLowerInvariant())" --test behavior_contract --test history_library
            if ($LASTEXITCODE -ne 0) { throw 'RS-09 Rust behavioral differential failed.' }
        } finally { Pop-Location }
    }
    Write-Output 'RS-09 search/history/home Go contract passed (Rust covered here or by the full verification caller).'
} finally { Pop-Location }
