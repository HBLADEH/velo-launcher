#Requires -Version 7.4
[CmdletBinding()]
param([Parameter(Mandatory)][string]$OutputDirectory)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'lib/rust-environment.ps1')
$environment = Initialize-VeloRustEnvironment
$OutputDirectory = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($OutputDirectory)
if (Test-Path -LiteralPath $OutputDirectory) { throw 'Animation capture needs a fresh output directory.' }
New-Item -ItemType Directory -Path $OutputDirectory | Out-Null
Push-Location $environment.RustRoot
try {
    $sources = @(@('Cargo.toml','Cargo.lock','build.rs','rust-toolchain.toml','windows-toolchain.json','.cargo/config.toml','examples/rs13_animation.rs','scripts/capture-rust-animation.ps1') | ForEach-Object { Get-Item -LiteralPath $_ })
    $sources += @(Get-ChildItem -LiteralPath 'ui' -Recurse -File)
    $sources += @(Get-ChildItem -LiteralPath 'examples/support' -Filter 'rs13_*.rs' -File)
    $fingerprints = @($sources | ForEach-Object { [ordered]@{
        Path = [IO.Path]::GetRelativePath($environment.RustRoot, $_.FullName).Replace('\','/')
        SHA256 = (Get-FileHash -LiteralPath $_.FullName).Hash
    } })
    foreach ($renderer in 'software','femtovg') {
        & cargo build --release --locked --no-default-features --features "renderer-$renderer" --example rs13_animation
        if ($LASTEXITCODE -ne 0) { throw "Animation build failed: $renderer" }
        $destination = Join-Path $OutputDirectory $renderer
        New-Item -ItemType Directory -Path $destination | Out-Null
        $executable = Join-Path $destination 'rs13_animation.exe'
        Copy-Item -LiteralPath 'target/x86_64-pc-windows-msvc/release/examples/rs13_animation.exe' -Destination $executable
        $info = [Diagnostics.ProcessStartInfo]::new($executable)
        $info.UseShellExecute = $false; $info.CreateNoWindow = $true; $info.WindowStyle = 'Hidden'
        $info.RedirectStandardOutput = $true; $info.RedirectStandardError = $true
        $info.ArgumentList.Add((Join-Path $destination 'frames'))
        $process = [Diagnostics.Process]::Start($info)
        $outputTask = $process.StandardOutput.ReadToEndAsync()
        $errorTask = $process.StandardError.ReadToEndAsync()
        try {
            if (-not $process.WaitForExit(90000)) { throw "Animation capture timed out: $renderer" }
            if ($process.ExitCode -ne 0) { throw "Animation capture failed: $renderer (exit $($process.ExitCode))" }
        } finally {
            if (-not $process.HasExited) { $process.Kill(); $process.WaitForExit() }
            $outputTask.GetAwaiter().GetResult() | Set-Content -LiteralPath (Join-Path $destination 'stdout.log') -Encoding utf8NoBOM
            $errorTask.GetAwaiter().GetResult() | Set-Content -LiteralPath (Join-Path $destination 'stderr.log') -Encoding utf8NoBOM
            $process.Dispose()
        }
        $recording = Get-Content -Raw -LiteralPath (Join-Path $destination 'frames/recording.json') | ConvertFrom-Json
        if (-not $recording.Complete -or $recording.Renderer -ne $renderer) { throw 'Incomplete animation recording.' }
        foreach ($source in $fingerprints) {
            if ((Get-FileHash -LiteralPath $source.Path).Hash -ne $source.SHA256) { throw 'Animation source changed during capture; retain this attempt.' }
        }
        [ordered]@{
            RecordedAtUtc = [DateTime]::UtcNow.ToString('o'); Renderer = $renderer
            ExecutableSHA256 = (Get-FileHash -LiteralPath $executable).Hash
            RecordingSHA256 = (Get-FileHash -LiteralPath (Join-Path $destination 'frames/recording.json')).Hash
            WorkspaceCommit = (& git rev-parse HEAD); WorkspaceDirty = [bool](& git status --porcelain)
            Sources = $fingerprints; SyntheticInput = $true; PresentedFrames = $false
            Images = @(Get-ChildItem -LiteralPath (Join-Path $destination 'frames') -Filter '*.png' | Sort-Object Name | ForEach-Object { [ordered]@{Name=$_.Name;SHA256=(Get-FileHash -LiteralPath $_.FullName).Hash} })
        } | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $destination 'capture.json') -Encoding utf8NoBOM
    }
} finally { Pop-Location }
