[CmdletBinding()]
param(
    [Parameter(Mandatory)][ValidateRange(1, 2147483647)][int]$ProcessId,
    [ValidateRange(1, 60)][int]$Seconds = 15,
    [ValidateRange(1, 20)][int]$Samples = 1,
    [ValidateSet('Installed', 'Candidate', 'Unknown')][string]$Edition = 'Unknown',
    [string]$ExpectedExecutable,
    [string]$ExecutableCommit,
    [string]$DataDirectory = (Join-Path $env:LOCALAPPDATA 'Velo'),
    [ValidateSet('Visible', 'Hidden', 'Unknown')][string]$WindowState = 'Unknown',
    [ValidateRange(0, 3600)][int]$MinimumStateSeconds = 0,
    [switch]$IdleConfirmed,
    [string]$IdleEvidence,
    [string]$OutputPath
)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'lib/resource-sample.ps1')
if ($MinimumStateSeconds -gt 0 -and $WindowState -eq 'Unknown') { throw 'A settling period requires Visible or Hidden.' }
if ($IdleConfirmed -and [string]::IsNullOrWhiteSpace($IdleEvidence)) { throw 'IdleConfirmed requires IdleEvidence describing how scanning completion was confirmed.' }
if ($OutputPath -and (Test-Path -LiteralPath $OutputPath)) { throw 'OutputPath already exists; keep previous evidence and use a new filename.' }

# Read-only native observation: match both class and owner PID. FindWindow alone
# could accidentally describe another Velo process/session. No window is changed.
if (-not ('VeloMeasurement.WindowProbe' -as [type])) {
    Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;
namespace VeloMeasurement {
    public sealed class WindowInfo {
        public string Handle;
        public bool Visible;
        public uint DPI;
    }
    public static class WindowProbe {
        private delegate bool EnumProc(IntPtr window, IntPtr data);
        [DllImport("user32.dll")] private static extern bool EnumWindows(EnumProc callback, IntPtr data);
        [DllImport("user32.dll")] private static extern uint GetWindowThreadProcessId(IntPtr window, out uint pid);
        [DllImport("user32.dll", CharSet=CharSet.Unicode)] private static extern int GetClassName(IntPtr window, StringBuilder name, int max);
        [DllImport("user32.dll")] private static extern bool IsWindowVisible(IntPtr window);
        [DllImport("user32.dll")] private static extern uint GetDpiForWindow(IntPtr window);
        public static WindowInfo[] Read(uint pid) {
            var result = new List<WindowInfo>();
            EnumWindows((window, data) => {
                uint owner;
                GetWindowThreadProcessId(window, out owner);
                if (owner != pid) return true;
                var name = new StringBuilder(256);
                GetClassName(window, name, name.Capacity);
                if (name.ToString() == "VeloLauncherWindow") {
                    result.Add(new WindowInfo { Handle=window.ToInt64().ToString("X"), Visible=IsWindowVisible(window), DPI=GetDpiForWindow(window) });
                }
                return true;
            }, IntPtr.Zero);
            return result.ToArray();
        }
    }
}
'@
}

function Get-WindowObservation {
    $windows = @([VeloMeasurement.WindowProbe]::Read($ProcessId))
    if ($windows.Count -ne 1) {
        return [pscustomobject]@{ State = 'Unknown'; DPI = $null; ScalePercent = $null; Handle = $null }
    }
    [pscustomobject]@{
        State = $(if ($windows[0].Visible) { 'Visible' } else { 'Hidden' })
        DPI = $windows[0].DPI
        ScalePercent = [math]::Round($windows[0].DPI / 96 * 100, 2)
        Handle = $windows[0].Handle
    }
}

function Get-FileIdentity([string]$Path) {
    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) { return $null }
    $file = Get-Item -LiteralPath $Path
    [pscustomobject]@{ Path = $file.FullName; Bytes = $file.Length; SHA256 = (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash }
}

