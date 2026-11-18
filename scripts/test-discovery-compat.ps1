param([ValidateSet('FemtoVG', 'Software')][string]$Renderer = 'Software')
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'lib/rust-environment.ps1')
$environment = Initialize-VeloRustEnvironment
Push-Location $environment.RustRoot
try {
    & cargo test --locked --no-default-features --features "renderer-$($Renderer.ToLowerInvariant())" --test discovery_contract --test discovery_runtime
    if ($LASTEXITCODE -ne 0) { throw 'Discovery/filter/icon regression failed.' }
} finally { Pop-Location }
