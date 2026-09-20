param([int]$ProcessId, [int]$Seconds = 15)
$ErrorActionPreference = 'Stop'
if ($ProcessId -le 0 -or $Seconds -lt 1 -or $Seconds -gt 60) { throw 'Supply a Velo process ID and 1–60 seconds.' }
function Snapshot {
    $all = @(Get-CimInstance Win32_Process)
    $root = $all | Where-Object ProcessId -EQ $ProcessId
    if (-not $root -or $root.Name -notlike 'velo-launcher*.exe') { throw 'ProcessId must identify a running Velo executable.' }
    $ids = [System.Collections.Generic.HashSet[int]]::new()
    [void]$ids.Add($ProcessId)
    do {
        $added = $false
        foreach ($entry in $all) {
            # Applications launched by Velo (e.g. Code) are not launcher cost.
            if ($entry.Name -eq 'msedgewebview2.exe' -and $ids.Contains([int]$entry.ParentProcessId) -and $ids.Add([int]$entry.ProcessId)) { $added = $true }
        }
    } while ($added)
    $processes = @(foreach ($id in $ids) {
        $process = Get-Process -Id $id -ErrorAction SilentlyContinue
        if ($process) {
            [pscustomobject]@{
                Identity = "$id/$($process.StartTime.ToUniversalTime().Ticks)"
                ProcessId = $id
                Name = $process.ProcessName
                CPUSeconds = $process.TotalProcessorTime.TotalSeconds
                WorkingSet64 = $process.WorkingSet64
                PrivateMemorySize64 = $process.PrivateMemorySize64
            }
        }
    })
    if (-not ($processes | Where-Object ProcessId -EQ $ProcessId)) { throw 'Velo exited during sampling.' }
    [pscustomobject]@{
        Timestamp = [DateTime]::UtcNow
        Processes = $processes
        Complete = $processes.Count -eq $ids.Count
        Count = $processes.Count
        WorkingSetMB = [math]::Round(($processes | Measure-Object WorkingSet64 -Sum).Sum / 1MB, 2)
        PrivateBytesMB = [math]::Round(($processes | Measure-Object PrivateMemorySize64 -Sum).Sum / 1MB, 2)
    }
}
$before = Snapshot
Start-Sleep -Seconds $Seconds
$after = Snapshot
$elapsed = ($after.Timestamp - $before.Timestamp).TotalSeconds
# Summing lifetime CPU counters across different process sets can produce a
# negative value or count CPU consumed before the sample. Reject such samples.
$beforeIDs = @($before.Processes.Identity | Sort-Object)
$afterIDs = @($after.Processes.Identity | Sort-Object)
$cpuValid = $before.Complete -and $after.Complete -and -not (Compare-Object $beforeIDs $afterIDs)
$cpuPercent = $null
if ($cpuValid) {
    $cpuDelta = ($after.Processes | Measure-Object CPUSeconds -Sum).Sum - ($before.Processes | Measure-Object CPUSeconds -Sum).Sum
    $cpuValid = $cpuDelta -ge 0
    if ($cpuValid) { $cpuPercent = [math]::Round($cpuDelta / $elapsed / [Environment]::ProcessorCount * 100, 3) }
}
[pscustomobject]@{
    ProcessId = $ProcessId
    ProcessCount = $after.Count
    IncludesWebViewChildren = $true
    SampleSeconds = [math]::Round($elapsed, 2)
    CPUSampleValid = $cpuValid
    CPUPercentOfMachine = $cpuPercent
    CPUInvalidReason = $(if (-not $cpuValid) { 'Process set changed during sampling; retry after the process tree stabilizes.' } else { $null })
    WorkingSetMB = $after.WorkingSetMB
    PrivateBytesMB = $after.PrivateBytesMB
    Processes = @($after.Processes | Select-Object ProcessId, Name, @{Name='WorkingSetMB'; Expression={[math]::Round($_.WorkingSet64 / 1MB, 2)}}, @{Name='PrivateBytesMB'; Expression={[math]::Round($_.PrivateMemorySize64 / 1MB, 2)}})
} | ConvertTo-Json -Depth 4