function Get-DataEvidence {
    $files = [ordered]@{}
    foreach ($name in @('config.json', 'index.json', 'history.json', 'application-library.json', 'quick-launch.json')) {
        $files[$name] = Get-FileIdentity (Join-Path $DataDirectory $name)
    }
    $config = $null
    $cachedCount = $null
    if ($files['config.json']) { $config = Read-VeloMetadataJson $files['config.json'].Path }
    if ($files['index.json']) { $cachedCount = @((Read-VeloMetadataJson $files['index.json'].Path).apps).Count }
    $lastRefresh = $null
    $sessionStarted = $null
    $logPath = Join-Path $DataDirectory 'logs/velo.log'
    if (Test-Path -LiteralPath $logPath -PathType Leaf) {
        foreach ($line in (Get-Content -LiteralPath $logPath -Encoding UTF8 -Tail 2000)) {
            try { $entry = $line | ConvertFrom-Json } catch { continue }
            if ($entry.msg -eq 'application started') { $sessionStarted = $entry.time; $lastRefresh = $null }
            if ($entry.msg -eq 'index refreshed') { $lastRefresh = $entry }
        }
    }
    [pscustomobject]@{
        Directory = $DataDirectory
        Files = $files
        # Raw settings plus a hash distinguish saved fields from in-memory defaults.
        SavedConfig = $config
        CachedIndexCount = $cachedCount
        LogSessionStarted = $sessionStarted
        LastRefresh = $lastRefresh
        VisibleIndexCount = $(if ($lastRefresh) { $lastRefresh.apps } else { $null })
        VisibleIndexCountSource = 'Last completed refresh log; not a live service-state query'
    }
}

function Get-DataSignature($Evidence) {
    [pscustomobject]@{ Files = $Evidence.Files; Session = $Evidence.LogSessionStarted; Refresh = $Evidence.LastRefresh } | ConvertTo-Json -Depth 10 -Compress
}

function Get-InputFingerprint($Evidence) {
    $hashes = [ordered]@{}
    foreach ($entry in $Evidence.Files.GetEnumerator()) {
        $hashes[$entry.Key] = $(if ($entry.Value) { $entry.Value.SHA256 } else { $null })
    }
    [pscustomobject]@{ Files = $hashes; LastRefresh = $Evidence.LastRefresh; CachedIndexCount = $Evidence.CachedIndexCount }
}

function Get-TreeSnapshot {
    $all = @(Get-CimInstance Win32_Process)
    $root = @($all | Where-Object ProcessId -EQ $ProcessId)
    if ($root.Count -ne 1 -or $root[0].Name -notlike 'velo-launcher*.exe') { throw 'ProcessId must identify a running Velo executable.' }
    if (-not $root[0].ExecutablePath -or $root[0].ExecutablePath -ne $target.Path) { throw 'Target executable path changed or cannot be read.' }
    $tree = @(Get-VeloProcessTree -Processes $all -RootProcessId $ProcessId)
    $processes = @(foreach ($id in ($tree.ProcessId | Sort-Object)) {
        try {
            $process = Get-Process -Id $id -ErrorAction Stop
            [pscustomobject]@{
                Identity = "$id/$($process.StartTime.ToUniversalTime().Ticks)"
                ProcessId = $id
                Name = $process.ProcessName
                CPUSeconds = $process.TotalProcessorTime.TotalSeconds
                WorkingSet = $process.WorkingSet64
                PrivateBytes = $process.PrivateMemorySize64
            }
        } catch { continue }
    })
    if (-not ($processes | Where-Object ProcessId -EQ $ProcessId)) { throw 'Velo exited during sampling.' }
    [pscustomobject]@{
        TimestampUtc = [DateTime]::UtcNow.ToString('o')
        MonotonicSeconds = [Diagnostics.Stopwatch]::GetTimestamp() / [double][Diagnostics.Stopwatch]::Frequency
        Processes = $processes
        Complete = $processes.Count -eq $tree.Count
        WorkingSet = ($processes | Measure-Object WorkingSet -Sum).Sum
        PrivateBytes = ($processes | Measure-Object PrivateBytes -Sum).Sum
        Window = Get-WindowObservation
    }
}
function Get-ToolVersion([string]$Name, [string[]]$Arguments) {
    if (-not (Get-Command $Name -ErrorAction SilentlyContinue)) { return $null }
    $lines = @(& $Name @Arguments 2>$null)
    if ($LASTEXITCODE -ne 0) { return $null }
    ($lines | Select-Object -First 1).ToString().Trim()
}

