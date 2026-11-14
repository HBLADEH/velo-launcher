#Requires -Version 7.4
$ErrorActionPreference='Stop'
function Assert($Condition,[string]$Message){if(-not $Condition){throw $Message}}
$fixture=Join-Path ([IO.Path]::GetTempPath()) ('velo-rs12-anim-check-'+[guid]::NewGuid().ToString('N'))
try{
    New-Item -ItemType Directory -Path (Join-Path $fixture 'frames') -Force|Out-Null
    $created=133000000000000000L;$ticks=[datetime]::FromFileTimeUtc($created).Ticks
    @{Edition='wails';ProcessId=42;StartedTicks=$ticks;SHA256=('a'*64);ExpectedCatalogCount=1000}|ConvertTo-Json|Set-Content -LiteralPath (Join-Path $fixture 'session.json') -Encoding utf8NoBOM
    @{Completed=$true;Exited=$true;ExitCode=0;ForcedCleanup=$false}|ConvertTo-Json|Set-Content -LiteralPath (Join-Path $fixture 'completion.json') -Encoding utf8NoBOM
    $snapshots=@(for($index=0;$index -le 28;$index++){@{StartedQpc=$index*20000;EndedQpc=$index*20000+100;CollectionMilliseconds=.1;Complete=$true;Processes=@(@{ProcessId=42;StartedTicks=$ticks;CPUSeconds=$index*.001},@{ProcessId=43;StartedTicks=$ticks+1;CPUSeconds=$index*.002})}})
    $capture=@{ProcessId=42;StartedTicks=$ticks;QpcFrequency=1000000;Snapshots=$snapshots;ObserverCPUSeconds=.01}
    $cpuPath=Join-Path $fixture 'cpu-resources.json';$capture|ConvertTo-Json -Depth 10|Set-Content -LiteralPath $cpuPath -Encoding utf8NoBOM
    $frames=@(@{event='capture_ready';pid=42;creation_filetime_100ns=$created;qpc_frequency=1000000;geometry_freshness_checked=$true})
    foreach($number in 1..12){$frames+=@{event='frame';present_qpc=10000+$number*16000;width=616;height=564;results_hash=[string]$number;accumulated_frames=1}}
    $frames+=@{event='end';target_ended=$true;observer_cpu_100ns=1000}
    $frames|ForEach-Object{$_|ConvertTo-Json -Compress}|Set-Content -LiteralPath (Join-Path $fixture 'frames/events.jsonl') -Encoding utf8NoBOM
    @(@{event='phase_start';start_lower_qpc=10000},@{event='accepted';unix_us=1})|ForEach-Object{$_|ConvertTo-Json -Compress}|Set-Content -LiteralPath (Join-Path $fixture 'query-1.jsonl') -Encoding utf8NoBOM
    $null=& (Join-Path $PSScriptRoot 'export-rs12-animation.ps1') -RunDirectory $fixture -OutputPath (Join-Path $fixture 'summary.json')
    $summary=Get-Content -Raw -LiteralPath (Join-Path $fixture 'summary.json')|ConvertFrom-Json
    Assert ($summary.ValidQueries -eq 1 -and $summary.Observations[0].ProcessCount -eq 2 -and $summary.Observations[0].CPUCounterUpperMilliseconds -gt 70) 'Animation CPU must include the full same-identity process tree and surrounding interval.'
    Assert ($summary.Observations[0].ChangedResultStates -eq 12 -and $summary.ChangedStateIntervalsMilliseconds.Median -eq 16 -and -not $summary.GPUExecutionTimeVerified) 'Observed desktop frame cadence must not invent GPU execution measurements.'
    $capture.Snapshots[10].Processes[1].StartedTicks++
    $capture|ConvertTo-Json -Depth 10|Set-Content -LiteralPath $cpuPath -Encoding utf8NoBOM
    $null=& (Join-Path $PSScriptRoot 'export-rs12-animation.ps1') -RunDirectory $fixture -OutputPath (Join-Path $fixture 'pid-churn.json')
    $churn=Get-Content -Raw -LiteralPath (Join-Path $fixture 'pid-churn.json')|ConvertFrom-Json
    Assert ($churn.ValidQueries -eq 0 -and $churn.InvalidQueries -eq 1 -and $churn.Observations[0].Reason -like 'CPU process identities changed*') 'Intermediate process reuse must invalidate the animation resource interval.'
    $capture.Snapshots[10].Processes[1].StartedTicks--
    $capture.Snapshots[12].Complete=$false
    $capture|ConvertTo-Json -Depth 10|Set-Content -LiteralPath $cpuPath -Encoding utf8NoBOM
    $null=& (Join-Path $PSScriptRoot 'export-rs12-animation.ps1') -RunDirectory $fixture -OutputPath (Join-Path $fixture 'incomplete.json')
    $incomplete=Get-Content -Raw -LiteralPath (Join-Path $fixture 'incomplete.json')|ConvertFrom-Json
    Assert ($incomplete.ValidQueries -eq 0 -and $incomplete.InvalidQueries -eq 1) 'An unreadable intermediate process sample cannot disappear from CPU accounting.'
}finally{
    if(Test-Path -LiteralPath $fixture){$resolved=(Resolve-Path -LiteralPath $fixture).Path;$temp=[IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\')+'\';if(-not $resolved.StartsWith($temp,[StringComparison]::OrdinalIgnoreCase) -or (Split-Path -Leaf $resolved) -notlike 'velo-rs12-anim-check-*'){throw 'Unexpected animation fixture cleanup.'};Remove-Item -LiteralPath $resolved -Recurse -Force}
}
'RS-12 animation CPU/frame checks passed (full process tree, intermediate PID reuse, incomplete snapshots, measured cadence and GPU claim separation).'
