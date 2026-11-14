#Requires -Version 7.4
[CmdletBinding()]
param(
    [Parameter(Mandatory)][ValidateSet('software', 'femtovg')][string]$Renderer,
    [Parameter(Mandatory)][string]$PreparedDirectory,
    [Parameter(Mandatory)][string]$RunName,
    [ValidateRange(0, 120)][int]$Minutes = 30,
    [switch]$VisibleOnly,
    [switch]$AllowScanWarnings,
    [switch]$CheckOnly
)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'lib/resource-sample.ps1')
$PreparedDirectory = (Resolve-Path -LiteralPath $PreparedDirectory).Path
if ($RunName -notmatch '^[a-zA-Z0-9-]{1,40}$') { throw 'Use a simple unique RunName of at most 40 characters.' }
if (@(Get-CimInstance Win32_Process -Filter "Name LIKE 'velo-launcher%' OR Name LIKE 'velo-slint%'").Count) { throw 'Run only one Velo edition at a time.' }
$manifest = Read-VeloMetadataJson (Join-Path $PreparedDirectory 'manifest.json')
$exe = Join-Path $PreparedDirectory "binaries/$Renderer/velo-slint.exe"
$identity = Read-VeloRustBuildIdentity $exe
if ($identity.ExecutableSHA256 -ne $manifest.Builds.$Renderer.ExecutableSHA256) { throw 'Frozen executable changed.' }
$output = Join-Path $PreparedDirectory "runs/$RunName"
if (Test-Path -LiteralPath $output) { throw 'Use a new RunName; failed observations must be preserved.' }
New-Item -ItemType Directory -Path $output -Force | Out-Null
# Every run starts from the exact same seed, including after a failed attempt.
$data = Join-Path $output 'data'
Copy-Item -LiteralPath (Join-Path $PreparedDirectory 'seed') -Destination $data -Recurse
$log = Join-Path $output 'measurement-events.log'
$start = [Diagnostics.ProcessStartInfo]::new($exe)
$start.UseShellExecute = $false
$start.CreateNoWindow = $true
$start.WindowStyle = 'Hidden'
$start.RedirectStandardInput = $true
$start.RedirectStandardOutput = $true
$start.RedirectStandardError = $true
foreach ($argument in @('--measurement-business', '--measurement-stdio', ('--measurement-log=' + $log), ('--instance-id=rs12-' + $RunName), ('--data-dir=' + $data))) { $start.ArgumentList.Add($argument) }
$process = [Diagnostics.Process]::Start($start)
$collectorSHA256 = (Get-FileHash -LiteralPath $PSCommandPath).Hash
$stderr = $process.StandardError.ReadToEndAsync()
$watch = [Diagnostics.Stopwatch]::StartNew()
$loopWatch = $null
$startup = [Collections.Generic.List[object]]::new()
$rounds = [Collections.Generic.List[object]]::new()
$completed = $false
$forced = $false
$cyclesCompleted = 0
function Command([string]$Text) {
    if ($process.HasExited) { throw "Slint exited before command $Text (exit $($process.ExitCode))." }
    $process.StandardInput.WriteLine($Text)
    $process.StandardInput.Flush()
    $pending = $process.StandardOutput.ReadLineAsync()
    if (-not $pending.Wait(10000)) { throw "Measurement command timed out: $Text" }
    $line = $pending.Result
    if ($line -notlike "event=measurement_state command=$Text *") { throw "Unexpected acknowledgment: $line" }
    $fields = @{}
    foreach ($match in [regex]::Matches($line, '(\w+)=([^ ]+)')) { $fields[$match.Groups[1].Value] = $match.Groups[2].Value }
    if ($fields.business -ne 'true') { throw 'Refusing to measure fixture mode as business mode.' }
    if ($Text -eq 'hide' -and ($fields.visible -ne 'false' -or $fields.presented -ne 'false')) { throw 'Hide failed.' }
    if ($Text -eq 'show' -and ($fields.visible -ne 'true' -or $fields.presented -ne 'true')) { throw 'Show failed.' }
    $fields
}
function Sample([string]$Name, [string]$State, [int]$Settle) {
    for ($attempt = 1; $attempt -le 3; $attempt++) {
        if ($attempt -gt 1) {
            $null = Command $(if ($State -eq 'Visible') { 'show' } else { 'hide' })
            Start-Sleep -Seconds 2
        }
        $status = Command status
        if ($status.scanning -ne 'false' -or [int]$status.count -lt 1) { throw 'The business model is not idle and loaded.' }
        $observation = Read-VeloBusinessObservation $log $process.Id
        if (-not $observation.Complete -or $observation.Scanning -or ($observation.Warning -and (-not $AllowScanWarnings -or -not $observation.PublishedDuringScan))) { throw 'A completed scan with a published snapshot is required; reviewed partial scans need AllowScanWarnings.' }
        Write-Output "$Renderer / ${Name}: $Settle s observed settling, 3 x 15 s sampling"
        $path = Join-Path $output ($Name + $(if ($attempt -gt 1) { '-attempt-' + $attempt }) + '.json')
        $null = & (Join-Path $PSScriptRoot 'measure-resources.ps1') -ProcessId $process.Id -ApplicationKind Slint -DataMode Business `
            -MeasurementLog $log -DataDirectory $data -WindowHandle $script:rs12Hwnd -ExpectedExecutable $exe -AllowScanWarnings:$AllowScanWarnings `
            -WindowState $State -MinimumStateSeconds $Settle -Seconds 15 -Samples 3 -IdleConfirmed `
            -IdleEvidence 'Business configuration loaded and successful scan completed; real home rows/icons and Chinese query warmed; animations settled; no builds, benchmarks or screenshots during sampling.' -OutputPath $path
        $report = Read-VeloMetadataJson $path
        if ($report.AllSamplesEligible) { return }
        Write-Output "Ineligible sample retained at $path; a new observation will use a new filename."
    }
    throw "Resource sampling remained invalid after three observations: $Name"
}
function Workload {
    foreach ($command in @('show', 'home', 'warm-bottom', 'warm-top', 'query-editor', 'query-chinese', 'home', 'settings', 'home', 'hide')) {
        $null = Command $command
        Start-Sleep -Milliseconds 250
    }
}
try {
    do {
        $status = Command status
        $process.Refresh()
        $startup.Add([ordered]@{ AtSeconds = $watch.Elapsed.TotalSeconds; PrivateBytes = $process.PrivateMemorySize64; WorkingSet = $process.WorkingSet64; CPUSeconds = $process.TotalProcessorTime.TotalSeconds; Handles = $process.HandleCount; Threads = $process.Threads.Count; Scanning = $status.scanning; Count = $status.count })
        if ($watch.Elapsed.TotalSeconds -gt 120) { throw 'Initial scan did not settle within two minutes.' }
        Start-Sleep -Milliseconds 200
        $observation = Read-VeloBusinessObservation $log $process.Id
    } until ($observation.Complete -and -not $observation.Scanning -and $observation.Count -gt 0)
    $native = Read-VeloEventLines $log | Where-Object { $_ -like 'event=native_window *' } | Select-Object -First 1
    $script:rs12Hwnd = [regex]::Match($native, 'hwnd=([0-9A-F]+)').Groups[1].Value
    if (-not $script:rs12Hwnd) { throw 'The native window was not observed.' }
    $startup | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $output 'startup-polls.json')
    if ($CheckOnly) {
        foreach ($command in @('show', 'warm-bottom', 'warm-top', 'query-editor', 'query-chinese', 'settings', 'home', 'hide', 'show')) { $null = Command $command; Start-Sleep -Milliseconds 250 }
        $null = & (Join-Path $PSScriptRoot 'measure-resources.ps1') -ProcessId $process.Id -ApplicationKind Slint -DataMode Business `
            -MeasurementLog $log -DataDirectory $data -WindowHandle $script:rs12Hwnd -ExpectedExecutable $exe -WindowState Visible -Seconds 1 -Samples 1 -OutputPath (Join-Path $output 'check-only.json')
    } else {
        Workload
        # Start the operation/idle observation after the initial workload,
        # independently of process startup and the first discovery scan.
        $loopWatch = [Diagnostics.Stopwatch]::StartNew()
        $null = Command show
        $null = Command home
        Sample 'visible' Visible 3
        if (-not $VisibleOnly) {
            $null = Command hide
            Sample 'hidden-30s' Hidden 30
            # A new hide starts an independent five-minute observation.
            $null = Command show
            $null = Command hide
            Sample 'hidden-5m' Hidden 300
            for ($cycle = 0; $cycle -lt 100; $cycle++) {
                $null = Command show
                Start-Sleep -Milliseconds 200
                $null = Command hide
                Start-Sleep -Milliseconds 50
                $cyclesCompleted++
            }
            $null = Command show
            Sample 'visible-after-100-cycles' Visible 3
            while ($loopWatch.Elapsed.TotalMinutes -lt $Minutes) {
                Workload
                $process.Refresh()
                $rounds.Add([ordered]@{ AtSeconds = $loopWatch.Elapsed.TotalSeconds; PrivateBytes = $process.PrivateMemorySize64; WorkingSet = $process.WorkingSet64; CPUSeconds = $process.TotalProcessorTime.TotalSeconds; Handles = $process.HandleCount; Threads = $process.Threads.Count; Business = Read-VeloBusinessObservation $log $process.Id })
                Write-Output ("{0} / operation-idle loop: {1:N1}/{2} minutes" -f $Renderer, $loopWatch.Elapsed.TotalMinutes, $Minutes)
                Start-Sleep -Seconds ([math]::Min(45, [math]::Max(0, [int](($Minutes * 60) - $loopWatch.Elapsed.TotalSeconds))))
            }
            $loopWatch.Stop()
            $rounds | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath (Join-Path $output 'operation-idle-polls.json')
            $null = Command show
            $null = Command home
            Sample 'visible-after-loop' Visible 3
        }
    }
    $null = Command quit
    if (-not $process.WaitForExit(10000) -or $process.ExitCode -ne 0) { throw 'Normal lifecycle shutdown failed.' }
    $completed = $true
} finally {
    if ($rounds.Count -and -not (Test-Path -LiteralPath (Join-Path $output 'operation-idle-polls.json'))) {
        $rounds | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath (Join-Path $output 'operation-idle-polls.json')
    }
    $process.StandardInput.Dispose()
    if (-not $process.HasExited -and -not $process.WaitForExit(5000)) { $forced = $true; $process.Kill(); $process.WaitForExit() }
    $stderr.GetAwaiter().GetResult() | Set-Content -LiteralPath (Join-Path $output 'stderr.log')
    [ordered]@{ Completed = $completed; CheckOnly = $CheckOnly.IsPresent; VisibleOnly = $VisibleOnly.IsPresent; Renderer = $Renderer; SHA256 = $identity.ExecutableSHA256; CollectorSHA256 = $collectorSHA256; CollectorStable = $collectorSHA256 -eq (Get-FileHash -LiteralPath $PSCommandPath).Hash; ProcessId = $process.Id; ExitCode = $process.ExitCode; ForcedCleanup = $forced; ElapsedSeconds = $watch.Elapsed.TotalSeconds; OperationIdleSeconds = $(if ($loopWatch) { $loopWatch.Elapsed.TotalSeconds }); SyntheticCycles = $cyclesCompleted; ThirtyMinuteLoop = -not ($CheckOnly -or $VisibleOnly) -and $Minutes -ge 30 -and $loopWatch.Elapsed.TotalMinutes -ge 30; Method = 'Application stdio commands through normal business/lifecycle callbacks; no physical keyboard or calibrated latency claim.' } |
        ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $output 'completion.json')
    $process.Dispose()
}
