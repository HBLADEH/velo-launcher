#Requires -Version 7.4
[CmdletBinding()]
param([string]$CalibratorExecutable)
$ErrorActionPreference='Stop'
function Assert($Condition,[string]$Message){if(-not $Condition){throw $Message}}
foreach($name in 'build-rs12-calibrator.ps1','measure-rs12-readiness.ps1','test-rs12-readiness.ps1','export-rs12-readiness.ps1','prepare-rs12-calibrated-cold.ps1','start-rs12-calibrated-cold.ps1','measure-rs12-animation.ps1','export-rs12-animation.ps1'){
    $tokens=$null;$errors=$null
    [void][System.Management.Automation.Language.Parser]::ParseFile((Join-Path $PSScriptRoot $name),[ref]$tokens,[ref]$errors)
    Assert (-not $errors) ('Calibration script syntax: '+$name)
}
$fixture=Join-Path ([IO.Path]::GetTempPath()) ('velo-rs12-ready-check-'+[guid]::NewGuid().ToString('N'))
try{
    $case=Join-Path $fixture 'warm-1';New-Item -ItemType Directory -Path $case -Force|Out-Null
    $created=133000000000000000L;$unixUs=[long](($created+200000-[datetime]::UnixEpoch.ToFileTimeUtc())/10)
    $log=Join-Path $case 'input.log'
    @{Edition='software';ProcessId=42;StartedTicks=[datetime]::FromFileTimeUtc($created).Ticks;SHA256=('a'*64);MeasurementLog=$log}|ConvertTo-Json|Set-Content -LiteralPath (Join-Path $case 'session.json') -Encoding utf8NoBOM
    @{Exited=$true;ExitCode=0;ForcedCleanup=$false;ExecutableMatches=$true}|ConvertTo-Json|Set-Content -LiteralPath (Join-Path $case 'completion.json') -Encoding utf8NoBOM
    "event=input_accepted pid=42 unix_us=$unixUs dropped=0 focused=true native_focused=true visible=true composing=false synthetic=false utf8_bytes=1"|Set-Content -LiteralPath $log -Encoding utf8NoBOM
    $events=@(@{event='clock';before_qpc=1000;after_qpc=1002;utc_filetime_100ns=$created},@{event='phase_start';pid=42;start_lower_qpc=1001;start_upper_qpc=1002;qpc_frequency=1000000;start_filetime_100ns=$created;machine_input=$true},@{event='accepted';unix_us=$unixUs;probes=1;first_probe_qpc=19000;max_probe_gap_qpc=0;machine_input=$true},@{event='clock';before_qpc=31000;after_qpc=31002;utc_filetime_100ns=$created+300000})
    $trace=Join-Path $case 'startup-calibration.jsonl'
    $events|ForEach-Object{$_|ConvertTo-Json -Compress}|Set-Content -LiteralPath $trace -Encoding utf8NoBOM
    $null=& (Join-Path $PSScriptRoot 'export-rs12-readiness.ps1') -GroupDirectory $fixture -OutputPath (Join-Path $fixture 'valid.json')
    $result=Get-Content -Raw -LiteralPath (Join-Path $fixture 'valid.json')|ConvertFrom-Json
    Assert ($result.Observations[0].Valid -and $result.Observations[0].DurationUpperMilliseconds -gt 20 -and $result.Observations[0].DurationUpperMilliseconds -lt 21) 'Calibrated acceptance must use the actual input clock, with its declared bracket/guard.'
    Assert ($result.Observations[0].MachineGeneratedInput -and -not $result.Observations[0].CountsAsPhysicalHotkey -and -not $result.ExactFirstPossibleInputClaim) 'Machine probes must remain separate from physical/first-possible-readiness claims.'
    [IO.File]::Move($log,$log+'.saved')
    $null=& (Join-Path $PSScriptRoot 'export-rs12-readiness.ps1') -GroupDirectory $fixture -OutputPath (Join-Path $fixture 'missing-log.json')
    $missing=Get-Content -Raw -LiteralPath (Join-Path $fixture 'missing-log.json')|ConvertFrom-Json
    Assert (-not $missing.Observations[0].Valid -and $null -eq $missing.Observations[0].FinalProductLogSHA256) 'A missing accepted-character log must remain an invalid sample, not disappear or fail export.'
    [IO.File]::Move($log+'.saved',$log)
    $events[-1].utc_filetime_100ns+=100000
    $events|ForEach-Object{$_|ConvertTo-Json -Compress}|Set-Content -LiteralPath $trace -Encoding utf8NoBOM
    $null=& (Join-Path $PSScriptRoot 'export-rs12-readiness.ps1') -GroupDirectory $fixture -OutputPath (Join-Path $fixture 'clock-step.json')
    $clock=Get-Content -Raw -LiteralPath (Join-Path $fixture 'clock-step.json')|ConvertFrom-Json
    Assert (-not $clock.Observations[0].Valid -and $clock.Observations[0].Reason -like 'Observed QPC/UTC mapping changed*') 'A clock step must invalidate timing while preserving the attempt.'
    $cold=Join-Path $fixture 'same-boot-cold';New-Item -ItemType Directory -Path $cold|Out-Null
    @{PreparedAtUtc=[datetime]::UtcNow.ToString('o')}|ConvertTo-Json|Set-Content -LiteralPath (Join-Path $cold 'case.json') -Encoding utf8NoBOM
    @{dummy=$true}|ConvertTo-Json|Set-Content -LiteralPath (Join-Path $fixture 'calibration-manifest.json') -Encoding utf8NoBOM
    $rejected=$false
    try{& (Join-Path $PSScriptRoot 'start-rs12-calibrated-cold.ps1') -CaseDirectory $cold -FirstVeloLaunchThisBootConfirmed}catch{$rejected=$_.Exception.Message -like 'Confirm a later independent boot*'}
    Assert ($rejected -and -not(Test-Path -LiteralPath (Join-Path $cold 'calibration.ready'))) 'Same-boot preparation must fail before any helper or candidate launches.'
    $events[-1].utc_filetime_100ns-=100000
    $events|ForEach-Object{$_|ConvertTo-Json -Compress}|Set-Content -LiteralPath $trace -Encoding utf8NoBOM
    $coldSession=Get-Content -Raw -LiteralPath (Join-Path $case 'session.json')|ConvertFrom-Json
    $coldSession|Add-Member -NotePropertyName BootId -NotePropertyValue 'synthetic-independent-boot'
    $coldSession|Add-Member -NotePropertyName IndependentBootClaim -NotePropertyValue $true
    $coldSession|Add-Member -NotePropertyName OperatorConfirmedFirstLaunch -NotePropertyValue $true
    $coldSession|ConvertTo-Json|Set-Content -LiteralPath (Join-Path $case 'session.json') -Encoding utf8NoBOM
    $null=& (Join-Path $PSScriptRoot 'export-rs12-readiness.ps1') -GroupDirectory $fixture -ColdCases -OutputPath (Join-Path $fixture 'cold.json')
    $coldResult=Get-Content -Raw -LiteralPath (Join-Path $fixture 'cold.json')|ConvertFrom-Json
    Assert ($coldResult.ColdStartupMeasured -and $coldResult.Observations[0].Kind -eq 'ColdStartup' -and $coldResult.Observations[0].BootId -eq 'synthetic-independent-boot') 'Calibrated cold export must retain the independent boot identity and machine-input source.'
    Copy-Item -LiteralPath $case -Destination (Join-Path $fixture 'duplicate-boot') -Recurse
    $rejected=$false
    try{& (Join-Path $PSScriptRoot 'export-rs12-readiness.ps1') -GroupDirectory $fixture -ColdCases -OutputPath (Join-Path $fixture 'duplicate-cold.json')}catch{$rejected=$_.Exception.Message -like 'Multiple calibrated cold launches share a boot*'}
    Assert $rejected 'Two valid calibrated cold observations in one boot must not count as independent samples.'
}finally{
    if(Test-Path -LiteralPath $fixture){$resolved=(Resolve-Path -LiteralPath $fixture).Path;$temp=[IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\')+'\';if(-not $resolved.StartsWith($temp,[StringComparison]::OrdinalIgnoreCase) -or (Split-Path -Leaf $resolved) -notlike 'velo-rs12-ready-check-*'){throw 'Unexpected readiness fixture cleanup.'};Remove-Item -LiteralPath $resolved -Recurse -Force}
}
if($CalibratorExecutable){
    $CalibratorExecutable=(Resolve-Path -LiteralPath $CalibratorExecutable).Path
    $directory=Join-Path (Join-Path (Split-Path -Parent $PSScriptRoot) '.reasonix') ('rs12-readiness-guards-'+[guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Path $directory|Out-Null
    & $CalibratorExecutable self-test (Join-Path $directory 'guards.jsonl')
    Assert ($LASTEXITCODE -eq 0) 'Hidden/foreign HWND guards must refuse generated input.'
    $guard=Get-Content -Raw -LiteralPath (Join-Path $directory 'guards.jsonl')|ConvertFrom-Json
    Assert ($guard.passed -and -not $guard.input_sent) 'Guard verification must send no keyboard input.'
    $identity=Join-Path $directory 'target.identity'
    [IO.File]::WriteAllText($identity,([string]$PID+' '+((Get-Process -Id $PID).StartTime.ToUniversalTime().ToFileTimeUtc()+1)),[Text.UTF8Encoding]::new($false))
    $trace=Join-Path $directory 'reused-pid.jsonl'
    & $CalibratorExecutable startup slint $identity (Join-Path $directory 'input.log') (Join-Path $directory 'does-not-exist.exe') $trace (Join-Path $directory 'ready') 2>&1|Out-Null
    Assert ($LASTEXITCODE -ne 0) 'Creation-time mismatch must be refused before any UI input.'
    $failure=@(Get-Content -LiteralPath $trace|ForEach-Object{$_|ConvertFrom-Json})|Where-Object event -eq 'failure'
    Assert ($failure.reason -eq 'process_creation_changed') 'A reused process identity must have an explicit failure record.'
}
'RS-12 readiness checks passed (acceptance clocks, explicit machine source, missing log/clock failure retention, same-boot cold refusal and syntax); no keyboard input sent by these checks.'
