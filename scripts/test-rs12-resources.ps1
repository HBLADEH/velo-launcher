#Requires -Version 7.4
$ErrorActionPreference = 'Stop'
function Assert($Condition, [string]$Message) { if (-not $Condition) { throw $Message } }
Add-Type -Path (Join-Path $PSScriptRoot 'lib/rs12-resource-observer.cs')
$fixture = Join-Path ([IO.Path]::GetTempPath()) ('velo-rs12-resources-'+[guid]::NewGuid().ToString('N'))
$processes = [Collections.Generic.List[Diagnostics.Process]]::new()
try {
    New-Item -ItemType Directory -Path $fixture | Out-Null
    $childFile = Join-Path $fixture 'child.ps1'
    @'
$ErrorActionPreference='Stop'
$block=[byte[]]::new(32MB)
for($index=0;$index -lt $block.Length;$index+=4096){$block[$index]=1}
Start-Sleep -Seconds 4
[GC]::KeepAlive($block)
'@ | Set-Content -LiteralPath $childFile -Encoding utf8NoBOM
    $parentFile = Join-Path $fixture 'parent.ps1'
    @'
param([string]$ChildFile)
$start=[Diagnostics.ProcessStartInfo]::new((Get-Process -Id $PID).Path)
$start.UseShellExecute=$false
$start.CreateNoWindow=$true
$start.WindowStyle='Hidden'
foreach($arg in @('-NoProfile','-File',$ChildFile)){$start.ArgumentList.Add($arg)}
$child=[Diagnostics.Process]::Start($start)
$child.Id | Set-Content -LiteralPath (Join-Path $PSScriptRoot 'child-id.txt')
while(-not $child.WaitForExit(100)){}
$child.Dispose()
Start-Sleep -Seconds 1
'@ | Set-Content -LiteralPath $parentFile -Encoding utf8NoBOM
    function Start-Fixture([string]$Path, [string[]]$Arguments) {
        $start = [Diagnostics.ProcessStartInfo]::new((Get-Process -Id $PID).Path)
        $start.UseShellExecute=$false; $start.CreateNoWindow=$true; $start.WindowStyle='Hidden'
        foreach ($argument in @('-NoProfile','-File',$Path)+$Arguments) { $start.ArgumentList.Add($argument) }
        $process = [Diagnostics.Process]::Start($start)
        $processes.Add($process)
        $process
    }
    # A simultaneous sibling must never be counted in the target's tree.
    $sibling = Start-Fixture $childFile @()
    $parent = Start-Fixture $parentFile @($childFile)
    $created = $parent.StartTime.ToUniversalTime().Ticks
    $task = [VeloRS12Resources.Observer]::Start($parent.Id, $created, 10, 20)
    $capture = $task.GetAwaiter().GetResult()
    $snapshots = @($capture.Snapshots)
    Assert ($snapshots.Count -gt 30) 'Expected a high-frequency native capture, not a single endpoint.'
    $childId = [int](Get-Content -LiteralPath (Join-Path $fixture 'child-id.txt'))
    $withChild = @($snapshots | Where-Object { $_.Processes.ProcessId -contains $childId })
    Assert ($withChild.Count -gt 10) 'The resource capture missed the actual allocating descendant.'
    Assert (@($snapshots | Where-Object { $_.Processes.ProcessId -contains $sibling.Id }).Count -eq 0) 'An unrelated sibling contaminated the process tree.'
    Assert (($withChild | Where-Object Complete | Measure-Object PrivateBytes -Maximum).Maximum -gt 32MB) 'Descendant private commit was omitted from the aggregate.'
    foreach ($snapshot in $snapshots) {
        Assert ($snapshot.PrivateBytes -eq ($snapshot.Processes | Measure-Object PrivateBytes -Sum).Sum) 'Private commit must sum the same captured process records.'
        Assert ($snapshot.StartedQpc -gt 0 -and $snapshot.EndedQpc -ge $snapshot.StartedQpc) 'CPU/frame correlation requires a monotonic bracket around each actual resource read.'
    }
    Assert ($capture.QpcFrequency -eq [Diagnostics.Stopwatch]::Frequency) 'Resource and native frame sampling must expose the actual shared QPC frequency.'
    Assert ($capture.RootExited) 'A root exit must end the capture without following a reused PID.'
    $self = Get-Process -Id $PID
    $mismatch = [VeloRS12Resources.Observer]::Start($PID, $self.StartTime.ToUniversalTime().Ticks+1, 1, 20).GetAwaiter().GetResult()
    Assert ($mismatch.RootExited -and $mismatch.Snapshots.Count -eq 0) 'A mismatched process creation identity must be rejected before any metric is accepted.'
    $identity = @{ ProcessId=$PID; StartedTicks=$self.StartTime.ToUniversalTime().Ticks; Executable=$self.Path; SHA256=(Get-FileHash -LiteralPath $self.Path).Hash; Edition='synthetic-fixture'; MeasurementLog=$null }
    $identity | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $fixture 'session.json')
    & (Join-Path $PSScriptRoot 'measure-rs12-peak.ps1') -SessionDirectory $fixture -Seconds 1 -IntervalMilliseconds 20 -OutputPath (Join-Path $fixture 'resources.json')
    $report = Get-Content -LiteralPath (Join-Path $fixture 'resources.json') -Raw | ConvertFrom-Json
    $maximumCount = ($report.Capture.Snapshots | ForEach-Object { $_.Processes.Count } | Measure-Object -Maximum).Maximum
    Assert ($report.Summary.ProcessCountMaximum -eq $maximumCount -and $maximumCount -lt $report.Summary.CompleteSamples -and $report.ExecutableMatches) 'Peak summary must count processes per snapshot, not flatten processes across the capture.'
    Assert ($report.Summary.ObservedIntervalMedianMilliseconds -ge 10 -and $report.Summary.PrivateBytesSampledMaximum -gt 0) 'Peak summary must retain actual intervals and measured memory.'
    Assert ($null -eq $report.ScanAfter -and $report.UnavailableMetrics.Count -gt 0) 'Resource observation must not invent scan completion, private working set or GPU values.'
    $overwritten=$false
    try { & (Join-Path $PSScriptRoot 'measure-rs12-peak.ps1') -SessionDirectory $fixture -OutputPath (Join-Path $fixture 'resources.json') } catch { $overwritten=$_.Exception.Message -like 'Use a new OutputPath*' }
    Assert $overwritten 'A repeated observation must preserve the earlier evidence file.'
    foreach ($script in 'measure-rs12-peak.ps1','start-rs12-session.ps1','test-rs12-resources.ps1') {
        $tokens=$null; $errors=$null
        [void][System.Management.Automation.Language.Parser]::ParseFile((Join-Path $PSScriptRoot $script),[ref]$tokens,[ref]$errors)
        Assert (-not $errors) "PowerShell parse errors in $script"
    }
} finally {
    foreach ($process in $processes) {
        if (-not $process.HasExited) { $process.Kill($true); $process.WaitForExit() }
        $process.Dispose()
    }
    if (Test-Path -LiteralPath $fixture) {
        $resolved=(Resolve-Path -LiteralPath $fixture).Path
        $temp=[IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\')+'\'
        if (-not $resolved.StartsWith($temp,[StringComparison]::OrdinalIgnoreCase) -or (Split-Path -Leaf $resolved) -notlike 'velo-rs12-resources-*') { throw 'Unexpected resource fixture cleanup path.' }
        Remove-Item -LiteralPath $resolved -Recurse -Force
    }
}
'RS-12 native resource checks passed (real descendant allocation, sibling exclusion, creation identity, root exit, polling intervals and preserved output).'
