param([ValidateSet('FemtoVG', 'Software')][string]$Renderer = 'Software', [switch]$GoOnly)
$ErrorActionPreference = 'Stop'
$repository = Split-Path -Parent $PSScriptRoot
Push-Location $repository
try {
    & go run ./cmd/rs10-compat check
    if ($LASTEXITCODE -ne 0) { throw 'RS-10 discovery/filter/action Go oracle failed.' }
    if (-not $GoOnly) {
        . (Join-Path $PSScriptRoot 'lib/rust-environment.ps1')
        $rs10Environment = Initialize-VeloRustEnvironment
        Push-Location $rs10Environment.RustRoot
        try {
            & cargo test --locked --no-default-features --features "renderer-$($Renderer.ToLowerInvariant())" --test discovery_contract --test discovery_runtime
            if ($LASTEXITCODE -ne 0) { throw 'RS-10 Rust discovery regression failed.' }
        } finally { Pop-Location }
    }
    Write-Output 'RS-10 discovery/filter/action Go contract passed (Rust covered here or by the full verification caller).'
} finally { Pop-Location }
