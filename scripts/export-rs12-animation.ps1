#Requires -Version 7.4
[CmdletBinding()]
param([Parameter(Mandatory)][string]$RunDirectory,[Parameter(Mandatory)][string]$OutputPath)
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'lib/resource-sample.ps1')
if(Test-Path -LiteralPath $OutputPath){throw 'Preserve earlier animation reports.'}
$session=Read-VeloMetadataJson (Join-Path $RunDirectory 'session.json')
$completion=Read-VeloMetadataJson (Join-Path $RunDirectory 'completion.json')
$capture=Read-VeloMetadataJson (Join-Path $RunDirectory 'cpu-resources.json')
$frames=@(Read-VeloEventLines (Join-Path $RunDirectory 'frames/events.jsonl')|ForEach-Object{$_|ConvertFrom-Json})
$ready=$frames|Where-Object event -eq 'capture_ready'|Select-Object -First 1
$end=$frames|Where-Object event -eq 'end'|Select-Object -Last 1
if(-not $completion.Completed -or -not $completion.Exited -or $completion.ExitCode -ne 0 -or $completion.ForcedCleanup -or -not $end.target_ended){throw 'Normal animation capture/exit is incomplete; preserve original evidence.'}
if($capture.ProcessId -ne $session.ProcessId -or $capture.StartedTicks -ne $session.StartedTicks -or $capture.QpcFrequency -ne $ready.qpc_frequency){throw 'Animation CPU/frame identities or QPC clocks differ.'}
if($ready.pid -ne $session.ProcessId -or $ready.creation_filetime_100ns -ne [datetime]::new([long]$session.StartedTicks,[DateTimeKind]::Utc).ToFileTimeUtc() -or -not $ready.geometry_freshness_checked){throw 'Animation frame process identity or geometry validation differs.'}
$f=[double]$capture.QpcFrequency
$snapshots=@($capture.Snapshots)
function IdentityKeys($Sample){@($Sample.Processes|ForEach-Object{[string]$_.ProcessId+'/'+$_.StartedTicks}|Sort-Object)}
function Stats([double[]]$Values){$s=@($Values|Sort-Object);if(-not $s.Count){return $null};$m=[int][math]::Floor($s.Count/2);[ordered]@{Count=$s.Count;Minimum=$s[0];Median=$(if($s.Count%2){$s[$m]}else{($s[$m-1]+$s[$m])/2});P95=$s[[int][math]::Ceiling(.95*$s.Count)-1];Maximum=$s[-1]}}
$steps=@(for($i=1;$i -lt $snapshots.Count;$i++){
    foreach($process in $snapshots[$i].Processes){$previous=$snapshots[$i-1].Processes|Where-Object{$_.ProcessId -eq $process.ProcessId -and $_.StartedTicks -eq $process.StartedTicks}|Select-Object -First 1;if($previous -and $process.CPUSeconds -gt $previous.CPUSeconds){$process.CPUSeconds-$previous.CPUSeconds}}
})
$rows=@(foreach($file in Get-ChildItem -LiteralPath $RunDirectory -File -Filter 'query-*.jsonl'){
    $events=@(Read-VeloEventLines $file.FullName|ForEach-Object{$_|ConvertFrom-Json})
    $phase=$events|Where-Object event -eq 'phase_start'|Select-Object -First 1
    $accepted=@($events|Where-Object event -eq 'accepted')
    $failure=@($events|Where-Object event -eq 'failure')
    if(-not $phase){throw 'Missing machine-query phase origin.'}
    $from=[double]$phase.start_lower_qpc;$to=$from+.5*$f
    $before=$snapshots|Where-Object{$_.EndedQpc -le $from}|Select-Object -Last 1
    $after=$snapshots|Where-Object{$_.StartedQpc -ge $to}|Select-Object -First 1
    $valid=$accepted.Count -eq 1 -and $failure.Count -eq 0 -and $null -ne $before -and $null -ne $after
    $reason='Full process-tree CPU counters bracket the 500ms query/animation window; desktop changed-state intervals recorded separately.'
    $cpu=$null;$states=@();$intervals=@();$totalFrames=0
    if($valid){
        $range=@($snapshots|Where-Object{$_.StartedQpc -ge $before.StartedQpc -and $_.EndedQpc -le $after.EndedQpc})
        $keys=(IdentityKeys $before) -join ','
        if(@($range|Where-Object{-not $_.Complete -or ((IdentityKeys $_) -join ',') -ne $keys}).Count){$valid=$false;$reason='CPU process identities changed or a bracketing snapshot was incomplete.'}else{
            $cpu=0.0
            foreach($process in $after.Processes){$old=$before.Processes|Where-Object{$_.ProcessId -eq $process.ProcessId -and $_.StartedTicks -eq $process.StartedTicks}|Select-Object -First 1;$delta=$process.CPUSeconds-$old.CPUSeconds;if($delta -lt 0){$valid=$false;$reason='A process CPU counter decreased.'};$cpu+=$delta}
            $queryFrames=@($frames|Where-Object{$_.event -eq 'frame' -and $_.present_qpc -ge $from -and $_.present_qpc -le $to})
            $totalFrames=$queryFrames.Count;$previous=$null
            foreach($frame in $queryFrames){$signature=[string]$frame.width+'/'+$frame.height+'/'+$frame.results_hash;if($signature -ne $previous){$states+=$frame;$previous=$signature}}
            $intervals=@(for($i=1;$i -lt $states.Count;$i++){($states[$i].present_qpc-$states[$i-1].present_qpc)*1000/$f})
            if($states.Count -lt 3){$valid=$false;$reason='Insufficient visible changed result frames in the animation window.'}
        }
    }else{$reason='Accepted machine query or surrounding CPU samples are missing.'}
    [pscustomobject][ordered]@{Trace=$file.Name;Valid=$valid;WindowStartQpc=$from;WindowEndQpc=$to;WindowMilliseconds=500;ProcessCount=$(if($before){$before.Processes.Count}else{$null});CPUCounterDeltaSeconds=$cpu;CPUCounterUpperMilliseconds=$(if($valid){$cpu*1000}else{$null});CPUReadBracketMilliseconds=$(if($before -and $after){($after.EndedQpc-$before.StartedQpc)*1000/$f}else{$null});MachineCPUPercentUpperOver500ms=$(if($valid){$cpu/.5/[Environment]::ProcessorCount*100}else{$null});ObservedDesktopFrames=$totalFrames;ChangedResultStates=$states.Count;ChangedResultStateIntervalsMilliseconds=$intervals;ChangedResultStateIntervalStatistics=(Stats $intervals);AverageCPUCounterMillisecondsPerChangedState=$(if($valid){$cpu*1000/$states.Count}else{$null});Reason=$reason;MachineInput=$true;TraceSHA256=(Get-FileHash -LiteralPath $file.FullName).Hash}
})
$valid=@($rows|Where-Object Valid)
[ordered]@{SchemaVersion=1;RecordedAtUtc=[datetime]::UtcNow.ToString('o');Edition=$session.Edition;SHA256=$session.SHA256;ExpectedCatalogCount=$session.ExpectedCatalogCount;ValidQueries=$valid.Count;InvalidQueries=$rows.Count-$valid.Count;Observations=$rows;CPUCounterUpperMilliseconds=(Stats @($valid.CPUCounterUpperMilliseconds));MachineCPUPercentUpperOver500ms=(Stats @($valid.MachineCPUPercentUpperOver500ms));AverageCPUCounterMillisecondsPerChangedState=(Stats @($valid.AverageCPUCounterMillisecondsPerChangedState));ChangedStateIntervalsMilliseconds=(Stats @($valid.ChangedResultStateIntervalsMilliseconds));ObservedMinimumPositiveProcessCPUCounterStepMilliseconds=$(if($steps.Count){($steps|Measure-Object -Minimum).Minimum*1000}else{$null});ResourceSamples=$snapshots.Count;CompleteResourceSamples=@($snapshots|Where-Object Complete).Count;ResourceObserverCPUSeconds=$capture.ObserverCPUSeconds;ResourceCollectionMilliseconds=(Stats @($snapshots.CollectionMilliseconds));FrameObserverCPUSeconds=$end.observer_cpu_100ns/10000000.0;RawCPUCaptureSHA256=(Get-FileHash -LiteralPath (Join-Path $RunDirectory 'cpu-resources.json')).Hash;RawFrameCaptureSHA256=(Get-FileHash -LiteralPath (Join-Path $RunDirectory 'frames/events.jsonl')).Hash;NormalExit=$completion.ExitCode;MachineInput=$true;GPUExecutionTimeVerified=$false;PhysicalInputLatencyClaim=$false;Scope='CPU accounting for the full recorded process tree during 500ms query+animation windows, bracketed conservatively; observed changed desktop-frame cadence. CPU counter granularity and capture overhead retained. Average CPU per changed state is an aggregate, not exact individual GPU/CPU render work.';RS12Complete=$false}|ConvertTo-Json -Depth 12|Set-Content -LiteralPath $OutputPath -Encoding utf8NoBOM
[ordered]@{Edition=$session.Edition;Catalog=$session.ExpectedCatalogCount;Valid=$valid.Count;Invalid=$rows.Count-$valid.Count;CPUUpperMilliseconds=(Stats @($valid.CPUCounterUpperMilliseconds));ChangedFrameIntervalsMilliseconds=(Stats @($valid.ChangedResultStateIntervalsMilliseconds))}|ConvertTo-Json -Depth 6
