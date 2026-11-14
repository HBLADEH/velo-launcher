#Requires -Version 7.4
[CmdletBinding()]
param([Parameter(Mandatory)][string]$GroupDirectory,[Parameter(Mandatory)][string]$OutputPath,[switch]$ColdCases)
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'lib/resource-sample.ps1')
if(Test-Path -LiteralPath $OutputPath){throw 'Preserve earlier readiness reports.'}
$GroupDirectory=(Resolve-Path -LiteralPath $GroupDirectory).Path
function Fields([string]$Line){$fields=@{};foreach($part in $Line.Split(' ')){if($part -match '^([^=]+)=(.*)$'){$fields[$Matches[1]]=$Matches[2]}};$fields}
function Statistics([double[]]$Values){$sorted=@($Values|Sort-Object);if(-not $sorted.Count){return $null};$m=[int][math]::Floor($sorted.Count/2);[ordered]@{Count=$sorted.Count;Minimum=$sorted[0];Median=$(if($sorted.Count%2){$sorted[$m]}else{($sorted[$m-1]+$sorted[$m])/2});P95=$sorted[[int][math]::Ceiling(.95*$sorted.Count)-1];Maximum=$sorted[-1]}}
$directories=@(if($ColdCases){Get-ChildItem -LiteralPath $GroupDirectory -Directory|Where-Object{Test-Path -LiteralPath (Join-Path $_.FullName 'session.json')}}else{Get-ChildItem -LiteralPath $GroupDirectory -Directory -Filter 'warm-*'})
$rows=@(foreach($case in $directories){
    $session=Read-VeloMetadataJson (Join-Path $case.FullName 'session.json')
    $completion=Read-VeloMetadataJson (Join-Path $case.FullName 'completion.json')
    $inputEvents=@(if(Test-Path -LiteralPath $session.MeasurementLog){Read-VeloEventLines $session.MeasurementLog|ForEach-Object{Fields $_}})
    $loss=@($inputEvents|Where-Object{$_.pid -eq [string]$session.ProcessId -and $_.dropped -ne '0'}).Count -gt 0
    $normal=$completion.Exited -and $completion.ExitCode -eq 0 -and -not $completion.ForcedCleanup -and $completion.ExecutableMatches
    foreach($file in Get-ChildItem -LiteralPath $case.FullName -File -Filter '*.jsonl'|Where-Object{$_.Name -eq 'startup-calibration.jsonl' -or $_.Name -match '^hotkey-\d+\.jsonl$'}){
        $events=@(Read-VeloEventLines $file.FullName|ForEach-Object{$_|ConvertFrom-Json})
        $start=@($events|Where-Object event -eq 'phase_start');$accepted=@($events|Where-Object event -eq 'accepted');$failure=@($events|Where-Object event -eq 'failure')
        $kind=if($file.Name -eq 'startup-calibration.jsonl'){$(if($ColdCases){'ColdStartup'}else{'WarmStartup'})}else{'InjectedHotkeyCalibration'}
        $valid=$normal -and -not $loss -and $start.Count -eq 1 -and $accepted.Count -eq 1 -and $failure.Count -eq 0
        $duration=$null;$uncertainty=$null;$range=$null;$reason='Accepted character, final normal exit, process creation, and observed QPC/UTC calibration checked. Machine-generated input; physical evidence remains separate.'
        if($valid){
            $s=$start[0];$a=$accepted[0];$frequency=[double]$s.qpc_frequency
            $clocks=@($events|Where-Object event -eq 'clock')
            if($frequency -le 0 -or $clocks.Count -lt 2 -or @($clocks|Where-Object{$_.before_qpc -gt $_.after_qpc}).Count){throw 'Invalid readiness clock brackets.'}
            $origin=$clocks[0];$originMid=([double]$origin.before_qpc+[double]$origin.after_qpc)/2
            $offsets=@($clocks|ForEach-Object{[double]((([decimal]$_.utc_filetime_100ns-[decimal]$origin.utc_filetime_100ns)/10000)-((([double]$_.before_qpc+[double]$_.after_qpc)/2-$originMid)*1000/$frequency))})
            $range=($offsets|Measure-Object -Maximum).Maximum-($offsets|Measure-Object -Minimum).Minimum
            $bracket=($clocks|ForEach-Object{($_.after_qpc-$_.before_qpc)*1000/$frequency}|Measure-Object -Maximum).Maximum
            if($range -gt .5){$valid=$false;$reason='Observed QPC/UTC mapping changed more than 0.5ms; trace retained.'}else{
                # Brackets + observed drift + declared guard; not a claim that
                # an arbitrary clock step between brackets is impossible.
                $uncertainty=$range+$bracket+.1
                $acceptedFiletime=[decimal]$a.unix_us*10+[datetime]::UnixEpoch.ToFileTimeUtc()
                $endQpc=$originMid+[double](($acceptedFiletime-[decimal]$origin.utc_filetime_100ns)/10000000)*$frequency
                $matching=@($inputEvents|Where-Object{$_.event -eq 'input_accepted' -and $_.pid -eq [string]$session.ProcessId -and [long]$_.unix_us -eq $a.unix_us -and $_.focused -eq 'true' -and $_.native_focused -eq 'true' -and $_.visible -eq 'true' -and $_.composing -eq 'false' -and $_.synthetic -ne 'true' -and $_.trusted -ne 'false' -and [int]$_.utf8_bytes -gt 0})
                $creationMatches=$kind -eq 'InjectedHotkeyCalibration' -or [long]$s.start_filetime_100ns -eq [datetime]::new([long]$session.StartedTicks,[DateTimeKind]::Utc).ToFileTimeUtc()
                if($ColdCases -and (-not $session.IndependentBootClaim -or -not $session.OperatorConfirmedFirstLaunch -or -not $session.BootId)){throw 'Cold calibration requires original independent-boot and operator confirmation records.'}
                if($matching.Count -ne 1 -or -not $creationMatches -or $s.pid -ne $session.ProcessId -or $a.probes -lt 1 -or $endQpc -lt $a.first_probe_qpc-($uncertainty*$frequency/1000)){$valid=$false;$reason='Selected edit, source probe, PID or creation identity did not agree.'}else{
                    $duration=($endQpc-$s.start_lower_qpc)*1000/$frequency+$uncertainty
                    if($duration -le 0){$valid=$false;$duration=$null;$reason='Non-positive calibrated interval.'}
                }
            }
        }else{$reason='Missing accepted edit, failed native guard, event loss, or abnormal exit; failure preserved. '+($failure.reason -join ',')}
        [pscustomobject][ordered]@{Edition=$session.Edition;SHA256=$session.SHA256;Kind=$kind;BootId=$(if($ColdCases){$session.BootId}else{$null});Valid=$valid;DurationUpperMilliseconds=$duration;ClockMappingObservedRangeMilliseconds=$range;DeclaredClockGuardAndObservedBracketMilliseconds=$uncertainty;MachineGeneratedInput=$true;CountsAsPhysicalHotkey=$false;Probes=$(if($accepted.Count){$accepted[0].probes}else{$null});MaximumProbeGapMilliseconds=$(if($accepted.Count -and $start.Count){$accepted[0].max_probe_gap_qpc*1000/[double]$start[0].qpc_frequency}else{$null});Case=$case.Name;Trace=$file.Name;TraceSHA256=(Get-FileHash -LiteralPath $file.FullName).Hash;FinalProductLogSHA256=$(if(Test-Path -LiteralPath $session.MeasurementLog){(Get-FileHash -LiteralPath $session.MeasurementLog).Hash}else{$null});Reason=$reason}
    }
})
if(-not $rows.Count){throw 'No readiness observations to export.'}
if($ColdCases -and @($rows|Where-Object Valid|Group-Object BootId|Where-Object Count -gt 1).Count){throw 'Multiple calibrated cold launches share a boot; keep failures but reject this grouping.'}
$groups=@($rows|Group-Object Kind,SHA256|ForEach-Object{$valid=@($_.Group|Where-Object Valid);$kind=$_.Group[0].Kind;$minimum=if($kind -ne 'InjectedHotkeyCalibration'){3}else{20};$budget=if($kind -ne 'InjectedHotkeyCalibration'){500}else{50};$stats=Statistics @($valid.DurationUpperMilliseconds);[ordered]@{Edition=$_.Group[0].Edition;SHA256=$_.Group[0].SHA256;Kind=$kind;ValidSamples=$valid.Count;InvalidSamples=$_.Count-$valid.Count;MinimumSamples=$minimum;SampleCountSatisfied=$valid.Count -ge $minimum;UpperMilliseconds=$stats;BudgetMilliseconds=$budget;UpperBoundP95WithinBudget=($null -ne $stats -and $stats.P95 -le $budget);CountsAsPhysicalHotkeys=$false}})
[ordered]@{SchemaVersion=1;RecordedAtUtc=[datetime]::UtcNow.ToString('o');Method='Fixed Unicode a via guarded native SendInput; operational QPC/UTC brackets to actual focused non-composing accepted edit. Separate from physical-key tests.';Groups=$groups;Observations=$rows;ColdStartupMeasured=$ColdCases.IsPresent;ExactFirstPossibleInputClaim=$false;BudgetInterpretation='An upper bound within budget supports the tested acceptance target; an above-budget upper bound alone does not prove an earlier possible readiness regression. Windows clock continuity is checked at stored brackets, with a declared 0.1ms guard.';Scope='Machine integration calibration only; not real IME or physical Alt+Space. No reinterpretation of earlier manual-delay records.';RS12Complete=$false}|ConvertTo-Json -Depth 12|Set-Content -LiteralPath $OutputPath -Encoding utf8NoBOM
$groups|ConvertTo-Json -Depth 7
