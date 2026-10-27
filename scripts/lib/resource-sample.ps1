# Helpers shared by the collector and its dependency-free regression checks.
function Read-VeloMetadataJson {
    param([string]$Path)
    try {
        # Go writes UTF-8 without a BOM; Windows PowerShell 5.1 otherwise uses
        # the system code page. Keep parser errors from dumping the user's index.
        [IO.File]::ReadAllText($Path, [Text.Encoding]::UTF8) | ConvertFrom-Json -ErrorAction Stop
    } catch {
        throw "Cannot read JSON metadata at '$Path'; check UTF-8 encoding and JSON syntax."
    }
}

function Get-VeloProcessTree {
    param([object[]]$Processes, [int]$RootProcessId)
    $ids = [System.Collections.Generic.HashSet[int]]::new()
    [void]$ids.Add($RootProcessId)
    do {
        $added = $false
        foreach ($entry in $Processes) {
            if ($entry.Name -eq 'msedgewebview2.exe' -and $ids.Contains([int]$entry.ParentProcessId) -and $ids.Add([int]$entry.ProcessId)) { $added = $true }
        }
    } while ($added)
    $Processes | Where-Object { $ids.Contains([int]$_.ProcessId) }
}

function Get-VeloSampleSummary {
    param([object[]]$Snapshots, [string]$ExpectedWindowState, [bool]$IdleConfirmed, [bool]$DataStable)
    if ($Snapshots.Count -lt 2) { throw 'At least two snapshots are required.' }
    $first = $Snapshots[0]
    $last = $Snapshots[-1]
    $elapsed = $last.MonotonicSeconds - $first.MonotonicSeconds
    if ($elapsed -le 0) { throw 'Sampling clock must advance.' }
    $identities = @($first.Processes.Identity | Sort-Object)
    $cpuValid = $true
    $reason = $null
    $previousCPU = $null
    foreach ($snapshot in $Snapshots) {
        $currentCPU = ($snapshot.Processes | Measure-Object CPUSeconds -Sum).Sum
        if (-not $snapshot.Complete -or (Compare-Object $identities @($snapshot.Processes.Identity | Sort-Object))) {
            $cpuValid = $false
            $reason = 'Process tree changed or a process could not be read.'
        }
        if ($null -ne $previousCPU -and $currentCPU -lt $previousCPU) {
            $cpuValid = $false
            $reason = 'CPU counters decreased during sampling.'
        }
        $previousCPU = $currentCPU
    }
    $cpuPercent = $null
    if ($cpuValid) {
        $delta = ($last.Processes | Measure-Object CPUSeconds -Sum).Sum - ($first.Processes | Measure-Object CPUSeconds -Sum).Sum
        $cpuPercent = [math]::Round($delta / $elapsed / [Environment]::ProcessorCount * 100, 6)
    }
    $states = @($Snapshots.Window.State | Select-Object -Unique)
    $windowConfirmed = $states.Count -eq 1 -and $states[0] -in @('Visible', 'Hidden')
    $windowMatches = $windowConfirmed -and ($ExpectedWindowState -eq 'Unknown' -or $states[0] -eq $ExpectedWindowState)
    $eligible = $cpuValid -and $windowMatches -and $IdleConfirmed -and $DataStable
    [pscustomobject]@{
        StartedAtUtc = $first.TimestampUtc
        EndedAtUtc = $last.TimestampUtc
        ProcessCount = $last.Processes.Count
        IncludesWebViewChildren = $true
        SampleSeconds = [math]::Round($elapsed, 3)
        CPUSampleValid = $cpuValid
        CPUPercentOfMachine = $cpuPercent
        CPUInvalidReason = $reason
        WindowState = $(if ($windowConfirmed) { $states[0] } else { 'Unknown' })
        WindowStateConfirmed = $windowConfirmed
        WindowStateMatches = $windowMatches
        IdleConfirmed = $IdleConfirmed
        DataStable = $DataStable
        BaselineEligible = $eligible
        PrivateBytes = $last.PrivateBytes
        WorkingSet = $last.WorkingSet
        PrivateBytesMB = [math]::Round($last.PrivateBytes / 1MB, 2)
        WorkingSetMB = [math]::Round($last.WorkingSet / 1MB, 2)
        # Endpoints preserve the old script's convention; range/mean describe all polls.
        PrivateBytesMean = [math]::Round(($Snapshots | Measure-Object PrivateBytes -Average).Average)
        WorkingSetMean = [math]::Round(($Snapshots | Measure-Object WorkingSet -Average).Average)
        PrivateBytesMin = ($Snapshots | Measure-Object PrivateBytes -Minimum).Minimum
        PrivateBytesMax = ($Snapshots | Measure-Object PrivateBytes -Maximum).Maximum
        WorkingSetMin = ($Snapshots | Measure-Object WorkingSet -Minimum).Minimum
        WorkingSetMax = ($Snapshots | Measure-Object WorkingSet -Maximum).Maximum
        PollCount = $Snapshots.Count
        Processes = $last.Processes
        WindowObservations = @($Snapshots | Select-Object TimestampUtc, Window)
    }
}
