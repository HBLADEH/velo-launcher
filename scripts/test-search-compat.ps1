param([ValidateSet('FemtoVG', 'Software')][string]$Renderer = 'Software')
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'lib/rust-environment.ps1')
$environment = Initialize-VeloRustEnvironment
Push-Location $environment.RustRoot
try {
    & cargo test --locked --no-default-features --features "renderer-$($Renderer.ToLowerInvariant())" --test behavior_contract --test history_library
    if ($LASTEXITCODE -ne 0) { throw 'Search/history/library regression failed.' }
} finally { Pop-Location }
