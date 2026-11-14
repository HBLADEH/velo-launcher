#Requires -Version 7.4
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$OutputDirectory,
    [string]$SeedDirectory = (Join-Path $env:LOCALAPPDATA 'Velo')
)
$ErrorActionPreference = 'Stop'
$workspace = Split-Path -Parent $PSScriptRoot
. (Join-Path $PSScriptRoot 'lib/resource-sample.ps1')
$OutputDirectory = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($OutputDirectory)
$SeedDirectory = (Resolve-Path -LiteralPath $SeedDirectory).Path
if (Test-Path -LiteralPath $OutputDirectory) { throw 'Use a fresh RS-12 directory; previous binaries and samples must remain immutable.' }
if (@(Get-CimInstance Win32_Process -Filter "Name LIKE 'velo-launcher%' OR Name LIKE 'velo-slint%'").Count) { throw 'Exit all Velo instances before preparing the comparison.' }
New-Item -ItemType Directory -Path $OutputDirectory | Out-Null
$binaries = Join-Path $OutputDirectory 'binaries'
New-Item -ItemType Directory -Path $binaries | Out-Null
$identities = [ordered]@{}
foreach ($renderer in @('software', 'femtovg')) {
    $source = Join-Path $workspace "rust/target/artifacts/$renderer"
    $identity = Read-VeloRustBuildIdentity (Join-Path $source 'velo-slint.exe')
    foreach ($entry in $identity.Sources) {
        if ($entry.SHA256 -ne (Get-FileHash -LiteralPath (Join-Path $workspace ('rust/' + $entry.Path))).Hash) { throw "Stale $renderer build input: $($entry.Path)" }
    }
    $destination = Join-Path $binaries $renderer
    New-Item -ItemType Directory -Path $destination | Out-Null
    foreach ($name in @('velo-slint.exe', 'build-identity.json', 'cargo-metadata.json', 'cargo-tree.txt', 'dependency-licenses.json')) {
        Copy-Item -LiteralPath (Join-Path $source $name) -Destination $destination
    }
    $identities[$renderer] = $identity
}
if (($identities.software.Sources | ConvertTo-Json -Compress) -ne ($identities.femtovg.Sources | ConvertTo-Json -Compress)) { throw 'The two renderers must have identical source inputs.' }
$wails = Join-Path $binaries 'wails'
New-Item -ItemType Directory -Path $wails | Out-Null
$reference = Join-Path $wails 'velo-launcher.exe'
$releaseURL = 'https://github.com/HBLADEH/velo-launcher/releases/download/v0.9.11/'
Invoke-WebRequest -Uri ($releaseURL + 'velo-launcher.exe') -OutFile $reference
Invoke-WebRequest -Uri ($releaseURL + 'SHA256SUMS.txt') -OutFile (Join-Path $wails 'SHA256SUMS.txt')
$expected = '9A1A898A1E3D70267481BCC68E3368F0B6594F68E8AE43B3F0DCDC06A9B329A6'
if ((Get-FileHash -LiteralPath $reference).Hash -ne $expected) { throw 'The Wails binary does not match the documented v0.9.11 CI release.' }
$releaseSums = Get-Content -LiteralPath (Join-Path $wails 'SHA256SUMS.txt')
if (-not ($releaseSums -match ('(?i)^' + $expected + '\s+\*?velo-launcher\.exe$'))) { throw 'Published checksum manifest disagrees with the pinned release identity.' }
$identities.wails = [ordered]@{ Version = '0.9.11'; Commit = 'abde1353117add4b58b9c34eb442e1ad9c445d97'; Origin = $releaseURL; SHA256 = $expected; Bytes = (Get-Item -LiteralPath $reference).Length; GoBuildInfo = @(& go version -m $reference) }
if ($LASTEXITCODE -ne 0) { throw 'Cannot inspect the Wails release.' }
$seed = Join-Path $OutputDirectory 'seed'
New-Item -ItemType Directory -Path $seed | Out-Null
$originalInputs = [ordered]@{}
foreach ($name in @('config.json', 'index.json', 'history.json', 'application-library.json', 'quick-launch.json')) {
    $source = Join-Path $SeedDirectory $name
    $originalInputs[$name] = if (Test-Path -LiteralPath $source) { (Get-FileHash -LiteralPath $source).Hash } else { $null }
    if ($originalInputs[$name]) { Copy-Item -LiteralPath $source -Destination $seed }
}
$icons = Join-Path $SeedDirectory 'cache/icons'
if (Test-Path -LiteralPath $icons) {
    New-Item -ItemType Directory -Path (Join-Path $seed 'cache') | Out-Null
    Copy-Item -LiteralPath $icons -Destination (Join-Path $seed 'cache/icons') -Recurse
}
$configPath = Join-Path $seed 'config.json'
$config = Read-VeloMetadataJson $configPath
# Only the copied, controlled preset is changed. Default-network regression is
# a separate run; 1440 minutes keeps scheduled scans outside a 30-minute cycle.
foreach ($pair in @(@('auto_check_updates', $false), @('launch_at_startup', $false), @('refresh_minutes', 1440), @('theme', 'light'), @('result_layout', 'list'), @('hotkey', 'Alt+Space'))) {
    $config | Add-Member -NotePropertyName $pair[0] -NotePropertyValue $pair[1] -Force
}
$config | ConvertTo-Json -Depth 40 | Set-Content -LiteralPath $configPath -Encoding utf8NoBOM
$inputs = [ordered]@{}
foreach ($name in @('config.json', 'index.json', 'history.json', 'application-library.json', 'quick-launch.json')) {
    $path = Join-Path $seed $name
    $inputs[$name] = if (Test-Path -LiteralPath $path) { (Get-FileHash -LiteralPath $path).Hash } else { $null }
}
$data = Join-Path $OutputDirectory 'data'
New-Item -ItemType Directory -Path $data | Out-Null
foreach ($edition in @('wails', 'software', 'femtovg')) {
    $local = Join-Path $data $edition
    New-Item -ItemType Directory -Path $local | Out-Null
    Copy-Item -LiteralPath $seed -Destination (Join-Path $local 'Velo') -Recurse
}
$patchPath = Join-Path $OutputDirectory 'workspace.patch'
& git -C $workspace diff --binary HEAD --output=$patchPath
if ($LASTEXITCODE -ne 0) { throw 'Cannot freeze the tracked workspace patch.' }
# Untracked migration modules also contribute to the build. Preserve a local
# content snapshot, since HEAD plus a tracked-only diff cannot reconstruct it.
$snapshot = Join-Path $OutputDirectory 'source'
New-Item -ItemType Directory -Path $snapshot | Out-Null
$tracked = @(& git -C $workspace ls-files)
$untracked = @(& git -C $workspace ls-files --others --exclude-standard)
$sources = foreach ($name in @($tracked + $untracked | Sort-Object -Unique)) {
    $source = Join-Path $workspace $name
    if (-not (Test-Path -LiteralPath $source -PathType Leaf)) { continue }
    $destination = Join-Path $snapshot $name
    New-Item -ItemType Directory -Path (Split-Path -Parent $destination) -Force | Out-Null
    Copy-Item -LiteralPath $source -Destination $destination
    [ordered]@{ Path = $name; SHA256 = (Get-FileHash -LiteralPath $source).Hash }
}
$os = Get-CimInstance Win32_OperatingSystem
$monitorProbe = Join-Path $workspace 'rust/target/x86_64-pc-windows-msvc/debug/examples/rs06_native_probe.exe'
$monitors = @(if (Test-Path -LiteralPath $monitorProbe) { & $monitorProbe | Where-Object { $_ -like 'event=probe_monitor *' } })
[ordered]@{
    SchemaVersion = 1; PreparedAtUtc = [DateTime]::UtcNow.ToString('o'); WorkspaceCommit = (& git -C $workspace rev-parse HEAD)
    WorkspacePatchSHA256 = (Get-FileHash -LiteralPath $patchPath).Hash; Sources = @($sources); Builds = $identities
    Inputs = $inputs; OriginalInputs = $originalInputs; Icons = Get-VeloIconDiskEvidence $seed
    Preset = 'Copied business data; light/list; Alt+Space; updates off; login off; refresh=1440 minutes. Full discovery remains enabled.'
    Environment = @{ Windows = $os.Version; Build = $os.BuildNumber; BootUtc = $os.LastBootUpTime.ToUniversalTime().ToString('o'); CPU = @((Get-CimInstance Win32_Processor).Name); GPU = @(Get-CimInstance Win32_VideoController | Select-Object Caption, DriverVersion); Monitors = $monitors; Locale = [cultureinfo]::CurrentCulture.Name; ActiveIME = $null; PowerScheme = ((& powercfg /getactivescheme) | Out-String).Trim() }
} | ConvertTo-Json -Depth 32 | Set-Content -LiteralPath (Join-Path $OutputDirectory 'manifest.json') -Encoding utf8NoBOM
Write-Output "RS-12 binaries and identical input copies frozen in $OutputDirectory"
