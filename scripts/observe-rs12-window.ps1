#Requires -Version 7.4
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$SessionDirectory,
    [ValidateRange(1,900)][int]$Seconds=900,
    [ValidateRange(20,1000)][int]$IntervalMilliseconds=50,
    [Parameter(Mandatory)][string]$OutputPath
)
$ErrorActionPreference='Stop'
$collectorSHA256=(Get-FileHash -LiteralPath $PSCommandPath).Hash
. (Join-Path $PSScriptRoot 'lib/resource-sample.ps1')
. (Join-Path $PSScriptRoot 'lib/native-observation.ps1')
if(Test-Path -LiteralPath $OutputPath){throw 'Use a fresh OutputPath; preserve earlier observations.'}
$session=Read-VeloMetadataJson (Join-Path $SessionDirectory 'session.json')
$process=Get-Process -Id $session.ProcessId
if($process.Path -ne $session.Executable -or $process.StartTime.ToUniversalTime().Ticks -ne $session.StartedTicks){throw 'Target process identity changed.'}
$hwnd=Get-VeloLauncherWindowHandle -ProcessId $session.ProcessId -ApplicationKind $(if($session.Edition -eq 'wails'){'Wails'}else{'Slint'})
$observerPath=Join-Path $PSScriptRoot 'lib/rs12-window-observer.cs'
$observerSHA256=(Get-FileHash -LiteralPath $observerPath).Hash
Add-Type -Path $observerPath
Write-Output "RS12_WINDOW_OBSERVER_READY pid=$($session.ProcessId)"
$capture=[VeloRS12Window.Observer]::Run($session.ProcessId,$session.StartedTicks,[Convert]::ToInt64($hwnd,16),$Seconds,$IntervalMilliseconds)
[ordered]@{SchemaVersion=1;SHA256=$session.SHA256;RecordedAtUtc=[datetime]::UtcNow.ToString('o');Capture=$capture;CollectorSHA256=$collectorSHA256;ObserverSHA256=$observerSHA256;ObserverIdentityScope='Source at load, before capture';Scope='Passive native physical coordinates/DPI/visibility/foreground ownership. Changes between polls can be missed. Does not measure presented frames, text or IME readiness.'}|ConvertTo-Json -Depth 9|Set-Content -LiteralPath $OutputPath -Encoding utf8NoBOM
if($capture.Truncated){throw 'Bounded state change capture truncated; original observations retained.'}
