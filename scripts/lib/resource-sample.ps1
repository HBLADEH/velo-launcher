# Helpers shared by the collector and its dependency-free regression checks.
function Get-VeloColdEnvironmentId {
    param(
        [Parameter(Mandatory)][string]$WindowsVersion,
        [Parameter(Mandatory)][string]$WindowsBuild,
        [Parameter(Mandatory)][string[]]$Processors
    )
    # Stable field and processor order keep the same environment comparable across PowerShell processes.
    [string[]]$orderedProcessors = $Processors.Clone()
    [Array]::Sort($orderedProcessors, [StringComparer]::Ordinal)
    $environment = [ordered]@{ Windows = $WindowsVersion; Build = $WindowsBuild; CPU = @($orderedProcessors) }
    [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes(($environment | ConvertTo-Json -Compress))))
}

function Get-VeloExecutableMode {
    param([string]$CommandLine)
    if ($CommandLine -match '--measurement-log=') { return 'MeasurementProbe' }
    if ($CommandLine -match '(?:^|\s)--diagnostics(?:\s|$)') { return 'Diagnostics' }
    'Production'
}

function Read-VeloEventLines {
    param([string]$Path)
    # The bounded Rust writer keeps its file open. Both sides must share write
    # access on Windows; File.ReadLines denies an already-open writer.
    $stream = [IO.FileStream]::new($Path, [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::ReadWrite -bor [IO.FileShare]::Delete)
    $reader = [IO.StreamReader]::new($stream, [Text.Encoding]::UTF8)
    try { $content = $reader.ReadToEnd() } finally { $reader.Dispose() }
    # Ignore a trailing partial write until its terminating newline is visible.
    $lines = $content -split '\r?\n'
    if ($lines.Count -gt 1) { $lines[0..($lines.Count - 2)] }
}

function Read-VeloBusinessObservation {
    param([string]$Path, [int]$ProcessId)
    $state = [ordered]@{ ProcessId = $ProcessId; Complete = $false; Scanning = $null; Warning = $null; Count = $null; IconCache = $null; LastScan = $null; PublishedDuringScan = $false; Dropped = $false }
    $scanStarted = $null
    $published = $null
    foreach ($line in Read-VeloEventLines $Path) {
        $fields = @{}
        foreach ($match in [regex]::Matches($line, '(\w+)=([^ ]+)')) { $fields[$match.Groups[1].Value] = $match.Groups[2].Value }
        if ($fields.pid -ne [string]$ProcessId -or $fields.dropped -ne '0') { $state.Dropped = $true; continue }
        switch ($fields.event) {
            'catalog_published' { $state.Count = [int]$fields.count; $published = [long]$fields.elapsed_us }
            'index_status' {
                $state.Scanning = $fields.scanning -eq 'true'
                if ($state.Scanning) { $scanStarted = [long]$fields.elapsed_us }
                else { $state.PublishedDuringScan = $null -ne $scanStarted -and $null -ne $published -and $published -gt $scanStarted }
                $state.Warning = $fields.warning -eq 'true'
                $state.LastScan = $fields.elapsed_us
            }
            'icon_cache' { $state.IconCache = [pscustomobject]@{ Entries = [int]$fields.entries; CPUAndTextureEstimateBytes = [long]$fields.cpu_and_texture_estimate_bytes } }
        }
    }
    $state.Complete = $null -ne $state.Count -and $null -ne $state.Scanning -and -not $state.Dropped
    [pscustomobject]$state
}

function Get-VeloIconDiskEvidence {
    param([string]$Directory)
    $path = Join-Path $Directory 'cache/icons'
    $files = @(if (Test-Path -LiteralPath $path) { Get-ChildItem -LiteralPath $path -File -Filter '*.png' | Sort-Object Name })
    [long]$bytes = 0
    foreach ($file in $files) { $bytes += $file.Length }
    $fingerprint = ($files | ForEach-Object { $_.Name + ':' + (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash }) -join "`n"
    [pscustomobject]@{
        Files = $files.Count
        Bytes = $bytes
        SHA256 = ([BitConverter]::ToString([Security.Cryptography.SHA256]::Create().ComputeHash([Text.Encoding]::UTF8.GetBytes($fingerprint)))).Replace('-', '')
    }
}

function Read-VeloMetadataJson {
    param([string]$Path)
    try {
        # Read JSON explicitly as UTF-8 and keep parser errors from dumping the
        # user's index into the normal diagnostic output.
        [IO.File]::ReadAllText($Path, [Text.Encoding]::UTF8) | ConvertFrom-Json -ErrorAction Stop
    } catch {
        throw "Cannot read JSON metadata at '$Path'; check UTF-8 encoding and JSON syntax."
    }
}

function Read-VeloRustBuildIdentity {
    param([string]$Executable, [string]$IdentityPath)
    if (-not $IdentityPath) { $IdentityPath = Join-Path (Split-Path -Parent $Executable) 'build-identity.json' }
    $identity = Read-VeloMetadataJson $IdentityPath
    $file = Get-Item -LiteralPath $Executable
    if ($identity.ExecutableSHA256 -ne (Get-FileHash -LiteralPath $Executable -Algorithm SHA256).Hash -or
        $identity.ExecutableBytes -ne $file.Length -or $identity.Backend -ne 'winit' -or $identity.Renderer -notin @('femtovg','software') -or
        @($identity.Sources | Where-Object { $_.Path -eq 'src/fixtures.rs' -and $_.SHA256 -match '^[0-9a-fA-F]{64}$' }).Count -ne 1) {
        throw 'Rust build identity does not match the sampled executable/fixture.'
    }
    foreach ($features in @($identity.SlintFeatures, $identity.WinitBackendFeatures)) {
        $renderers = @($features | Where-Object { $_ -like 'renderer-*' })
        if ($renderers.Count -ne 1 -or $renderers[0] -ne ('renderer-' + $identity.Renderer)) { throw 'Rust identity must contain exactly one matching renderer feature.' }
    }
    $identity
}

function Get-VeloProcessTree {
    param([object[]]$Processes, [int]$RootProcessId)
    $ids = [System.Collections.Generic.HashSet[int]]::new()
    [void]$ids.Add($RootProcessId)
    do {
        $added = $false
        foreach ($entry in $Processes) {
            if ($ids.Contains([int]$entry.ParentProcessId) -and $ids.Add([int]$entry.ProcessId)) { $added = $true }
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
        ProcessTreePolicy = 'All descendants'
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
