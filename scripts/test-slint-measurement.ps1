#Requires -Version 7.4
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
. (Join-Path $PSScriptRoot 'lib/resource-sample.ps1')
function Assert($Condition, [string]$Message) { if (-not $Condition) { throw $Message } }
$testRoot = Join-Path ([IO.Path]::GetTempPath()) ('velo-slint-measurement-' + [guid]::NewGuid().ToString('N'))
try {
    # CIM does not guarantee parent-before-child order. Include every descendant
    # while keeping unrelated processes out of the application's resource sum.
    $processes = @(
        [pscustomobject]@{ ProcessId = 13; ParentProcessId = 12; Name = 'helper.exe' },
        [pscustomobject]@{ ProcessId = 99; ParentProcessId = 0; Name = 'unrelated.exe' },
        [pscustomobject]@{ ProcessId = 11; ParentProcessId = 0; Name = 'velo-slint.exe' },
        [pscustomobject]@{ ProcessId = 12; ParentProcessId = 11; Name = 'helper.exe' }
    )
    $tree = @(Get-VeloProcessTree -Processes $processes -RootProcessId 11)
    Assert (($tree.ProcessId | Sort-Object) -join ',' -eq '11,12,13') 'Include nested descendants regardless of enumeration order, excluding unrelated processes.'
    $snapshots = @(0, 1 | ForEach-Object {
        [pscustomobject]@{
            TimestampUtc = [DateTime]::UnixEpoch.AddSeconds($_).ToString('o'); MonotonicSeconds = [double]$_
            Complete = $true; Window = [pscustomobject]@{ State = 'Visible' }
            Processes = @([pscustomobject]@{ Identity = '11/1'; CPUSeconds = [double]$_ })
            PrivateBytes = 1048576; WorkingSet = 2097152
        }
    })
    Assert (Get-VeloSampleSummary $snapshots Visible $true $true).BaselineEligible 'Stable process/window/data snapshots are eligible.'
    $snapshots[1].Processes += [pscustomobject]@{ Identity = '12/1'; CPUSeconds = 0.0 }
    Assert (-not (Get-VeloSampleSummary $snapshots Visible $true $true).BaselineEligible) 'A child appearing during sampling invalidates CPU comparison and baseline eligibility.'
    New-Item -ItemType Directory -Path $testRoot | Out-Null
    $exe = Join-Path $testRoot 'velo-slint.exe'
    'synthetic executable bytes' | Set-Content -LiteralPath $exe
    $identityPath = Join-Path $testRoot 'build-identity.json'
    $identity = [pscustomobject]@{
        ExecutableSHA256 = (Get-FileHash -LiteralPath $exe).Hash; ExecutableBytes = (Get-Item -LiteralPath $exe).Length
        Renderer = 'software'; Backend = 'winit'; SlintFeatures = @('renderer-software'); WinitBackendFeatures = @('renderer-software')
        Sources = @([pscustomobject]@{ Path = 'src/fixtures.rs'; SHA256 = ('a' * 64) })
    }
    $identity | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $identityPath
    Assert ((Read-VeloRustBuildIdentity $exe).Renderer -eq 'software') 'Accept a matching single-renderer build.'
    $identity.SlintFeatures = @('renderer-software','renderer-femtovg')
    $identity | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $identityPath
    $rejected = $false
    try { $null = Read-VeloRustBuildIdentity $exe } catch { $rejected = $true }
    Assert $rejected 'Reject a build that could silently fall back to another renderer.'
    $identity.SlintFeatures = @('renderer-software')
    $identity.ExecutableBytes++
    $identity | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $identityPath
    $rejected = $false
    try { $null = Read-VeloRustBuildIdentity $exe } catch { $rejected = $true }
    Assert $rejected 'Reject mismatching executable identity.'

    # Export is exercised without launching or interacting with a real process.
    $collector = Join-Path $PSScriptRoot 'collect-slint-latency.ps1'
    $start = [DateTime]::UnixEpoch.AddSeconds(100)
    $log = Join-Path $testRoot 'fixture-events.log'
    [ordered]@{ Executable = $exe; ExecutableSHA256 = (Get-FileHash -LiteralPath $exe).Hash; EnvironmentId = 'fixture-environment'; InputId = 'fixture-input'; Method = 'ComputerUseAssistedUpperBound' } |
        ConvertTo-Json | Set-Content -LiteralPath (Join-Path $testRoot 'slint-manifest.json')
    [ordered]@{ Name = 'fixture'; ProcessId = 2147483647; StartedTicks = $start.Ticks; LogPath = $log } |
        ConvertTo-Json | Set-Content -LiteralPath (Join-Path $testRoot 'slint-active.json')
    $native = 'event=native_window pid=2147483647 unix_us=100010000 elapsed_us=1000 dropped=0 hwnd=ABCD scale=1.25'
    $input = 'event=input_accepted pid=2147483647 unix_us=100020000 elapsed_us=11000 dropped=0 utf8_bytes=1 focused=true native_focused=true visible=true composing=false synthetic=false'
    @($native, $input) | Set-Content -LiteralPath $log
    $null = & $collector -Action Snapshot -WorkDirectory $testRoot -OutputDirectory $testRoot
    $row = Import-Csv -LiteralPath (Join-Path $testRoot 'desktop-latency.csv')
    Assert ($row.Valid -eq 'true' -and [double]$row.DurationMilliseconds -eq 20 -and $row.Boundary -eq 'ProcessStartToEditable') 'Use actual accepted input, process creation time and the correct boundary.'
    Assert ($row.Method -eq 'ComputerUseAssistedUpperBound' -and $row.Reason -like '*not a calibrated startup*') 'Preserve the tool waiting limitation.'
    @($native, $input.Replace(' synthetic=false', '')) | Set-Content -LiteralPath $log
    $null = & $collector -Action Snapshot -WorkDirectory $testRoot -OutputDirectory $testRoot
    $row = Import-Csv -LiteralPath (Join-Path $testRoot 'desktop-latency.csv')
    Assert ($row.Valid -eq 'true') 'Legacy real-input records without the newer synthetic field remain readable.'
    foreach ($replacement in @('native_focused=false', 'composing=true', 'dropped=1', 'pid=123', 'synthetic=true')) {
        $key = $replacement.Split('=')[0]
        $bad = [regex]::Replace($input, ('\b' + $key + '=[^ ]+'), $replacement)
        @($native, $bad) | Set-Content -LiteralPath $log
        $null = & $collector -Action Snapshot -WorkDirectory $testRoot -OutputDirectory $testRoot
        $row = Import-Csv -LiteralPath (Join-Path $testRoot 'desktop-latency.csv')
        Assert ($row.Valid -eq 'false') "Reject invalid input evidence: $replacement"
    }
    $native | Set-Content -LiteralPath $log
    $null = & $collector -Action Snapshot -WorkDirectory $testRoot -OutputDirectory $testRoot
    $row = Import-Csv -LiteralPath (Join-Path $testRoot 'desktop-latency.csv')
    Assert ($row.Valid -eq 'false' -and -not $row.DurationMilliseconds) 'A window/ready marker alone must not be accepted as editable input.'
    $input | Set-Content -LiteralPath $log
    $null = & $collector -Action Snapshot -WorkDirectory $testRoot -OutputDirectory $testRoot
    $row = Import-Csv -LiteralPath (Join-Path $testRoot 'desktop-latency.csv')
    Assert ($row.Valid -eq 'false' -and -not $row.DurationMilliseconds) 'Retain a failed observation when no native window was ever observed, including under StrictMode.'
    $businessLog = Join-Path $testRoot 'business-events.log'
    $business = @(
        'event=catalog_published pid=42 elapsed_us=1 dropped=0 count=1000',
        'event=index_status pid=42 elapsed_us=2 dropped=0 scanning=true warning=false',
        'event=icon_cache pid=42 elapsed_us=3 dropped=0 entries=12 cpu_and_texture_estimate_bytes=98304'
    )
    $business | Set-Content -LiteralPath $businessLog
    $state = Read-VeloBusinessObservation $businessLog 42
    Assert ($state.Complete -and $state.Scanning -and $state.Count -eq 1000 -and $state.IconCache.Entries -eq 12) 'Read real business count/cache without falling back to 28 fixtures; retain a busy scan.'
    $business += 'event=index_status pid=42 elapsed_us=4 dropped=0 scanning=false warning=false'
    $business | Set-Content -LiteralPath $businessLog
    $state = Read-VeloBusinessObservation $businessLog 42
    Assert ($state.Complete -and -not $state.Scanning -and $state.LastScan -eq '4') 'Require the latest completed scan.'
    Assert (-not (Read-VeloBusinessObservation $businessLog 43).Complete) 'Reject evidence from another PID.'
    Add-Content -LiteralPath $businessLog 'event=index_status pid=42 elapsed_us=5 dropped=1 scanning=false warning=false'
    Assert (-not (Read-VeloBusinessObservation $businessLog 42).Complete) 'Lost business events invalidate idle evidence.'
    $openWriter = [IO.FileStream]::new($businessLog, [IO.FileMode]::Append, [IO.FileAccess]::Write, [IO.FileShare]::ReadWrite)
    try { Assert ((Read-VeloBusinessObservation $businessLog 42).Count -eq 1000) 'Read a live business log while the Windows writer still owns it.' } finally { $openWriter.Dispose() }
    @('event=index_status pid=42 elapsed_us=1 dropped=0 scanning=true warning=true',
      'event=catalog_published pid=42 elapsed_us=2 dropped=0 count=619',
      'event=index_status pid=42 elapsed_us=3 dropped=0 scanning=false warning=true') | Set-Content -LiteralPath $businessLog
    Assert ((Read-VeloBusinessObservation $businessLog 42).PublishedDuringScan) 'Partial discovery is identifiable by a snapshot published during the completed scan.'
    @('event=catalog_published pid=42 elapsed_us=1 dropped=0 count=619',
      'event=index_status pid=42 elapsed_us=2 dropped=0 scanning=true warning=true',
      'event=index_status pid=42 elapsed_us=3 dropped=0 scanning=false warning=true') | Set-Content -LiteralPath $businessLog
    Assert (-not (Read-VeloBusinessObservation $businessLog 42).PublishedDuringScan) 'A failed scan retaining only the earlier snapshot cannot pass the partial-scan exception.'
    $iconDir = Join-Path $testRoot 'cache/icons'
    New-Item -ItemType Directory -Path $iconDir | Out-Null
    $empty = Get-VeloIconDiskEvidence $testRoot
    [IO.File]::WriteAllBytes((Join-Path $iconDir 'fixture.png'), [byte[]]@(1, 2, 3))
    $icons = Get-VeloIconDiskEvidence $testRoot
    Assert ($empty.Files -eq 0 -and $icons.Files -eq 1 -and $icons.Bytes -eq 3 -and $empty.SHA256 -ne $icons.SHA256) 'Fingerprint disk icon content independently of decoded/GPU estimates.'
    Write-Output 'Slint measurement checks passed (full process tree, unstable samples, build identity, feature isolation, actual input boundary, focus/composition, loss/PID rejection and missing-input failure).'
} finally {
    if (Test-Path -LiteralPath $testRoot) {
        $resolved = (Resolve-Path -LiteralPath $testRoot).Path
        $temp = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\') + '\'
        if (-not $resolved.StartsWith($temp, [StringComparison]::OrdinalIgnoreCase) -or (Split-Path -Leaf $resolved) -notlike 'velo-slint-measurement-*') { throw 'Unexpected cleanup target.' }
        Remove-Item -LiteralPath $resolved -Recurse -Force
    }
}
