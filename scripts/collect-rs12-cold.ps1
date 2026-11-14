#Requires -Version 7.4
[CmdletBinding()]
param([Parameter(Mandatory)][string]$CaseDirectory)
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'lib/resource-sample.ps1')
$CaseDirectory=(Resolve-Path -LiteralPath $CaseDirectory).Path
$case=Read-VeloMetadataJson (Join-Path $CaseDirectory 'case.json')
$machineInput=$case.PSObject.Properties['InputSource'] -and $case.InputSource -eq 'MachineCalibration'
if(-not $case.PSObject.Properties['ActualInputProbe'] -or -not $case.ActualInputProbe){throw 'This older cold case has no actual-input probe contract; use the newer prepared set.'}
$start=Read-VeloMetadataJson (Join-Path $CaseDirectory 'started.json')
$exit=Read-VeloMetadataJson (Join-Path $CaseDirectory 'exit.json')
$destination=Join-Path $CaseDirectory 'cold-input.json'
if(Test-Path -LiteralPath $destination){throw 'This cold attempt has already been reviewed; keep it and prepare a new case.'}
if(-not $start.OperatorConfirmedFirstLaunch -or $start.Case -ne $case.Case -or $exit.Case -ne $case.Case){throw 'Cold case/start/exit identities do not agree.'}
$boot=(Get-CimInstance Win32_OperatingSystem).LastBootUpTime.ToUniversalTime()
if($boot.Ticks -ne ([datetime]$start.BootUtc).ToUniversalTime().Ticks -or ([datetime]$case.PreparedAtUtc).ToUniversalTime() -ge $boot){throw 'Cold case must be prepared before this boot and captured in the same boot as its recorded launch.'}
$ledger=Read-VeloMetadataJson (Join-Path (Split-Path -Parent $PSScriptRoot) ('.reasonix/rs12-cold-boot-ledger/boot-'+$start.BootID+'.json'))
if($ledger.Case -ne $case.Case -or ([datetime]$ledger.BootUtc).ToUniversalTime().Ticks -ne ([datetime]$start.BootUtc).ToUniversalTime().Ticks){throw 'The independent cold-condition ledger does not identify this case.'}
$current=Get-Process -Id $start.ProcessId -ErrorAction SilentlyContinue
if($current -and $current.StartTime.ToUniversalTime().Ticks -eq $start.StartedTicks){throw 'Exit the cold measurement process normally before exporting its evidence.'}
Add-Type -Path (Join-Path $PSScriptRoot 'lib/rs12-input-observer.cs')
$startUs=[long](($start.StartedTicks-[datetime]::UnixEpoch.Ticks)/10)
$accepted=$null
$lost=$false
if(Test-Path -LiteralPath $case.MeasurementLog){foreach($line in Read-VeloEventLines $case.MeasurementLog){
    $fields=[VeloRS12Input.Observer]::Parse($line)
    if($fields.ContainsKey('pid') -and $fields['pid'] -eq [string]$start.ProcessId -and $fields['dropped'] -ne '0'){$lost=$true}
    if(-not $accepted -and [VeloRS12Input.Observer]::Accepted($fields,$start.ProcessId) -and [long]$fields['unix_us'] -ge $startUs){$accepted=$fields}
}}
$valid=$null -ne $accepted -and -not $lost -and $exit.MatchesPreparedSHA256 -and $exit.ExitCode -eq 0
$os=Get-CimInstance Win32_OperatingSystem
$environmentId=Get-VeloColdEnvironmentId -WindowsVersion $os.Version -WindowsBuild $os.BuildNumber -Processors @((Get-CimInstance Win32_Processor).Name)
$observation=@{Number=1;Kind='ColdStartup';Valid=$valid;InjectedStart=$false;StartLowerUnixUs=$startUs-1000;StartObservedUnixUs=$startUs;AcceptedUnixUs=$(if($accepted){[long]$accepted['unix_us']}else{0});DurationUpperMilliseconds=$(if($accepted){([long]$accepted['unix_us']-$startUs+1000)/1000.0}else{0});ClockUncertaintyMilliseconds=1.0;Reason=$(if($valid){'Independent boot ledger and actual focused character accepted; operator launch/typing interval included. First-Velo-launch precondition is human-attested, not proven by this script.'}else{'Missing valid actual input, event loss, changed executable or abnormal exit; failed cold attempt retained.'})}
[ordered]@{SchemaVersion=1;SHA256=$case.SHA256;Kind='ColdStartup';Boundary='ProcessStartToEditable';BootId=$start.BootID;EnvironmentId=$environmentId;EnvironmentIdAlgorithm='ColdWindowsBuildCpuOrdinalV1';InputId=$case.InputId;InputSource=$(if($machineInput){'MachineCalibration'}else{'OperatorActualInputObservation'});MachineGeneratedInput=[bool]$machineInput;RecordedAtUtc=[datetime]::UtcNow.ToString('o');IndependentBootClaim=$true;FirstLaunchOperatorConfirmed=$start.OperatorConfirmedFirstLaunch;Method='IndependentBootToAcceptedCharacterUpperBound';StartupClockCalibration='OS creation FILETIME and accepted-input UTC with a 1ms guard; wall-clock step/error is not independently bounded. Not calibrated first-possible-readiness timing.';Result=@{Completed=$valid;Observations=@($observation)};RawLogSHA256=$(if(Test-Path -LiteralPath $case.MeasurementLog){(Get-FileHash -LiteralPath $case.MeasurementLog).Hash})}|ConvertTo-Json -Depth 10|Set-Content -LiteralPath $destination -Encoding utf8NoBOM
if(-not $valid){throw 'Cold actual-input sample invalid; failure retained.'}
'Independent cold attempt with actual accepted-character evidence exported; inspect manual delay and cold preflight separately.'
