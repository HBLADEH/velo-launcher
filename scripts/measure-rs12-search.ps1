#Requires -Version 7.4
[CmdletBinding()]
param([Parameter(Mandatory)][string]$OutputDirectory)
$ErrorActionPreference = 'Stop'
$workspace = Split-Path -Parent $PSScriptRoot
$OutputDirectory = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($OutputDirectory)
if (Test-Path -LiteralPath $OutputDirectory) { throw 'Use a fresh search evidence directory.' }
if (@(Get-CimInstance Win32_Process -Filter "Name LIKE 'velo-launcher%' OR Name LIKE 'velo-slint%'").Count) { throw 'Do not run benchmarks during desktop resource sampling.' }
. (Join-Path $PSScriptRoot 'lib/rust-environment.ps1')
$environment = Initialize-VeloRustEnvironment
$previousDirectory = $env:VELO_RS12_BENCH_DIR
$previousGoToolchain = $env:GOTOOLCHAIN
try {
    # Match development.md and the official v0.9.11 release compiler, even if
    # an older Go launcher appears first on this machine's PATH.
    $env:GOTOOLCHAIN = 'go1.25.6'
    if ((& go version) -notlike 'go version go1.25.6 *') { throw 'Go 1.25.6 is required for this comparison.' }
    $env:VELO_RS12_BENCH_DIR = $OutputDirectory
    Push-Location $workspace
    try {
        & go test ./internal/app -run '^TestRS12SearchMeasurement$' -count=1 -timeout=10m
        if ($LASTEXITCODE -ne 0) { throw 'Go shared search measurement failed.' }
    } finally { Pop-Location }
    $env:VELO_RS12_BENCH_DIR = $previousDirectory
    Push-Location $environment.RustRoot
    try {
        & cargo run --release --locked --no-default-features --features renderer-software --example rs12_search -- $OutputDirectory
        if ($LASTEXITCODE -ne 0) { throw 'Rust shared search measurement/result comparison failed.' }
        $rustVersion = & rustc --version
    } finally { Pop-Location }
    [ordered]@{
        Method = 'Sequential Go then Rust Release; fixed clock; 8 warm-up queries then 3 x 256 timed queries per case. Kernel excludes history scoring; service includes it. No UI, disk I/O or fixture setup in timed scope.'
        Go = (& go version); Rust = $rustVersion; WorkspaceCommit = (& git -C $workspace rev-parse HEAD)
        FixtureSHA256 = (Get-FileHash -LiteralPath (Join-Path $OutputDirectory 'fixture.json')).Hash
        GoSHA256 = (Get-FileHash -LiteralPath (Join-Path $OutputDirectory 'go.json')).Hash
        RustSHA256 = (Get-FileHash -LiteralPath (Join-Path $OutputDirectory 'rust.json')).Hash
        Sources = @(foreach ($path in @('internal/app/rs12_benchmark_test.go', 'internal/app/service.go', 'internal/search/search.go', 'internal/history/history.go', 'rust/examples/rs12_search.rs', 'rust/src/core/search.rs', 'rust/src/core/catalog.rs', 'rust/src/core/history.rs')) { @{ Path = $path; SHA256 = (Get-FileHash -LiteralPath (Join-Path $workspace $path)).Hash } })
        ExecutableSHA256 = (Get-FileHash -LiteralPath (Join-Path $environment.RustRoot 'target/x86_64-pc-windows-msvc/release/examples/rs12_search.exe')).Hash
        CompletedAtUtc = [DateTime]::UtcNow.ToString('o')
    } | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $OutputDirectory 'identity.json')
} finally { $env:VELO_RS12_BENCH_DIR = $previousDirectory; $env:GOTOOLCHAIN = $previousGoToolchain }
