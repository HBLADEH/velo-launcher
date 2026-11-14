#Requires -Version 7.4
[CmdletBinding()]
param([Parameter(Mandatory)][string]$PreparedDirectory, [Parameter(Mandatory)][string]$OutputPath)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'lib/resource-sample.ps1')
if (Test-Path -LiteralPath $OutputPath) { throw 'Keep previous summaries; use a new OutputPath.' }
function Median($Values) {
    $sorted = @($Values | Sort-Object)
    if (-not $sorted.Count) { return $null }
    $middle = [int][math]::Floor($sorted.Count / 2)
    if ($sorted.Count % 2) { return $sorted[$middle] }
    ($sorted[$middle - 1] + $sorted[$middle]) / 2
}
$rows = @(foreach ($run in Get-ChildItem -LiteralPath (Join-Path $PreparedDirectory 'runs') -Directory) {
    $completionPath = Join-Path $run.FullName 'completion.json'
    if (-not (Test-Path -LiteralPath $completionPath)) { continue }
    $completion = Read-VeloMetadataJson $completionPath
    if ($completion.ForcedCleanup -or $completion.CheckOnly -or $completion.ExitCode -ne 0) { continue }
    foreach ($name in @('visible', 'hidden-30s', 'hidden-5m', 'visible-after-100-cycles', 'visible-after-loop')) {
        $paths = @(Get-ChildItem -LiteralPath $run.FullName -File | Where-Object { $_.Name -eq ($name + '.json') -or $_.Name -match ('^' + [regex]::Escape($name) + '-attempt-[2-3]\.json$') } | Sort-Object Name)
        $path = $null
        $report = $null
        foreach ($candidate in $paths) {
            $observation = Read-VeloMetadataJson $candidate.FullName
            if ($observation.AllSamplesEligible) { $path = $candidate.FullName; $report = $observation }
        }
        if (-not $path) { continue }
        if ($report.Samples.Count -lt 3 -or $report.DataMode -ne 'Business') { throw "Invalid business baseline: $path" }
        foreach ($sample in $report.Samples) {
            if (-not $sample.BaselineEligible -or $sample.SampleSeconds -lt 15 -or $sample.ProcessCount -lt 1) { throw "Incomplete sample in $path" }
        }
        $private = @($report.Samples.PrivateBytes)
        $working = @($report.Samples.WorkingSet)
        $privateWorking = @($report.Samples | Where-Object { $null -ne $_.PrivateWorkingSet } | ForEach-Object PrivateWorkingSet)
        [pscustomobject][ordered]@{
            Run = $run.Name; RunCompleted = $completion.Completed; Edition = $completion.Renderer; SHA256 = $report.Executable.SHA256; Phase = $name; Samples = $report.Samples.Count
            Mode = $report.Executable.Mode; PrivateBytesMedian = Median $private; PrivateBytesMin = ($private | Measure-Object -Minimum).Minimum; PrivateBytesMax = ($private | Measure-Object -Maximum).Maximum
            PrivateBytesPollingMin = ($report.Samples.PrivateBytesMin | Measure-Object -Minimum).Minimum; PrivateBytesPollingMax = ($report.Samples.PrivateBytesMax | Measure-Object -Maximum).Maximum
            PrivateMiB = (Median $private) / 1MB; WorkingSetMiB = (Median $working) / 1MB
            PrivateWorkingSetMiB = $(if ($privateWorking.Count -eq $report.Samples.Count) { (Median $privateWorking) / 1MB } else { $null })
            CPUPercentOfMachineMedian = Median @($report.Samples.CPUPercentOfMachine)
            Processes = @($report.Samples.ProcessCount | Sort-Object -Unique); Threads = @($report.Samples.Threads | Sort-Object -Unique); Handles = @($report.Samples.Handles | Sort-Object -Unique)
            CatalogCount = $(if ($report.ContextAfter.Business) { $report.ContextAfter.Business.Count } else { $report.ContextAfter.VisibleIndexCount })
            CachedIndexCount = $report.ContextAfter.CachedIndexCount; IconDisk = $report.ContextAfter.IconDisk
            IconCache = $(if ($report.ContextAfter.Business) { $report.ContextAfter.Business.IconCache })
            ScanWarningsAccepted = $report.AllowScanWarnings
            OperatorBusinessConfirmed = $(if ($report.PSObject.Properties['OperatorBusinessConfirmed']) { $report.OperatorBusinessConfirmed } else { $false })
            ScanWarning = $(if ($report.ContextAfter.Business) { $report.ContextAfter.Business.Warning } else { $null })
            InputsBefore = $report.Samples[0].InputBefore; InputsAfter = $report.Samples[-1].InputAfter
            GPU = @($report.Samples.GPU)
            EvidenceSHA256 = (Get-FileHash -LiteralPath $path).Hash
            CollectorSHA256 = $report.Environment.CollectorSHA256; ValidatorSHA256 = $report.Environment.ValidatorSHA256
        }
    }
})
if (-not $rows.Count) { throw 'No completed eligible business matrix exists yet.' }
[ordered]@{
    SchemaVersion = 1; CreatedAtUtc = [DateTime]::UtcNow.ToString('o'); Rows = $rows
    Scope = 'Observed business-mode idle resource samples only. Synthetic workload cycles do not establish physical input latency, strict cold-start performance, DPI transition costs or renderer selection.'
    RS12Complete = $false
} | ConvertTo-Json -Depth 18 | Set-Content -LiteralPath $OutputPath -Encoding utf8NoBOM
$rows | Select-Object Edition, Phase, Samples, PrivateMiB, PrivateWorkingSetMiB, CPUPercentOfMachineMedian | Format-Table -AutoSize
