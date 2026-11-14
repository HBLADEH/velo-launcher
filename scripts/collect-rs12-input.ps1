#Requires -Version 7.4
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$SessionDirectory,
    [Parameter(Mandatory)][ValidateSet('WarmStartup','ColdStartup','HotkeyActivation')][string]$Kind,
    [ValidateRange(0,30)][int]$Count = 0,
    [ValidateRange(10,900)][int]$TimeoutSeconds = 300,
    [ValidateSet('ValidSamples','PhysicalStarts','UntilExit')][string]$CountMode = 'ValidSamples',
    [Parameter(Mandatory)][string]$OutputPath
)
$ErrorActionPreference = 'Stop'
$collectorSHA256=(Get-FileHash -LiteralPath $PSCommandPath).Hash
if($Count -eq 0){$Count=if($Kind -eq 'HotkeyActivation'){20}else{1}}
if($Kind -ne 'HotkeyActivation' -and $Count -ne 1){throw 'A startup observation belongs to one process; use a new session for each startup sample.'}
if($Kind -ne 'HotkeyActivation' -and $CountMode -ne 'ValidSamples'){throw 'Only hotkey observations support diagnostic count modes.'}
. (Join-Path $PSScriptRoot 'lib/resource-sample.ps1')
. (Join-Path $PSScriptRoot 'lib/native-observation.ps1')
if(Test-Path -LiteralPath $OutputPath){throw 'Use a new OutputPath; keep all previous input observations.'}
$session=Read-VeloMetadataJson (Join-Path $SessionDirectory 'session.json')
if(-not $session.MeasurementLog){throw 'Actual-input timing requires a log-producing measurement session; do not substitute a normal-mode focus flag.'}
$live=Get-CimInstance Win32_Process -Filter "ProcessId=$($session.ProcessId)"
if(-not $live -or $live.ExecutablePath -ne $session.Executable -or (Get-Process -Id $session.ProcessId).StartTime.ToUniversalTime().Ticks -ne $session.StartedTicks){throw 'The recorded process identity changed.'}
if($Kind -eq 'ColdStartup'){throw 'Cold samples require independent boot evidence and the prepared cold entry; this session entry cannot establish it.'}
$hwnd=Get-VeloLauncherWindowHandle -ProcessId $session.ProcessId -ApplicationKind $(if($session.Edition -eq 'wails'){'Wails'}else{'Slint'})
$observerPath=Join-Path $PSScriptRoot 'lib/rs12-input-observer.cs'
$observerSHA256=(Get-FileHash -LiteralPath $observerPath).Hash
Add-Type -Path $observerPath
$startUs=($session.StartedTicks-[datetime]::UnixEpoch.Ticks)/10
$result=[VeloRS12Input.Observer]::Run($session.ProcessId,[Convert]::ToInt64($hwnd,16),$session.MeasurementLog,$Kind,$Count,$TimeoutSeconds,[long]$startUs,$CountMode)
[ordered]@{
    SchemaVersion=3;SHA256=$session.SHA256;ObserverSHA256=$observerSHA256;CollectorSHA256=$collectorSHA256;ObserverIdentityScope='Source at load, before capture';CountMode=$CountMode;RequestedCount=$Count
    Kind=$Kind;Result=$result;RecordedAtUtc=[datetime]::UtcNow.ToString('o');EnvironmentId=$session.EnvironmentId;InputId=$session.InputId
    Method=$(if($Kind -eq 'HotkeyActivation'){'Physical Alt+Space Windows event lower bound to actual non-composing character accepted by the focused visible launcher. Includes manual typing delay. Injected hotkeys excluded.'}else{'OS process creation lower bound to actual non-composing character accepted by the focused visible launcher. Includes launch/activation and operator typing delay.'})
    Boundary=$(if($Kind -eq 'HotkeyActivation'){'KeyDownToEditable'}else{'ProcessStartToEditable'})
    Limitations=@('Actual accepted character is later than first-possible readiness; human delay is not removed. Above-budget upper bounds alone cannot establish a readiness regression.','Windows keyboard-event clock quantum and UTC mapping bounds are reported. Native timestamps are callback observations, not presented frames or application-level cause.','Only starts on the selected hidden launcher; no other typed keys or text are collected.','Completed describes capture termination only. It does not establish reliable activation or absence of operator-reported failures.','UntilExit retains all bounded-session attempts until the target window ends or the timeout; minimum sample counts are reported separately.')
    StartupClockCalibration=$(if($Kind -ne 'HotkeyActivation'){'OS process creation FILETIME to accepted UTC with a 1ms added guard; this is not an independently calibrated error bound.'}else{$null})
} | ConvertTo-Json -Depth 12 | Set-Content -LiteralPath $OutputPath -Encoding utf8NoBOM
if(-not $result.Completed){
    $validCount=@($result.Observations|Where-Object { $_.Valid -and -not $_.InjectedStart }).Count
    throw "Physical-input capture ended with $($result.EndReason); valid samples $validCount/$Count. Full report and failures retained."
}
