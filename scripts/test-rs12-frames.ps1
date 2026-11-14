#Requires -Version 7.4
$ErrorActionPreference='Stop'
function Assert($Condition,[string]$Message){if(-not $Condition){throw $Message}}
$fixture=Join-Path ([IO.Path]::GetTempPath()) ('velo-rs12-frame-check-'+[guid]::NewGuid().ToString('N'))
try{
    New-Item -ItemType Directory -Path $fixture|Out-Null
    $rows=@(
        @{event='clock';before_qpc=1000;after_qpc=1002;utc_filetime_100ns=133000000000000000},
        @{event='capture_ready';qpc=1100;qpc_frequency=1000000;pid=42;calibration=$true}
    )
    foreach($number in 1..20){
        $paint=2000+$number*1000
        $rows+=@{event='pattern_paint';sequence=$number;begin_qpc=$paint;end_qpc=$paint+100}
        $rows+=@{event='frame';present_qpc=$paint+500;observed_qpc=$paint+900;accumulated_frames=1;pattern=$number;changed=$true}
    }
    $rows+=@{event='clock';before_qpc=101000;after_qpc=101002;utc_filetime_100ns=133000000001000000}
    $rows+=@{event='end';qpc=102000;rejected_frames=0}
    $log=Join-Path $fixture 'events.jsonl'
    $rows|ForEach-Object{$_|ConvertTo-Json -Compress}|Set-Content -LiteralPath $log -Encoding utf8NoBOM
    $output=Join-Path $fixture 'summary.json'
    $null=& (Join-Path $PSScriptRoot 'summarize-rs12-frames.ps1') -CaptureDirectory $fixture -OutputPath $output
    $summary=Get-Content -Raw -LiteralPath $output|ConvertFrom-Json
    Assert ($summary.CalibrationRows.Count -eq 20 -and $summary.CalibrationPaintToDesktopPresentMilliseconds.Median -eq .5) 'Use the actual present timestamp, not the later screenshot/read time.'
    Assert (-not $summary.PhotonPresentationVerified -and -not $summary.CorrectResultFrameIndependentlyVerified -and -not $summary.FirstPossibleInputLatencyVerified) 'Calibration must not upgrade unrelated result/readiness/scanout claims.'
    $rejected=$false
    try{& (Join-Path $PSScriptRoot 'summarize-rs12-frames.ps1') -CaptureDirectory $fixture -OutputPath $output}catch{$rejected=$_.Exception.Message -like 'Preserve earlier*'}
    Assert $rejected 'Earlier frame summaries must not be overwritten.'
    $rows[3].observed_qpc=$rows[3].present_qpc-1
    $rows|ForEach-Object{$_|ConvertTo-Json -Compress}|Set-Content -LiteralPath $log -Encoding utf8NoBOM
    $rejected=$false
    try{& (Join-Path $PSScriptRoot 'summarize-rs12-frames.ps1') -CaptureDirectory $fixture -OutputPath (Join-Path $fixture 'bad-order.json')}catch{$rejected=$_.Exception.Message -like 'Invalid desktop present/read*'}
    Assert $rejected 'A frame timestamp later than its read is invalid.'
    $rows[3].observed_qpc=$rows[3].present_qpc+400
    $rows[-2].utc_filetime_100ns+=100000
    $rows|ForEach-Object{$_|ConvertTo-Json -Compress}|Set-Content -LiteralPath $log -Encoding utf8NoBOM
    $rejected=$false
    try{& (Join-Path $PSScriptRoot 'summarize-rs12-frames.ps1') -CaptureDirectory $fixture -OutputPath (Join-Path $fixture 'bad-clock.json')}catch{$rejected=$_.Exception.Message -like 'QPC/UTC mapping moved*'}
    Assert $rejected 'A wall-clock step must not produce an apparently precise timing result.'
    $created=133000000000000000L
    $unixFiletime=[datetime]::UnixEpoch.ToFileTimeUtc()
    $inputUnixUs=[long](($created+1395000-$unixFiletime)/10)
    $rows=@(
        @{event='clock';before_qpc=1000;after_qpc=1002;utc_filetime_100ns=$created},
        @{event='capture_ready';qpc=1100;qpc_frequency=1000000;pid=42;creation_filetime_100ns=$created;calibration=$false;capture_schema=2;geometry_freshness_checked=$true},
        @{event='key';kind='fixed_a';qpc=60000;event_lower_qpc=59900;injected=$false;control=$false;shift=$false;alt=$false;foreground_target=$true;visible=$true},
        @{event='clock';before_qpc=101000;after_qpc=101002;utc_filetime_100ns=$created+1100000},
        @{event='frame';present_qpc=129000;observed_qpc=129100;geometry_observed_qpc=120000;accumulated_frames=1;pattern=-1;changed=$false;results_hash='before';foreground_target=$true},
        @{event='key';kind='fixed_a';qpc=130000;event_lower_qpc=129900;injected=$false;control=$false;shift=$false;alt=$false;foreground_target=$true;visible=$true},
        @{event='frame';present_qpc=130700;observed_qpc=131000;geometry_observed_qpc=120000;accumulated_frames=1;pattern=-1;changed=$true;results_hash='after';foreground_target=$true},
        @{event='clock';before_qpc=201000;after_qpc=201002;utc_filetime_100ns=$created+2100000},
        @{event='end';qpc=202000;rejected_frames=0}
    )
    $rows|ForEach-Object{$_|ConvertTo-Json -Compress}|Set-Content -LiteralPath $log -Encoding utf8NoBOM
    $inputLog=Join-Path $fixture 'input-events.log'
    "event=input_accepted pid=42 unix_us=$inputUnixUs dropped=0 utf8_bytes=1 focused=true native_focused=true visible=true composing=false synthetic=false"|Set-Content -LiteralPath $inputLog -Encoding utf8NoBOM
    @{ProcessId=42;StartedTicks=[datetime]::FromFileTimeUtc($created).Ticks;MeasurementLog=$inputLog}|ConvertTo-Json|Set-Content -LiteralPath (Join-Path $fixture 'session.json') -Encoding utf8NoBOM
    @{ExitCode=0;ForcedCleanup=$false}|ConvertTo-Json|Set-Content -LiteralPath (Join-Path $fixture 'completion.json') -Encoding utf8NoBOM
    $null=& (Join-Path $PSScriptRoot 'summarize-rs12-frames.ps1') -CaptureDirectory $fixture -SessionDirectory $fixture -ClockSegmentStartQpc 101000 -OutputPath (Join-Path $fixture 'segmented.json')
    $segmented=Get-Content -Raw -LiteralPath (Join-Path $fixture 'segmented.json')|ConvertFrom-Json
    Assert ($segmented.PhysicalInputRows.Count -eq 2 -and $segmented.ValidPhysicalCropChangeRows -eq 1 -and $segmented.PhysicalKeysOutsideSelectedClockSegment -eq 1) 'Explicit clock selection must retain out-of-segment physical keys as invalid instead of silently deleting them.'
    Assert ($segmented.KeyToChangedResultCropUpperMilliseconds.Median -eq .8) 'Physical-to-crop timing must use QPC and the conservative event lower bound.'
    $rejected=$false
    try{& (Join-Path $PSScriptRoot 'summarize-rs12-frames.ps1') -CaptureDirectory $fixture -ClockSegmentStartQpc 100999 -OutputPath (Join-Path $fixture 'invented-segment.json')}catch{$rejected=$_.Exception.Message -like 'Clock segment must begin*'}
    Assert $rejected 'A selected clock region must begin at a preserved original clock sample.'
    $rows[1].creation_filetime_100ns++
    $rows|ForEach-Object{$_|ConvertTo-Json -Compress}|Set-Content -LiteralPath $log -Encoding utf8NoBOM
    $rejected=$false
    try{& (Join-Path $PSScriptRoot 'summarize-rs12-frames.ps1') -CaptureDirectory $fixture -SessionDirectory $fixture -ClockSegmentStartQpc 101000 -OutputPath (Join-Path $fixture 'reused-pid.json')}catch{$rejected=$_.Exception.Message -like 'Frame capture and business process creation*'}
    Assert $rejected 'A reused PID must not inherit frame/input evidence from another process creation.'
    $rows[1].creation_filetime_100ns--
    $rows[1].capture_schema=1
    $rows|ForEach-Object{$_|ConvertTo-Json -Compress}|Set-Content -LiteralPath $log -Encoding utf8NoBOM
    $rejected=$false
    try{& (Join-Path $PSScriptRoot 'summarize-rs12-frames.ps1') -CaptureDirectory $fixture -SessionDirectory $fixture -ClockSegmentStartQpc 101000 -OutputPath (Join-Path $fixture 'legacy.json')}catch{$rejected=$_.Exception.Message -like 'Legacy geometry capture is development-only*'}
    Assert $rejected 'Old geometry captures must not silently become accepted product frame evidence.'
    foreach($name in 'build-rs12-observer.ps1','observe-rs12-frames.ps1','prepare-rs12-frame-fixtures.ps1','summarize-rs12-frames.ps1','test-rs12-frames.ps1','test-rs12-frame-native.ps1'){
        $tokens=$null;$errors=$null
        [void][System.Management.Automation.Language.Parser]::ParseFile((Join-Path $PSScriptRoot $name),[ref]$tokens,[ref]$errors)
        Assert (-not $errors) ('PowerShell parse errors: '+$name)
    }
}finally{
    if(Test-Path -LiteralPath $fixture){
        $resolved=(Resolve-Path -LiteralPath $fixture).Path
        $temporary=[IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\')+'\'
        if(-not $resolved.StartsWith($temporary,[StringComparison]::OrdinalIgnoreCase) -or (Split-Path -Leaf $resolved) -notlike 'velo-rs12-frame-check-*'){throw 'Unexpected frame fixture cleanup path.'}
        Remove-Item -LiteralPath $resolved -Recurse -Force
    }
}
'RS-12 frame summary checks passed (present/read boundary, clock steps/segments, physical-key retention, creation identity and claim boundaries). No desktop or input hook was used.'
