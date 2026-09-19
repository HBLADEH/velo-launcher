param([int]$ProcessId, [int]$Seconds = 15)
$ErrorActionPreference = 'Stop'
if ($ProcessId -le 0 -or $Seconds -lt 1 -or $Seconds -gt 60) { throw 'Supply a Velo process ID and 1–60 seconds.' }
function Snapshot {
    $all = @(Get-CimInstance Win32_Process)
    $ids = [System.Collections.Generic.HashSet[int]]::new()
    [void]$ids.Add($ProcessId)
    do {
        $added = $false
        foreach ($entry in $all) {
            # Applications launched by Velo (e.g. Code) are not launcher cost.
            if ($entry.Name -eq 'msedgewebview2.exe' -and $ids.Contains([int]$entry.ParentProcessId) -and $ids.Add([int]$entry.ProcessId)) { $added = $true }
        }
    } while ($added)
    $processes = @(foreach ($id in $ids) { Get-Process -Id $id -ErrorAction SilentlyContinue })
    [pscustomobject]@{
        Timestamp = [DateTime]::UtcNow
        Count = $processes.Count
        CPUSeconds = ($processes | Measure-Object CPU -Sum).Sum
        WorkingSetMB = [math]::Round(($processes | Measure-Object WorkingSet64 -Sum).Sum / 1MB, 2)
        PrivateBytesMB = [math]::Round(($processes | Measure-Object PrivateMemorySize64 -Sum).Sum / 1MB, 2)
    }
}
$before = Snapshot
Start-Sleep -Seconds $Seconds
$after = Snapshot
$elapsed = ($after.Timestamp - $before.Timestamp).TotalSeconds
[pscustomobject]@{
    ProcessId = $ProcessId
    ProcessCount = $after.Count
    IncludesWebViewChildren = $true
    SampleSeconds = [math]::Round($elapsed, 2)
    CPUPercentOfMachine = [math]::Round(($after.CPUSeconds - $before.CPUSeconds) / $elapsed / [Environment]::ProcessorCount * 100, 3)
    WorkingSetMB = $after.WorkingSetMB
    PrivateBytesMB = $after.PrivateBytesMB
} | ConvertTo-Json
