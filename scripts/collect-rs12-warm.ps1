#Requires -Version 7.4
[CmdletBinding()]
param([Parameter(Mandatory)][string]$PreparedDirectory,[ValidateRange(30,900)][int]$TimeoutSeconds=600)
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'lib/resource-sample.ps1')
$plan=Read-VeloMetadataJson (Join-Path (Resolve-Path -LiteralPath $PreparedDirectory).Path 'warm-plan.json')
Add-Type -Path (Join-Path $PSScriptRoot 'lib/rs12-input-observer.cs')
foreach($case in $plan.Cases){
    $root=Split-Path -Parent $case.DataDirectory
    $root=Split-Path -Parent $root
    $destination=Join-Path $root 'warm-input.json'
    if(Test-Path -LiteralPath $destination){throw 'This warm case was already attempted; preserve it and prepare new cases.'}
    Write-Output "RS12_WARM_READY case=$($case.Name); launch its shortcut, type one English character, wait for results, then exit normally."
    $watch=[Diagnostics.Stopwatch]::StartNew()
    $native=$null
    while($watch.Elapsed.TotalSeconds -lt $TimeoutSeconds){
        $matches=@(Get-CimInstance Win32_Process -Filter "Name LIKE 'velo-slint%' OR Name LIKE 'velo-launcher%'"|Where-Object ExecutablePath -eq $case.Executable)
        if($matches.Count -eq 1){$native=$matches[0];break}
        Start-Sleep -Milliseconds 100
    }
    if(-not $native){throw 'No warm-case process observed before timeout. The unlaunched case remains pending.'}
    $process=Get-Process -Id $native.ProcessId
    $null=$process.Handle
    $startedTicks=$process.StartTime.ToUniversalTime().Ticks
    $startUs=[long](($startedTicks-[datetime]::UnixEpoch.Ticks)/10)
    [ordered]@{Edition=$case.Edition;ProcessId=$process.Id;StartedTicks=$startedTicks;Executable=$case.Executable;SHA256=$case.SHA256;MeasurementLog=$case.MeasurementLog;InputId=$case.InputId;EnvironmentId=$plan.EnvironmentId}|ConvertTo-Json|Set-Content -LiteralPath (Join-Path $root 'session.json') -Encoding utf8NoBOM
    $accepted=$null
    $lost=$false
    do{
        if(Test-Path -LiteralPath $case.MeasurementLog){foreach($line in Read-VeloEventLines $case.MeasurementLog){
            $fields=[VeloRS12Input.Observer]::Parse($line)
            if($fields.ContainsKey('pid') -and $fields['pid'] -eq [string]$process.Id -and $fields['dropped'] -ne '0'){$lost=$true}
            if([VeloRS12Input.Observer]::Accepted($fields,$process.Id) -and [long]$fields['unix_us'] -ge $startUs){$accepted=$fields;break}
        }}
        if($accepted -or $process.HasExited){break}
        Start-Sleep -Milliseconds 100
    }while($watch.Elapsed.TotalSeconds -lt $TimeoutSeconds)
    $valid=$null -ne $accepted -and -not $lost
    $observation=[ordered]@{Number=1;Kind='WarmStartup';Valid=$valid;InjectedStart=$false;StartLowerUnixUs=$startUs-1000;StartObservedUnixUs=$startUs;AcceptedUnixUs=$(if($accepted){[long]$accepted['unix_us']}else{0});DurationUpperMilliseconds=$(if($accepted){([long]$accepted['unix_us']-$startUs+1000)/1000.0}else{0});ClockUncertaintyMilliseconds=1.0;Reason=$(if($valid){'Actual focused accepted character; includes manual launch/activation and typing interval. No claim of first-possible readiness.'}else{'No focused non-composing character accepted, or input evidence was lost.'})}
    [ordered]@{SchemaVersion=1;SHA256=$case.SHA256;Kind='WarmStartup';Boundary='ProcessStartToEditable';InputId=$case.InputId;EnvironmentId=$plan.EnvironmentId;RecordedAtUtc=[datetime]::UtcNow.ToString('o');Method=$case.Scope;StartupClockCalibration='OS creation FILETIME and accepted-input UTC with a 1ms guard; wall-clock step/error is not independently bounded. Not a calibrated first-possible-readiness measurement.';Result=@{Completed=$valid;Observations=@($observation)}}|ConvertTo-Json -Depth 10|Set-Content -LiteralPath $destination -Encoding utf8NoBOM
    while(-not $process.HasExited -and $watch.Elapsed.TotalSeconds -lt $TimeoutSeconds){$null=$process.WaitForExit(1000)}
    [ordered]@{Exited=$process.HasExited;ExitCode=$(if($process.HasExited){$process.ExitCode});ProcessId=$process.Id;SHA256=$case.SHA256;SampleValid=$valid;RawLogSHA256=$(if(Test-Path -LiteralPath $case.MeasurementLog){(Get-FileHash -LiteralPath $case.MeasurementLog).Hash})}|ConvertTo-Json|Set-Content -LiteralPath (Join-Path $root 'completion.json') -Encoding utf8NoBOM
    Write-Output "RS12_WARM_SAVED case=$($case.Name) valid=$valid upper_ms=$($observation.DurationUpperMilliseconds)"
    if(-not $process.HasExited){throw 'Warm-case process is still running; leave the next case pending.'}
    $process.Dispose()
}
"$($plan.Cases.Count) actual-input warm-start observations saved."
