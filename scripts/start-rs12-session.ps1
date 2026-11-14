#Requires -Version 7.4
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$PreparedDirectory,
    [Parameter(Mandatory)][ValidateSet('wails', 'software', 'femtovg')][string]$Edition,
    [Parameter(Mandatory)][ValidatePattern('^[a-zA-Z0-9-]{1,40}$')][string]$RunName,
    [switch]$Background,
    [switch]$Uninstrumented,
    [ValidateRange(0,120)][int]$StartupResourceSeconds = 0,
    [ValidateRange(10,1000)][int]$StartupResourceIntervalMilliseconds = 100
)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'lib/resource-sample.ps1')
$PreparedDirectory = (Resolve-Path -LiteralPath $PreparedDirectory).Path
if (@(Get-CimInstance Win32_Process -Filter "Name LIKE 'velo-launcher%' OR Name LIKE 'velo-slint%'").Count) { throw 'Exit the other Velo edition before starting the desktop session.' }
$manifest = Read-VeloMetadataJson (Join-Path $PreparedDirectory 'manifest.json')
$exe = Join-Path $PreparedDirectory "binaries/$Edition/$(if ($Edition -eq 'wails') { 'velo-launcher.exe' } else { 'velo-slint.exe' })"
$expected = if ($Edition -eq 'wails') { $manifest.Builds.wails.SHA256 } else { $manifest.Builds.$Edition.ExecutableSHA256 }
if ((Get-FileHash -LiteralPath $exe).Hash -ne $expected) { throw 'Frozen executable changed.' }
$output = Join-Path $PreparedDirectory "runs/$RunName"
if (Test-Path -LiteralPath $output) { throw 'Use a new RunName.' }
New-Item -ItemType Directory -Path (Join-Path $output 'local') -Force | Out-Null
$data = Join-Path $output 'local/Velo'
Copy-Item -LiteralPath (Join-Path $PreparedDirectory 'seed') -Destination $data -Recurse
$log = Join-Path $output 'measurement-events.log'
$start = [Diagnostics.ProcessStartInfo]::new($exe)
$start.UseShellExecute = $false
$start.CreateNoWindow = $true
$start.WindowStyle = 'Hidden'
if ($Edition -eq 'wails') {
    # The released Go binary has no instance-id override. Refusing concurrent
    # launchers is therefore required in addition to redirecting its data.
    $start.Environment['LOCALAPPDATA'] = Join-Path $output 'local'
} else {
    foreach ($argument in @(('--instance-id=rs12-' + $RunName), ('--data-dir=' + $data))) { $start.ArgumentList.Add($argument) }
    if (-not $Uninstrumented) { $start.ArgumentList.Add('--measurement-business'); $start.ArgumentList.Add('--measurement-log=' + $log) }
}
if ($Background) { $start.ArgumentList.Add('--background') }
if ($StartupResourceSeconds -gt 0) { Add-Type -Path (Join-Path $PSScriptRoot 'lib/rs12-resource-observer.cs') }
$process = [Diagnostics.Process]::Start($start)
$startedTicks = $process.StartTime.ToUniversalTime().Ticks
$resourceTask = if ($StartupResourceSeconds -gt 0) { [VeloRS12Resources.Observer]::Start($process.Id, $startedTicks, $StartupResourceSeconds, $StartupResourceIntervalMilliseconds) }
[ordered]@{ Edition = $Edition; ProcessId = $process.Id; StartedTicks = $process.StartTime.ToUniversalTime().Ticks; Executable = $exe; SHA256 = $expected; DataDirectory = $data; MeasurementLog = $(if ($Edition -ne 'wails' -and -not $Uninstrumented) { $log } else { $null }); Uninstrumented = $Edition -eq 'wails' -or $Uninstrumented.IsPresent; Background = $Background.IsPresent; Method = 'Normal business lifecycle; no injected commands or keys. Operator supplies physical input.' } |
    ConvertTo-Json | Set-Content -LiteralPath (Join-Path $output 'session.json')
Write-Output "Desktop session started: $Edition / $RunName / PID $($process.Id). Exit normally from the tray or in-app Quit when finished."
if ($resourceTask) {
    & (Join-Path $PSScriptRoot 'measure-rs12-peak.ps1') -SessionDirectory $output -OutputPath (Join-Path $output 'startup-resources.json') -ObservationTask $resourceTask
}
# Retain the OS process handle so actual exit code and creation identity survive
# a normal UI exit; no polling changes focus, window state or application data.
while (-not $process.WaitForExit(1000)) { }
[ordered]@{ Completed = $process.ExitCode -eq 0; CheckOnly = $false; Renderer = $Edition; SHA256 = $expected; ProcessId = $process.Id; ExitCode = $process.ExitCode; ForcedCleanup = $false; SyntheticCycles = 0; ThirtyMinuteLoop = $false; Method = 'Operator desktop session; resource/latency coverage is established by separate evidence.' } |
    ConvertTo-Json | Set-Content -LiteralPath (Join-Path $output 'completion.json')
$process.Dispose()