$root = Get-CimInstance Win32_Process -Filter "ProcessId=$ProcessId"
if (-not $root -or $root.Name -notlike 'velo-launcher*.exe' -or -not $root.ExecutablePath) { throw 'ProcessId must identify a readable running Velo executable.' }
$target = Get-FileIdentity $root.ExecutablePath
if ($ExpectedExecutable -and $target.Path -ne (Resolve-Path -LiteralPath $ExpectedExecutable).Path) { throw 'Running executable does not match ExpectedExecutable (possibly an older single instance).' }
$binaryInfo = @()
if (Get-Command go -ErrorAction SilentlyContinue) { $binaryInfo = @(& go version -m $target.Path); if ($LASTEXITCODE -ne 0) { throw 'Cannot read Go executable build information.' } }
$revisionLine = $binaryInfo | Where-Object { $_ -match '^\s*build\s+vcs.revision=' } | Select-Object -First 1
$embeddedRevision = $(if ($revisionLine) { ($revisionLine -split 'vcs.revision=', 2)[1].Trim() } else { $null })
if ($ExecutableCommit -and $embeddedRevision -and $ExecutableCommit -ne $embeddedRevision) { throw 'ExecutableCommit disagrees with embedded vcs.revision; supply a full commit ID.' }
$versionInfo = (Get-Item -LiteralPath $target.Path).VersionInfo
$workspace = Split-Path -Parent $PSScriptRoot
$workspaceCommit = (& git -C $workspace rev-parse HEAD 2>$null | Out-String).Trim()
$workspaceStatus = @(& git -C $workspace status --porcelain 2>$null)
$initialTree = @(Get-VeloProcessTree -Processes @(Get-CimInstance Win32_Process) -RootProcessId $ProcessId)
$webviewVersions = @($initialTree | Where-Object { $_.Name -eq 'msedgewebview2.exe' -and $_.ExecutablePath } | ForEach-Object {
    [pscustomobject]@{ Path = $_.ExecutablePath; Version = (Get-Item -LiteralPath $_.ExecutablePath).VersionInfo.FileVersion }
} | Sort-Object Path -Unique)
$os = Get-CimInstance Win32_OperatingSystem
$environment = [pscustomobject]@{
    Windows = [pscustomobject]@{ Caption = $os.Caption; Version = $os.Version; Build = $os.BuildNumber; LastBootUtc = $os.LastBootUpTime.ToUniversalTime().ToString('o') }
    Processor = @((Get-CimInstance Win32_Processor).Name)
    LogicalProcessors = [Environment]::ProcessorCount
    PowerShell = $PSVersionTable.PSVersion.ToString()
    Go = Get-ToolVersion 'go' @('version')
    WailsCLI = Get-ToolVersion 'wails' @('version')
    Node = Get-ToolVersion 'node' @('--version')
    WebView2 = $webviewVersions
    # Repository HEAD describes the collector workspace, never an installed exe.
    WorkspaceCommit = $workspaceCommit
    WorkspaceDirty = $workspaceStatus.Count -gt 0
    CollectorSHA256 = (Get-FileHash -LiteralPath $PSCommandPath -Algorithm SHA256).Hash
    ValidatorSHA256 = (Get-FileHash -LiteralPath (Join-Path $PSScriptRoot 'lib/resource-sample.ps1') -Algorithm SHA256).Hash
}
$executable = [pscustomobject]@{
    Edition = $Edition; Path = $target.Path; SHA256 = $target.SHA256; Bytes = $target.Bytes
    FileVersion = $versionInfo.FileVersion; ProductVersion = $versionInfo.ProductVersion
    FixedFileVersion = ('{0}.{1}.{2}.{3}' -f $versionInfo.FileMajorPart, $versionInfo.FileMinorPart, $versionInfo.FileBuildPart, $versionInfo.FilePrivatePart)
    ProcessId = $ProcessId; StartedAtUtc = $root.CreationDate.ToUniversalTime().ToString('o')
    SourceCommit = $(if ($embeddedRevision) { $embeddedRevision } elseif ($ExecutableCommit) { $ExecutableCommit } else { $null })
    SourceCommitEvidence = $(if ($embeddedRevision) { 'Embedded vcs.revision' } elseif ($ExecutableCommit) { 'Operator supplied; verify against build manifest' } else { 'Unknown; no vcs.revision in binary' })
    GoCompiler = $(if ($binaryInfo.Count) { ($binaryInfo[0] -split ': ')[-1] } else { $null })
    WailsDependency = ($binaryInfo | Where-Object { $_ -match '^\s*dep\s+github.com/wailsapp/wails/v2\s' } | Out-String).Trim()
    BuildSettings = @($binaryInfo | Where-Object { $_ -match '^\s*build\s+(vcs[^=]*=|-tags=|-ldflags=|GOOS=|GOARCH=)' } | ForEach-Object { $_.Trim() })
    Mode = $(if ($root.CommandLine -match '(?:^|\s)--diagnostics(?:\s|$)') { 'Diagnostics' } else { 'Production' })
    BackgroundFlag = $root.CommandLine -match '(?:^|\s)--background(?:\s|$)'
}

