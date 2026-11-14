#Requires -Version 7.4
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$SessionDirectory,
    [Parameter(Mandatory)][string]$ObserverExecutable,
    [Parameter(Mandatory)][string]$OutputDirectory,
    [ValidateRange(1,900)][int]$Seconds=180,
    [ValidateRange(0,128)][int]$SaveFrames=0
)
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'lib/resource-sample.ps1')
$collectorSHA256=(Get-FileHash -LiteralPath $PSCommandPath).Hash
$ObserverExecutable=(Resolve-Path -LiteralPath $ObserverExecutable).Path
$observerHash=(Get-FileHash -LiteralPath $ObserverExecutable).Hash
$identity=Read-VeloMetadataJson (Join-Path (Split-Path -Parent $ObserverExecutable) 'identity.json')
if($observerHash -ne $identity.ExecutableSHA256){throw 'Frame observer binary differs from its build identity.'}
$session=Read-VeloMetadataJson (Join-Path $SessionDirectory 'session.json')
$target=Get-Process -Id $session.ProcessId
if($target.Path -ne $session.Executable -or $target.StartTime.ToUniversalTime().Ticks -ne $session.StartedTicks){throw 'Target process identity changed.'}
$OutputDirectory=$ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($OutputDirectory)
if(Test-Path -LiteralPath $OutputDirectory){throw 'Use a new frame evidence directory.'}
$kind=if($session.Edition -eq 'wails'){'wails'}else{'slint'}
& $ObserverExecutable $kind $session.ProcessId $Seconds $SaveFrames $OutputDirectory
$nativeExitCode=$LASTEXITCODE
if(Test-Path -LiteralPath $OutputDirectory){
    [ordered]@{RecordedAtUtc=[datetime]::UtcNow.ToString('o');NativeExitCode=$nativeExitCode;CollectorSHA256=$collectorSHA256;ObserverSHA256=$observerHash;ObserverSourceSHA256=$identity.SourceSHA256;TargetSHA256=$session.SHA256;ProcessId=$session.ProcessId;StartedTicks=$session.StartedTicks;Seconds=$Seconds;SaveFrames=$SaveFrames;Scope='Selected HWND inset crop only; physical fixed A/Backspace/Escape and Alt+Space metadata. No injected input, activation or product writes. Raw bitmap crops stay local.'}|ConvertTo-Json|Set-Content -LiteralPath (Join-Path $OutputDirectory 'identity.json') -Encoding utf8NoBOM
}
if($nativeExitCode -ne 0){throw "Native frame capture failed (exit $nativeExitCode); preserve earlier rows and diagnostics."}
$ready=Read-VeloEventLines (Join-Path $OutputDirectory 'events.jsonl')|ForEach-Object{$_|ConvertFrom-Json}|Where-Object event -eq 'capture_ready'|Select-Object -First 1
$expectedFiletime=[datetime]::new([long]$session.StartedTicks,[DateTimeKind]::Utc).ToFileTimeUtc()
if($ready.pid -ne $session.ProcessId -or $ready.creation_filetime_100ns -ne $expectedFiletime){throw 'Frame observer attached to a different process creation identity; capture rejected.'}
