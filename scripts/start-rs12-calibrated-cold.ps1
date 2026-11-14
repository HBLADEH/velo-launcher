#Requires -Version 7.4
[CmdletBinding()]
param([Parameter(Mandatory)][string]$CaseDirectory,[Parameter(Mandatory)][switch]$FirstVeloLaunchThisBootConfirmed)
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'lib/resource-sample.ps1')
$CaseDirectory=(Resolve-Path -LiteralPath $CaseDirectory).Path
$case=Read-VeloMetadataJson (Join-Path $CaseDirectory 'case.json')
$manifest=Read-VeloMetadataJson (Join-Path (Split-Path -Parent $CaseDirectory) 'calibration-manifest.json')
$boot=(Get-CimInstance Win32_OperatingSystem).LastBootUpTime.ToUniversalTime()
if(-not $FirstVeloLaunchThisBootConfirmed -or ([datetime]$case.PreparedAtUtc).ToUniversalTime() -ge $boot){throw 'Confirm a later independent boot with no earlier Velo launch before calibrated cold measurement.'}
$ledger=Join-Path (Join-Path (Split-Path -Parent $PSScriptRoot) '.reasonix/rs12-cold-boot-ledger') ('boot-'+$boot.Ticks+'.json')
if(Test-Path -LiteralPath $ledger){throw 'Current boot was already consumed by a cold case.'}
if(@(Get-CimInstance Win32_Process -Filter "Name LIKE 'velo-launcher%' OR Name LIKE 'velo-slint%'").Count -or (Test-Path -LiteralPath (Join-Path $CaseDirectory 'started.json'))){throw 'Calibrated cold case already attempted or another Velo is running.'}
foreach($property in $manifest.ToolsSHA256.PSObject.Properties){if((Get-FileHash -LiteralPath (Join-Path $PSScriptRoot $property.Name)).Hash -ne $property.Value){throw 'Prepared cold collector source identity changed; review before launch.'}}
if((Get-FileHash -LiteralPath $manifest.CalibratorExecutable).Hash -ne $manifest.CalibratorSHA256){throw 'Prepared native calibrator changed.'}
# Only metadata and the external calibrator are read above. Never read/hash/map
# the candidate executable or its application files before the measured launch.
$trace=Join-Path $CaseDirectory 'startup-calibration.jsonl';$ready=Join-Path $CaseDirectory 'calibration.ready';$pidFile=Join-Path $CaseDirectory 'target.identity'
$helper=[Diagnostics.ProcessStartInfo]::new($manifest.CalibratorExecutable)
$helper.UseShellExecute=$false;$helper.CreateNoWindow=$true;$helper.WindowStyle='Hidden'
$kind=if($case.Edition -eq 'wails'){'wails'}else{'slint'}
foreach($argument in @('startup',$kind,$pidFile,$case.MeasurementLog,$case.Executable,$trace,$ready)){$helper.ArgumentList.Add($argument)}
$calibrator=[Diagnostics.Process]::Start($helper)
$watch=[Diagnostics.Stopwatch]::StartNew()
while(-not(Test-Path -LiteralPath $ready)){if($calibrator.HasExited -or $watch.Elapsed.TotalSeconds -gt 15){throw 'Cold machine input observer did not arm.'};Start-Sleep -Milliseconds 2}
$launch=[Diagnostics.ProcessStartInfo]::new((Get-Process -Id $PID).Path)
$launch.UseShellExecute=$false;$launch.CreateNoWindow=$true;$launch.WindowStyle='Hidden'
foreach($argument in @('-NoProfile','-File',(Join-Path $PSScriptRoot 'start-rs12-cold.ps1'),'-CaseDirectory',$CaseDirectory,'-FirstVeloLaunchThisBootConfirmed')){$launch.ArgumentList.Add($argument)}
$launcher=[Diagnostics.Process]::Start($launch)
$startPath=Join-Path $CaseDirectory 'started.json'
while(-not(Test-Path -LiteralPath $startPath)){if($launcher.HasExited -or $watch.Elapsed.TotalSeconds -gt 25){throw 'Cold launcher failed before recording its identity.'};Start-Sleep -Milliseconds 2}
$start=Read-VeloMetadataJson $startPath
[ordered]@{Edition=$case.Edition;ProcessId=$start.ProcessId;StartedTicks=$start.StartedTicks;SHA256=$case.SHA256;DataDirectory=$case.DataDirectory;MeasurementLog=$case.MeasurementLog;BootId=$start.BootID;IndependentBootClaim=$true;OperatorConfirmedFirstLaunch=$true;InputSource='Native machine Unicode cold calibration';CandidateFilesReadBeforeLaunch=$false}|ConvertTo-Json|Set-Content -LiteralPath (Join-Path $CaseDirectory 'session.json') -Encoding utf8NoBOM
$temporary=$pidFile+'.tmp'
[IO.File]::WriteAllText($temporary,([string]$start.ProcessId+' '+[datetime]::new([long]$start.StartedTicks,[DateTimeKind]::Utc).ToFileTimeUtc()),[Text.UTF8Encoding]::new($false));[IO.File]::Move($temporary,$pidFile)
while(-not $calibrator.WaitForExit(250)){if($watch.Elapsed.TotalSeconds -gt 45){throw 'Calibrated cold input exceeded its bounded probe window.'}}
[ordered]@{RecordedAtUtc=[datetime]::UtcNow.ToString('o');Case=$case.Case;InputSource='Native generated Unicode startup calibration; not physical input';NativeExitCode=$calibrator.ExitCode;CalibratorSHA256=$manifest.CalibratorSHA256;CandidateFilesReadBeforeLaunch=$false;OperatorConfirmedFirstLaunch=$true;BootId=$start.BootID}|ConvertTo-Json|Set-Content -LiteralPath (Join-Path $CaseDirectory 'machine-input-source.json') -Encoding utf8NoBOM
Write-Output 'Cold machine-input trace saved. Exit the isolated Velo normally through its Quit button or tray. This is not physical-key evidence.'
while(-not $launcher.WaitForExit(1000)){}
if($launcher.ExitCode -ne 0){throw 'Cold launcher did not confirm normal process exit.'}
$exit=Read-VeloMetadataJson (Join-Path $CaseDirectory 'exit.json')
[ordered]@{Exited=$true;ExitCode=$exit.ExitCode;ForcedCleanup=$false;ExecutableMatches=$exit.MatchesPreparedSHA256;MachineCalibration=$true}|ConvertTo-Json|Set-Content -LiteralPath (Join-Path $CaseDirectory 'completion.json') -Encoding utf8NoBOM
& (Join-Path $PSScriptRoot 'collect-rs12-cold.ps1') -CaseDirectory $CaseDirectory
$calibrator.Dispose();$launcher.Dispose()
