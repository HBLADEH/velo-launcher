#Requires -Version 7.4
[CmdletBinding()]
param([Parameter(Mandatory)][string]$OutputDirectory)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'lib/rust-environment.ps1')
$environment = Initialize-VeloRustEnvironment
$OutputDirectory = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($OutputDirectory)
if (Test-Path -LiteralPath $OutputDirectory) { throw 'Visual capture needs a fresh output directory.' }
New-Item -ItemType Directory -Path $OutputDirectory | Out-Null
Push-Location $environment.RustRoot
try {
    $sources = @(@('Cargo.toml','Cargo.lock','build.rs','rust-toolchain.toml','windows-toolchain.json','.cargo/config.toml','examples/rs13_visual.rs','../scripts/capture-rust-visual.ps1') | ForEach-Object { Get-Item -LiteralPath $_ })
    $sources += @(Get-ChildItem -LiteralPath 'ui' -Recurse -File)
    $sources += @(Get-ChildItem -LiteralPath 'examples/support' -Filter 'rs13_*.rs' -File)
    $sourceHashes = @($sources | ForEach-Object { [ordered]@{
        Path = [IO.Path]::GetRelativePath($environment.RustRoot, $_.FullName).Replace('\','/')
        SHA256 = (Get-FileHash -LiteralPath $_.FullName).Hash
    } })
    foreach ($renderer in @('software', 'femtovg')) {
        & cargo build --release --locked --no-default-features --features "renderer-$renderer" --example rs13_visual
        if ($LASTEXITCODE -ne 0) { throw "Visual example build failed: $renderer" }
        $destination = Join-Path $OutputDirectory $renderer
        New-Item -ItemType Directory -Path $destination | Out-Null
        $executable = Join-Path $destination 'rs13_visual.exe'
        Copy-Item -LiteralPath 'target/x86_64-pc-windows-msvc/release/examples/rs13_visual.exe' -Destination $executable
        $info = [Diagnostics.ProcessStartInfo]::new($executable)
        $info.UseShellExecute = $false
        $info.CreateNoWindow = $true
        $info.WindowStyle = 'Hidden'
        $info.ArgumentList.Add((Join-Path $destination 'images'))
        $process = [Diagnostics.Process]::Start($info)
        try {
            if (-not $process.WaitForExit(90000)) { throw "Visual capture timed out: $renderer" }
            if ($process.ExitCode -ne 0) { throw "Visual capture failed: $renderer (exit $($process.ExitCode))" }
        } finally {
            if (-not $process.HasExited) { $process.Kill(); $process.WaitForExit() }
            $process.Dispose()
        }
        $images = @(Get-ChildItem -LiteralPath (Join-Path $destination 'images') -Filter '*.png' -File)
        $sceneNames = @(Get-Content -Raw -LiteralPath (Join-Path $destination 'images/scenes.json') | ConvertFrom-Json)
        $expectedNames = @($sceneNames | ForEach-Object { "light-$_.png"; "dark-$_.png" } | Sort-Object)
        if ((($images.Name | Sort-Object) -join "`n") -cne ($expectedNames -join "`n")) { throw "Static scene inventory differs: $renderer" }
        foreach ($source in $sourceHashes) {
            if ((Get-FileHash -LiteralPath $source.Path).Hash -ne $source.SHA256) { throw 'Visual source changed during capture; retain this attempt and use a fresh directory.' }
        }
        [ordered]@{
            RecordedAtUtc = [DateTime]::UtcNow.ToString('o')
            Renderer = $renderer
            Kind = 'Dedicated synthetic Slint component snapshot example; not the measured product executable'
            ExecutableSHA256 = (Get-FileHash -LiteralPath $executable).Hash
            SourceSHA256 = (Get-FileHash -LiteralPath 'examples/rs13_visual.rs').Hash
            Sources = $sourceHashes
            WorkspaceCommit = (& git rev-parse HEAD)
            WorkspaceDirty = [bool](& git status --porcelain)
            ReducedMotion = $true
            ExpectedScenes = $sceneNames
            PhysicalInput = $false
            MonitorFrameCapture = $false
            WailsComparisonComplete = $false
            Images = @($images | Sort-Object Name | ForEach-Object {
                [ordered]@{ Name = $_.Name; SHA256 = (Get-FileHash -LiteralPath $_.FullName).Hash }
            })
        } | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $destination 'capture.json') -Encoding utf8NoBOM
    }
} finally { Pop-Location }
