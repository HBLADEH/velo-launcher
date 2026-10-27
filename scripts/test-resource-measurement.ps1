$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
. (Join-Path $PSScriptRoot 'lib/resource-sample.ps1')

function Assert($Condition, [string]$Message) { if (-not $Condition) { throw $Message } }
function New-Snapshot([double]$At, [double]$CPU, [string]$State = 'Hidden', [string]$Identity = '42/100', [bool]$Complete = $true) {
    [pscustomobject]@{
        TimestampUtc = ([DateTimeOffset]'2026-10-27T00:00:00Z').AddSeconds($At).ToString('o')
        MonotonicSeconds = $At
        Processes = @([pscustomobject]@{ Identity = $Identity; ProcessId = 42; CPUSeconds = $CPU })
        Complete = $Complete
        PrivateBytes = 20MB
        WorkingSet = 30MB
        Window = [pscustomobject]@{ State = $State; DPI = 120 }
    }
}
function Summarize([object[]]$Points, [bool]$Idle = $true, [bool]$Stable = $true) {
    Get-VeloSampleSummary -Snapshots $Points -ExpectedWindowState Hidden -IdleConfirmed $Idle -DataStable $Stable
}

$stable = Summarize @((New-Snapshot 0 10), (New-Snapshot 15 13.6))
Assert ($stable.CPUSampleValid -and $stable.BaselineEligible) 'A stable, confirmed sample must be usable.'
Assert ([math]::Abs($stable.CPUPercentOfMachine - (24 / [Environment]::ProcessorCount)) -lt 0.000001) 'CPU must use elapsed time and logical processor normalization.'
Assert ($stable.PrivateBytes -eq 20MB -and $stable.WorkingSet -eq 30MB -and $stable.PollCount -eq 2) 'Preserve both memory metrics and poll count.'

$churn = Summarize @((New-Snapshot 0 10), (New-Snapshot 5 11 Hidden '42/200'), (New-Snapshot 15 14))
Assert (-not $churn.CPUSampleValid -and $null -eq $churn.CPUPercentOfMachine) 'Reject intermediate PID reuse even if endpoints match.'
$incomplete = Summarize @((New-Snapshot 0 10), (New-Snapshot 15 14 Hidden '42/100' $false))
Assert (-not $incomplete.CPUSampleValid) 'An unreadable process invalidates CPU.'
$decrease = Summarize @((New-Snapshot 0 10), (New-Snapshot 5 9), (New-Snapshot 15 14))
Assert (-not $decrease.CPUSampleValid) 'Reject a counter decrease between polls.'
$unknown = Summarize @((New-Snapshot 0 10 Unknown), (New-Snapshot 15 14 Unknown))
Assert (-not $unknown.WindowStateConfirmed -and -not $unknown.BaselineEligible) 'Unknown visibility must never pass as hidden.'
$changed = Summarize @((New-Snapshot 0 10), (New-Snapshot 5 11 Visible), (New-Snapshot 15 14))
Assert (-not $changed.WindowStateConfirmed -and -not $changed.BaselineEligible) 'Reject a show/hide cycle within a sample.'
$mismatch = Summarize @((New-Snapshot 0 10 Visible), (New-Snapshot 15 14 Visible))
Assert ($mismatch.WindowStateConfirmed -and -not $mismatch.WindowStateMatches -and -not $mismatch.BaselineEligible) 'A confirmed visible state does not satisfy hidden.'
$unconfirmed = Summarize @((New-Snapshot 0 10), (New-Snapshot 15 14)) $false
Assert ($unconfirmed.CPUSampleValid -and -not $unconfirmed.BaselineEligible) 'Valid CPU alone does not prove scanning completed.'
$dataChange = Summarize @((New-Snapshot 0 10), (New-Snapshot 15 14)) $true $false
Assert (-not $dataChange.BaselineEligible) 'Changed input/refresh evidence invalidates an idle baseline.'
$badClock = $false
try { $null = Summarize @((New-Snapshot 1 10), (New-Snapshot 1 14)) } catch { $badClock = $true }
Assert $badClock 'Reject a non-advancing monotonic clock.'

# Deliberately unordered ancestry; Code and its WebView are launched applications.
$tree = @(
    [pscustomobject]@{ ProcessId = 3; ParentProcessId = 2; Name = 'msedgewebview2.exe' },
    [pscustomobject]@{ ProcessId = 5; ParentProcessId = 4; Name = 'msedgewebview2.exe' },
    [pscustomobject]@{ ProcessId = 4; ParentProcessId = 1; Name = 'Code.exe' },
    [pscustomobject]@{ ProcessId = 2; ParentProcessId = 1; Name = 'msedgewebview2.exe' },
    [pscustomobject]@{ ProcessId = 1; ParentProcessId = 9; Name = 'velo-launcher.exe' },
    [pscustomobject]@{ ProcessId = 6; ParentProcessId = 9; Name = 'msedgewebview2.exe' }
)
$selected = @(Get-VeloProcessTree -Processes $tree -RootProcessId 1)
Assert (-not (Compare-Object @(1, 2, 3) @($selected.ProcessId | Sort-Object))) 'Include all launcher WebViews and exclude launched/unrelated applications.'

$jsonPath = Join-Path ([IO.Path]::GetTempPath()) ('velo-metadata-test-' + [guid]::NewGuid().ToString('N') + '.json')
try {
    # Construct Chinese characters without relying on this script's source BOM.
    $chineseName = ([string][char]0x8ba1) + [char]0x7b97 + [char]0x5668
    [IO.File]::WriteAllText($jsonPath, ('{"apps":[{"name":"' + $chineseName + '"}]}'), [Text.UTF8Encoding]::new($false))
    $metadata = Read-VeloMetadataJson $jsonPath
    Assert ($metadata.apps[0].name -eq $chineseName) 'Read UTF-8 Chinese index metadata without a BOM on PowerShell 5.1 and 7.'
    [IO.File]::WriteAllText($jsonPath, '{"private-content":"never-dump-this-index', [Text.UTF8Encoding]::new($false))
    $boundedError = $false
    try { $null = Read-VeloMetadataJson $jsonPath } catch {
        $boundedError = $_.Exception.Message -like 'Cannot read JSON metadata*' -and $_.Exception.Message -notlike '*never-dump-this-index*'
    }
    Assert $boundedError 'Malformed metadata must fail without dumping index contents.'
} finally {
    if (Test-Path -LiteralPath $jsonPath) { Remove-Item -LiteralPath $jsonPath }
}

foreach ($script in @('measure-resources.ps1', 'lib/resource-sample.ps1', 'test-resource-measurement.ps1')) {
    $tokens = $null; $parseErrors = $null
    [void][System.Management.Automation.Language.Parser]::ParseFile((Join-Path $PSScriptRoot $script), [ref]$tokens, [ref]$parseErrors)
    Assert (-not $parseErrors) "PowerShell syntax errors in $script"
}
Write-Output 'Resource measurement regression checks passed (CPU, process ancestry, visibility, idle evidence, memory, UTF-8 metadata, bounded errors and syntax).'
