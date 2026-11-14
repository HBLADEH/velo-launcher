#Requires -Version 7.4
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$CaptureDirectory,
    [Parameter(Mandatory)][string]$OutputPath,
    [string]$SessionDirectory,
    [long]$ClockSegmentStartQpc=0,
    [switch]$AllowLegacyGeometryForDiagnostics
)
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'lib/resource-sample.ps1')
if(Test-Path -LiteralPath $OutputPath){throw 'Preserve earlier frame summaries; use a new OutputPath.'}
$log=Join-Path $CaptureDirectory 'events.jsonl'
$events=@(Read-VeloEventLines $log|ForEach-Object{$_|ConvertFrom-Json})
$ready=@($events|Where-Object event -eq 'capture_ready')
$end=@($events|Where-Object event -eq 'end')
if($ready.Count -ne 1 -or $end.Count -ne 1 -or $ready[0].qpc_frequency -le 0){throw 'Incomplete native frame capture; preserve raw records.'}
$frequency=[double]$ready[0].qpc_frequency
$frames=@($events|Where-Object event -eq 'frame')
if(-not $frames.Count -or @($frames|Where-Object{$_.present_qpc -le 0 -or $_.present_qpc -gt $_.observed_qpc}).Count){throw 'Invalid desktop present/read clock ordering.'}
$geometryChecked=$ready[0].capture_schema -ge 2 -and $ready[0].geometry_freshness_checked -and @($frames|Where-Object{$_.geometry_observed_qpc -le 0 -or $_.present_qpc -lt $_.geometry_observed_qpc}).Count -eq 0
if($SessionDirectory -and -not $geometryChecked -and -not $AllowLegacyGeometryForDiagnostics){throw 'Legacy geometry capture is development-only; product frame timing requires a fresh geometry-checked recording.'}
$clocks=@($events|Where-Object event -eq 'clock')
if($clocks.Count -lt 2 -or @($clocks|Where-Object{$_.before_qpc -gt $_.after_qpc}).Count){throw 'Missing or invalid QPC/UTC clock brackets.'}
$allClocks=$clocks
if($ClockSegmentStartQpc -gt 0){
    if(-not @($clocks|Where-Object before_qpc -eq $ClockSegmentStartQpc).Count){throw 'Clock segment must begin at an original clock bracket.'}
    $clocks=@($clocks|Where-Object before_qpc -ge $ClockSegmentStartQpc)
    if($clocks.Count -lt 2){throw 'Clock segment needs at least two original clock brackets.'}
}
$origin=$clocks[0]
$originMid=([double]$origin.before_qpc+[double]$origin.after_qpc)/2
$offsets=@($clocks|ForEach-Object{
    (([decimal]$_.utc_filetime_100ns-[decimal]$origin.utc_filetime_100ns)/10000.0)-((([double]$_.before_qpc+[double]$_.after_qpc)/2-$originMid)*1000/$frequency)
})
$offsetRange=($offsets|Measure-Object -Maximum).Maximum-($offsets|Measure-Object -Minimum).Minimum
$bracketMax=($clocks|ForEach-Object{($_.after_qpc-$_.before_qpc)*1000/$frequency}|Measure-Object -Maximum).Maximum
if($offsetRange -gt 1){throw 'QPC/UTC mapping moved by more than 1ms; timing summary rejected.'}
function Stats([double[]]$Values){
    $sorted=@($Values|Sort-Object)
    if(-not $sorted.Count){return $null}
    $middle=[int][math]::Floor($sorted.Count/2)
    [ordered]@{Count=$sorted.Count;Minimum=$sorted[0];Median=$(if($sorted.Count%2){$sorted[$middle]}else{($sorted[$middle-1]+$sorted[$middle])/2});P95=$sorted[[int][math]::Ceiling(.95*$sorted.Count)-1];Maximum=$sorted[-1]}
}
$calibrationRows=@()
if($ready[0].calibration){
    $paints=@($events|Where-Object event -eq 'pattern_paint')
    # The desktop can present repeatedly while this target's pattern is unchanged.
    # Calibrate each painted sequence at its first observed desktop presentation.
    $patterns=@($frames|Where-Object pattern -gt 0|Group-Object pattern|ForEach-Object{$_.Group|Sort-Object present_qpc|Select-Object -First 1})
    $calibrationRows=@(foreach($frame in $patterns){
        $paint=@($paints|Where-Object sequence -eq $frame.pattern|Select-Object -Last 1)
        if($paint.Count -ne 1 -or $frame.present_qpc -lt $paint[0].end_qpc){throw 'Decoded pattern does not have a preceding completed paint.'}
        [ordered]@{Sequence=$frame.pattern;PaintToDesktopPresentMilliseconds=($frame.present_qpc-$paint[0].begin_qpc)*1000/$frequency;DesktopPresentToReadMilliseconds=($frame.observed_qpc-$frame.present_qpc)*1000/$frequency;AccumulatedDesktopFrames=$frame.accumulated_frames}
    })
    if($calibrationRows.Count -lt 20){throw 'Insufficient decoded pattern frames for calibration.'}
}
$inputRows=@()
if($SessionDirectory){
    $session=Read-VeloMetadataJson (Join-Path $SessionDirectory 'session.json')
    if($session.ProcessId -ne $ready[0].pid){throw 'Frame capture and business session PID differ.'}
    $expectedCreation=[datetime]::new([long]$session.StartedTicks,[DateTimeKind]::Utc).ToFileTimeUtc()
    if($ready[0].creation_filetime_100ns -ne $expectedCreation){throw 'Frame capture and business process creation identity differ.'}
    $completion=Read-VeloMetadataJson (Join-Path $SessionDirectory 'completion.json')
    $inputEvents=@(Read-VeloEventLines $session.MeasurementLog|ForEach-Object{
        $fields=@{}
        foreach($part in $_.Split(' ')){if($part -match '^([^=]+)=(.*)$'){$fields[$Matches[1]]=$Matches[2]}}
        $fields
    })
    $loss=@($inputEvents|Where-Object{$_.pid -eq [string]$session.ProcessId -and $_.dropped -ne '0'}).Count -gt 0
    $normalExit=$completion.ExitCode -eq 0 -and -not $completion.ForcedCleanup
    $eligible=@($inputEvents|Where-Object{$_.event -eq 'input_accepted' -and $_.pid -eq [string]$session.ProcessId -and $_.focused -eq 'true' -and $_.native_focused -eq 'true' -and $_.visible -eq 'true' -and $_.composing -eq 'false' -and $_.synthetic -ne 'true' -and $_.trusted -ne 'false' -and [int]$_.utf8_bytes -gt 0})
    function InputQpc($Event){
        $utc=[decimal]$Event.unix_us*10+[datetime]::UnixEpoch.ToFileTimeUtc()
        $originMid+[double](($utc-[decimal]$origin.utc_filetime_100ns)/10000000)*$frequency
    }
    $keys=@($events|Where-Object{$_.event -eq 'key' -and $_.kind -eq 'fixed_a' -and -not $_.injected -and -not $_.control -and -not $_.shift -and -not $_.alt -and $_.foreground_target -and $_.visible})
    foreach($key in $keys){
        $nextKey=$events|Where-Object{$_.event -eq 'key' -and $_.qpc -gt $key.qpc}|Select-Object -First 1
        $deadline=if($nextKey){[double]$nextKey.qpc}else{[double]$end[0].qpc}
        $input=@($eligible|Where-Object{(InputQpc $_) -ge $key.event_lower_qpc -and (InputQpc $_) -lt $deadline}|Select-Object -First 1)
        $before=@($frames|Where-Object{$_.present_qpc -lt $key.event_lower_qpc}|Select-Object -Last 1)
        $changed=@(if($before.Count){$frames|Where-Object{$_.present_qpc -ge $key.qpc -and $_.present_qpc -lt $deadline -and $_.foreground_target -and $_.results_hash -ne $before[0].results_hash}|Select-Object -First 1})
        $clockCovered=$key.event_lower_qpc -ge $clocks[0].before_qpc -and $input.Count -eq 1 -and (InputQpc $input[0]) -le $clocks[-1].after_qpc -and $changed.Count -eq 1 -and $changed[0].present_qpc -le $clocks[-1].after_qpc
        $valid=$normalExit -and -not $loss -and $input.Count -eq 1 -and $before.Count -eq 1 -and $changed.Count -eq 1 -and $clockCovered
        $inputRows += [ordered]@{Valid=$valid;ClockSegmentCovered=$clockCovered;KeyObservedQpc=$key.qpc;KeyEventLowerQpc=$key.event_lower_qpc;AcceptedInputQpc=$(if($input.Count){InputQpc $input[0]}else{$null});FirstChangedResultCropQpc=$(if($changed.Count){$changed[0].present_qpc}else{$null});KeyToChangedResultCropUpperMilliseconds=$(if($valid){($changed[0].present_qpc-$key.event_lower_qpc)*1000/$frequency}else{$null});KeyToAcceptedInputUpperMilliseconds=$(if($valid){((InputQpc $input[0])-$key.event_lower_qpc)*1000/$frequency}else{$null});Scope='Physical fixed A key to accepted edit and first changed result crop. A crop change is not independent proof of correct result text or first possible readiness.'}
    }
}
$report=[ordered]@{
    SchemaVersion=1;RecordedAtUtc=[datetime]::UtcNow.ToString('o');Method='DXGI Desktop Duplication LastPresentTime QPC to cropped pixels; separate callback/read times.'
    RawLogSHA256=(Get-FileHash -LiteralPath $log).Hash;Calibration=$ready[0].calibration;Frames=$frames.Count;ChangedFrames=@($frames|Where-Object changed).Count;RejectedFrames=$end[0].rejected_frames
    GeometryFreshnessVerified=[bool]$geometryChecked;LegacyGeometryDiagnosticsOnly=[bool]($AllowLegacyGeometryForDiagnostics -and -not $geometryChecked);ProductFrameEvidenceEligible=[bool]($geometryChecked -and -not $ready[0].calibration -and $SessionDirectory)
    MaximumAccumulatedFrames=($frames|Measure-Object accumulated_frames -Maximum).Maximum;FrameReadLagMilliseconds=(Stats @($frames|ForEach-Object{($_.observed_qpc-$_.present_qpc)*1000/$frequency}))
    ClockOffsetRangeMilliseconds=$offsetRange;ClockBracketMaximumMilliseconds=$bracketMax;QpcFrequency=$frequency
    ClockSegmentStartQpc=$ClockSegmentStartQpc;SelectedClockPairs=$clocks.Count;TotalClockPairs=$allClocks.Count;PhysicalKeysOutsideSelectedClockSegment=@($inputRows|Where-Object{-not $_.ClockSegmentCovered}).Count
    ObserverCPUSeconds=$(if($end[0].PSObject.Properties['observer_cpu_100ns']){$end[0].observer_cpu_100ns/10000000.0}else{$null})
    ObserverElapsedSeconds=$(if($end[0].PSObject.Properties['observer_elapsed_qpc']){$end[0].observer_elapsed_qpc/$frequency}else{$null})
    CalibrationRows=$calibrationRows;CalibrationPaintToDesktopPresentMilliseconds=(Stats @($calibrationRows|ForEach-Object{$_.PaintToDesktopPresentMilliseconds}))
    RepeatedPatternFrames=$(if($ready[0].calibration){@($frames|Where-Object pattern -gt 0).Count-$calibrationRows.Count}else{$null})
    PhysicalInputRows=$inputRows;PhysicalFixedAKeys=$inputRows.Count;ValidPhysicalCropChangeRows=@($inputRows|Where-Object{$_.Valid}).Count
    KeyToChangedResultCropUpperMilliseconds=(Stats @($inputRows|Where-Object{$_.Valid}|ForEach-Object{$_.KeyToChangedResultCropUpperMilliseconds}))
    FirstPossibleInputLatencyVerified=$false;CorrectResultFrameIndependentlyVerified=$false;PhotonPresentationVerified=$false;RS12Complete=$false
    Limitations=@('Composed desktop timestamp, not photon scanout; occluded/hidden/out-of-monitor crops rejected.','Background updates can change the desktop with identical target pixels; only changed crop hashes describe target changes.','Accumulated frames may omit intermediate states. Pixel hashes alone do not prove correct results or quantify GPU rendering work.','The physical key lower bound retains Windows event-clock quantum and callback delay. Actual input is paired using bracketed QPC/UTC mapping.','No subtraction of human startup delay, no first possible readiness or calibrated cold startup claim.')
}
$report|ConvertTo-Json -Depth 12|Set-Content -LiteralPath $OutputPath -Encoding utf8NoBOM
[ordered]@{Frames=$frames.Count;Calibration=$ready[0].calibration;ClockOffsetRangeMilliseconds=$offsetRange;PhysicalFixedAKeys=$inputRows.Count;ValidPhysicalCropChangeRows=$report.ValidPhysicalCropChangeRows}|ConvertTo-Json
