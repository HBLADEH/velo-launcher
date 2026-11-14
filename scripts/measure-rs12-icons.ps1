#Requires -Version 7.4
[CmdletBinding()]
param([Parameter(Mandatory)][string]$OutputDirectory)
$ErrorActionPreference='Stop'
$workspace=Split-Path -Parent $PSScriptRoot
$OutputDirectory=$ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($OutputDirectory)
if(Test-Path -LiteralPath $OutputDirectory){throw 'Use a new icon measurement directory; keep earlier failures.'}
if(@(Get-CimInstance Win32_Process -Filter "Name LIKE 'velo-launcher%' OR Name LIKE 'velo-slint%'").Count){throw 'Do not measure icons while a desktop resource sample is running.'}
New-Item -ItemType Directory -Path (Join-Path $OutputDirectory 'binaries') -Force|Out-Null
. (Join-Path $PSScriptRoot 'lib/rust-environment.ps1')
$environment=Initialize-VeloRustEnvironment
$originalGo=$env:GOTOOLCHAIN
try{
    $env:GOTOOLCHAIN='go1.25.6'
    Push-Location $workspace
    try{
        & go build -o (Join-Path $OutputDirectory 'binaries/rs12-icons-go.exe') ./cmd/rs12-icons
        if($LASTEXITCODE -ne 0){throw 'Go icon measurement build failed.'}
    }finally{Pop-Location}
    Push-Location $environment.RustRoot
    try{
        & cargo fmt --all -- --check
        if($LASTEXITCODE -ne 0){throw 'Rust formatting failed.'}
        & cargo clippy --locked --no-default-features --features renderer-software --example rs12_icons -- -D warnings
        if($LASTEXITCODE -ne 0){throw 'Rust icon measurement clippy failed.'}
        & cargo build --release --locked --no-default-features --features renderer-software --example rs12_icons
        if($LASTEXITCODE -ne 0){throw 'Rust icon measurement build failed.'}
        Copy-Item -LiteralPath 'target/x86_64-pc-windows-msvc/release/examples/rs12_icons.exe' -Destination (Join-Path $OutputDirectory 'binaries/rs12-icons-rust.exe')
        $rustVersion=& rustc --version
    }finally{Pop-Location}
    # All builds finish before either timing process starts.
    & (Join-Path $OutputDirectory 'binaries/rs12-icons-go.exe') (Join-Path $OutputDirectory 'go')
    if($LASTEXITCODE -ne 0){throw 'Go native icon extraction/cache checks failed.'}
    & (Join-Path $OutputDirectory 'binaries/rs12-icons-rust.exe') (Join-Path $OutputDirectory 'rust')
    if($LASTEXITCODE -ne 0){throw 'Rust native icon extraction/cache checks failed.'}
    $go=Get-Content -LiteralPath (Join-Path $OutputDirectory 'go/go-icons.json') -Raw|ConvertFrom-Json
    $rust=Get-Content -LiteralPath (Join-Path $OutputDirectory 'rust/rust-icons.json') -Raw|ConvertFrom-Json
    $comparisons=@(foreach($round in 1..3){
        $g=@($go|Where-Object{$_.round -eq $round -and $_.phase -eq 'empty-disk-cache-extract-encode'})[0]
        $r=@($rust|Where-Object{$_.round -eq $round -and $_.phase -eq 'empty-application-disk-cache-extract-encode-flush'})[0]
        if(-not $g -or -not $r -or $g.extractions -ne 4 -or $r.extractions -ne 4){throw 'Missing native extraction evidence.'}
        [ordered]@{Round=$round;Items=4;SameIconURLs=($g.icon_urls|ConvertTo-Json -Compress) -eq ($r.icon_urls|ConvertTo-Json -Compress);SamePixels=($g.pixel_hashes|ConvertTo-Json -Compress) -eq ($r.pixel_hashes|ConvertTo-Json -Compress);DecodedGoBytes=$g.decoded_rgba_bytes;DecodedRustBytes=$r.decoded_rgba_bytes}
    })
    [ordered]@{Method='Offline Shell extraction for the same four System32 executables, three fresh application-cache directories; OS cache retained. Go encodes after extraction, Rust also flushes/syncs. Disk-hit Go Populate checks file existence; Rust load decodes PNG. LRU is a CPU reference clone, not a presented texture.';Go=(& go version);Rust=$rustVersion;Comparisons=$comparisons;Sources=@(foreach($file in 'cmd/rs12-icons/main.go','internal/icon/cache.go','internal/platform/icon_windows.go','rust/examples/rs12_icons.rs','rust/src/core/icons.rs','rust/src/platform/windows/icons.rs'){@{Path=$file;SHA256=(Get-FileHash -LiteralPath (Join-Path $workspace $file)).Hash}});Executables=@(Get-ChildItem -LiteralPath (Join-Path $OutputDirectory 'binaries') -File|ForEach-Object{@{Name=$_.Name;SHA256=(Get-FileHash -LiteralPath $_.FullName).Hash}});CompletedAtUtc=[datetime]::UtcNow.ToString('o');OSCacheCold=$false;GPUTextureMeasured=$false}|ConvertTo-Json -Depth 12|Set-Content -LiteralPath (Join-Path $OutputDirectory 'identity.json') -Encoding utf8NoBOM
    'RS-12 offline native icon cache measurements saved. Review timing scopes and pixel comparisons before drawing conclusions.'
}finally{$env:GOTOOLCHAIN=$originalGo}