# Observe the entire settling period. A caller-provided hide timestamp or process
# age is not evidence that the window stayed hidden. Polling is read-only.
$settleStartSeconds = [Diagnostics.Stopwatch]::GetTimestamp() / [double][Diagnostics.Stopwatch]::Frequency
$settleWatch = [Diagnostics.Stopwatch]::StartNew()
while ($settleWatch.Elapsed.TotalSeconds -lt $MinimumStateSeconds) {
    $observation = Get-WindowObservation
    if ($observation.State -ne $WindowState) { throw 'Window state changed or is unknown during settling; restart the measurement.' }
    Start-Sleep -Milliseconds 1000
}
$dataBefore = Get-DataEvidence
$stateContinuous = $true
$results = @(for ($sample = 1; $sample -le $Samples; $sample++) {
    $inputBefore = Get-DataEvidence
    $snapshots = [System.Collections.Generic.List[object]]::new()
    $snapshots.Add((Get-TreeSnapshot))
    $watch = [Diagnostics.Stopwatch]::StartNew()
    while ($watch.Elapsed.TotalSeconds -lt $Seconds) {
        $remaining = $Seconds - $watch.Elapsed.TotalSeconds
        Start-Sleep -Milliseconds ([math]::Max(1, [math]::Min(1000, [int]($remaining * 1000))))
        $snapshots.Add((Get-TreeSnapshot))
    }
    $inputAfter = Get-DataEvidence
    $stable = (Get-DataSignature $inputBefore) -eq (Get-DataSignature $inputAfter)
    $result = Get-VeloSampleSummary -Snapshots $snapshots.ToArray() -ExpectedWindowState $WindowState -IdleConfirmed $IdleConfirmed.IsPresent -DataStable $stable
    $stateContinuous = $stateContinuous -and $result.WindowStateMatches
    $result.BaselineEligible = $result.BaselineEligible -and $stateContinuous
    $result | Add-Member -NotePropertyName StateContinuouslyObserved -NotePropertyValue $stateContinuous
    $result | Add-Member -NotePropertyName Number -NotePropertyValue $sample
    $result | Add-Member -NotePropertyName InputBefore -NotePropertyValue (Get-InputFingerprint $inputBefore)
    $result | Add-Member -NotePropertyName InputAfter -NotePropertyValue (Get-InputFingerprint $inputAfter)
    $result | Add-Member -NotePropertyName ObservedStateSecondsAtStart -NotePropertyValue $(if ($stateContinuous) { [math]::Round($snapshots[0].MonotonicSeconds - $settleStartSeconds, 3) } else { $null })
    $result
})
$dataAfter = Get-DataEvidence
$binaryStable = $target.SHA256 -eq (Get-FileHash -LiteralPath $target.Path -Algorithm SHA256).Hash
$report = [pscustomobject]@{
    SchemaVersion = 2
    CapturedAtUtc = [DateTime]::UtcNow.ToString('o')
    Environment = $environment
    Executable = $executable
    ExecutableFileStable = $binaryStable
    ContextBefore = $dataBefore
    ContextAfter = $dataAfter
    RequestedWindowState = $WindowState
    MinimumStateSeconds = $MinimumStateSeconds
    IdleEvidence = $IdleEvidence
    StatePollingIntervalMilliseconds = 1000
    AllSamplesEligible = $binaryStable -and @($results | Where-Object { -not $_.BaselineEligible }).Count -eq 0
    Samples = $results
    Limitations = @(
        'PrivateBytes is private commit, not physical resident memory. WorkingSet sums double-count shared pages.',
        'CPUPercentOfMachine is normalized by logical processors. Process/window polling can miss changes entirely between polls.',
        'IdleConfirmed is operator confirmation after scanning; log evidence alone cannot prove the live scanning flag.',
        'No startup or key-to-editable latency is measured. Diagnostics mode must be compared separately.'
    )
}
$json = $report | ConvertTo-Json -Depth 14
if ($OutputPath) {
    if (Test-Path -LiteralPath $OutputPath) { throw 'OutputPath already exists; keep previous evidence and use a new filename.' }
    $json | Set-Content -LiteralPath $OutputPath -Encoding UTF8
}
$json
