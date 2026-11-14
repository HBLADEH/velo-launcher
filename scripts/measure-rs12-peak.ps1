#Requires -Version 7.4
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$SessionDirectory,
    [ValidateRange(1,120)][int]$Seconds = 30,
    [ValidateRange(10,1000)][int]$IntervalMilliseconds = 100,
    [Parameter(Mandatory)][string]$OutputPath,
    # The session launcher starts this task immediately after Process.Start.
    [System.Threading.Tasks.Task]$ObservationTask
)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'lib/resource-sample.ps1')
if (Test-Path -LiteralPath $OutputPath) { throw 'Use a new OutputPath; previous peak observations must be preserved.' }
$session = Read-VeloMetadataJson (Join-Path $SessionDirectory 'session.json')
if (-not ('VeloRS12Resources.Observer' -as [type])) { Add-Type -Path (Join-Path $PSScriptRoot 'lib/rs12-resource-observer.cs') }
if (-not $ObservationTask) {
    $live = Get-Process -Id $session.ProcessId
    if ($live.StartTime.ToUniversalTime().Ticks -ne $session.StartedTicks -or $live.Path -ne $session.Executable) { throw 'The session process identity changed.' }
    $ObservationTask = [VeloRS12Resources.Observer]::Start($session.ProcessId, $session.StartedTicks, $Seconds, $IntervalMilliseconds)
}
Write-Output "RS12_RESOURCE_OBSERVER_READY pid=$($session.ProcessId)"
$capture = $ObservationTask.GetAwaiter().GetResult()
if ($capture.ProcessId -ne $session.ProcessId -or $capture.StartedTicks -ne $session.StartedTicks) { throw 'The resource capture belongs to a different process.' }
$complete = @($capture.Snapshots | Where-Object Complete)
$intervals = @(for ($i=1; $i -lt $capture.Snapshots.Count; $i++) { $capture.Snapshots[$i].ElapsedMilliseconds - $capture.Snapshots[$i-1].ElapsedMilliseconds })
$medianInterval = if ($intervals.Count) {
    $ordered = @($intervals | Sort-Object)
    $middle = [int][math]::Floor($ordered.Count/2)
    if ($ordered.Count % 2) { $ordered[$middle] } else { ($ordered[$middle-1]+$ordered[$middle])/2 }
}
$scan = $null
if ($session.MeasurementLog -and (Test-Path -LiteralPath $session.MeasurementLog)) {
    $scan = Read-VeloBusinessObservation $session.MeasurementLog $session.ProcessId
    $scanEvents = @(foreach ($line in Read-VeloEventLines $session.MeasurementLog) {
        $fields = @{}
        foreach ($match in [regex]::Matches($line, '(\w+)=([^ ]+)')) { $fields[$match.Groups[1].Value] = $match.Groups[2].Value }
        if ($fields.pid -eq [string]$session.ProcessId -and $fields.dropped -eq '0' -and $fields.event -eq 'index_status' -and $fields.elapsed_us -match '^\d+$') {
            [pscustomobject]@{ UnixMicroseconds=[long]$fields.unix_us; ElapsedMicroseconds=[long]$fields.elapsed_us; Scanning=$fields.scanning -eq 'true'; Warning=$fields.warning -eq 'true' }
        }
    })
    $scanEnd = $scanEvents | Where-Object { -not $_.Scanning } | Select-Object -Last 1
    $scanStart = if ($scanEnd) { $scanEvents | Where-Object { $_.Scanning -and $_.ElapsedMicroseconds -le $scanEnd.ElapsedMicroseconds } | Select-Object -Last 1 }
    if ($scanStart -and $scanEnd) {
        $scan | Add-Member -NotePropertyName StartElapsedMicroseconds -NotePropertyValue $scanStart.ElapsedMicroseconds -Force
        $scan | Add-Member -NotePropertyName EndElapsedMicroseconds -NotePropertyValue $scanEnd.ElapsedMicroseconds -Force
        $scan | Add-Member -NotePropertyName DurationMilliseconds -NotePropertyValue ([math]::Round(($scanEnd.ElapsedMicroseconds-$scanStart.ElapsedMicroseconds)/1000,3)) -Force
        $scan | Add-Member -NotePropertyName Warning -NotePropertyValue ($scanStart.Warning -or $scanEnd.Warning) -Force
        $scan | Add-Member -NotePropertyName StartUnixMicroseconds -NotePropertyValue $scanStart.UnixMicroseconds -Force
        $scan | Add-Member -NotePropertyName EndUnixMicroseconds -NotePropertyValue $scanEnd.UnixMicroseconds -Force
    }
} elseif ($session.Edition -eq 'wails') {
    $log = Join-Path $session.DataDirectory 'logs/velo.log'
    $refresh = @(if (Test-Path -LiteralPath $log) { foreach ($line in Read-VeloEventLines $log) {
        try { $entry = $line | ConvertFrom-Json -ErrorAction Stop } catch { continue }
        if ($entry.msg -eq 'index refreshed' -and ([datetime]$entry.time).ToUniversalTime().Ticks -ge $session.StartedTicks) { $entry }
    } }) | Select-Object -Last 1
    if ($refresh) {
        $completedAt = ([datetime]$refresh.time).ToUniversalTime()
        $startAt = $completedAt.AddMilliseconds(-[double]$refresh.duration_ms)
        $scan = [pscustomobject]@{ Complete=$true; Count=$refresh.apps; Warning=[bool]$refresh.error; StartedAtUtc=$startAt.ToString('o'); CompletedAtUtc=$completedAt.ToString('o'); DurationMilliseconds=$refresh.duration_ms; Source='Successful current-session refresh log; scan start reconstructed from duration_ms.' }
    }
}
$binaryMatches = (Get-FileHash -LiteralPath $session.Executable).Hash -eq $session.SHA256
[ordered]@{
    SchemaVersion=1; Edition=$session.Edition; SHA256=$session.SHA256; ProcessId=$session.ProcessId; StartedTicks=$session.StartedTicks
    CapturedAtUtc=[datetime]::UtcNow.ToString('o'); CollectorSHA256=(Get-FileHash -LiteralPath $PSCommandPath).Hash
    ObserverSHA256=(Get-FileHash -LiteralPath (Join-Path $PSScriptRoot 'lib/rs12-resource-observer.cs')).Hash
    CaptureStartedAfterProcessMilliseconds=($capture.CaptureStartedUtcTicks-$session.StartedTicks)/10000.0
    ExecutableMatches=$binaryMatches; ScanAfter=$scan
    Summary=[ordered]@{
        Samples=$capture.Snapshots.Count; CompleteSamples=$complete.Count; IncompleteSamples=$capture.Snapshots.Count-$complete.Count
        RootExited=$capture.RootExited; ElapsedMilliseconds=$capture.ElapsedMilliseconds
        RequestedIntervalMilliseconds=$capture.IntervalMilliseconds
        ObservedIntervalMedianMilliseconds=$medianInterval
        ObservedIntervalMaximumMilliseconds=$(if ($intervals.Count) { ($intervals | Measure-Object -Maximum).Maximum })
        CollectionMaximumMilliseconds=($capture.Snapshots.CollectionMilliseconds | Measure-Object -Maximum).Maximum
        ObserverCPUSeconds=$capture.ObserverCPUSeconds
        ObserverCPUPercentOfMachine=$(if ($capture.ElapsedMilliseconds -gt 0) { [math]::Round(($capture.ObserverCPUSeconds/($capture.ElapsedMilliseconds/1000)/[Environment]::ProcessorCount)*100,3) })
        PrivateBytesSampledMaximum=$(if ($complete.Count) { ($complete.PrivateBytes | Measure-Object -Maximum).Maximum })
        WorkingSetSampledMaximum=$(if ($complete.Count) { ($complete.WorkingSet | Measure-Object -Maximum).Maximum })
        ProcessCountMaximum=$(if ($complete.Count) { ($complete | ForEach-Object { $_.Processes.Count } | Measure-Object -Maximum).Maximum })
        HandlesSampledMaximum=$(if ($complete.Count) { ($complete.Handles | Measure-Object -Maximum).Maximum })
        ThreadsSampledMaximum=$(if ($complete.Count) { ($complete.Threads | Measure-Object -Maximum).Maximum })
    }
    Capture=$capture
    UnavailableMetrics=@('Private working set and GPU memory are not read by this high-frequency sampler; use the separate steady-state collector.','Slint internal GPU texture/glyph cache allocation is not exposed.')
    Limitations=@('Sampled maxima are lower bounds on instantaneous peaks. Processes born and exited between enumerations may be missed.','Each tree snapshot reads processes sequentially; Complete means every discovered descendant was readable, not an atomic kernel snapshot.','Creation identity excludes reused PIDs. Previously observed descendants remain tracked after their parent exits; an unobserved orphan cannot be assigned safely.','Startup begins before the first snapshot; the initial unsampled gap and observed polling intervals are reported. This is a warm-cache observation, never a strict cold-start sample.','PrivateBytes is private commit. Summed working sets double-count shared pages. Observer CPU belongs to the PowerShell collector, separate from the application.','The requested polling interval trades observer cost against missed short peaks; review actual intervals and ObserverCPUPercentOfMachine.','Scan timing comes from app events or logs and is reported separately; incomplete scan evidence is not inferred as a measured scan peak.')
} | ConvertTo-Json -Depth 12 | Set-Content -LiteralPath $OutputPath -Encoding utf8NoBOM
if (-not $binaryMatches -or -not $complete.Count) { throw 'Invalid high-frequency resource observation retained.' }
Write-Output "Resource observation saved: $($capture.Snapshots.Count) samples, $($complete.Count) complete."
