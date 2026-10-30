#Requires -Version 7.4
[CmdletBinding()]
param(
    [ValidateSet('Prepare', 'Arm', 'Capture', 'Warm', 'Cold', 'Export', 'Snapshot', 'Close', 'Finish')][string]$Action,
    [ValidateSet('Wails', 'Slint')][string]$ApplicationKind = 'Wails',
    [string]$Executable,
    [Parameter(Mandatory)][string]$WorkDirectory,
    [Parameter(Mandatory)][string]$OutputDirectory,
    [string]$SeedSource,
    [ValidateRange(1, 20)][int]$Count = 3,
    [ValidateSet('WarmStartup', 'ColdStartup')][string]$Kind = 'WarmStartup',
    [switch]$KeepLastRunning
)
$ErrorActionPreference = 'Stop'
if ($ApplicationKind -eq 'Slint') {
    & (Join-Path $PSScriptRoot 'collect-slint-latency.ps1') -Action $Action -WorkDirectory $WorkDirectory -OutputDirectory $OutputDirectory -Executable $Executable
    return
}
$workspace = Split-Path -Parent $PSScriptRoot
$WorkDirectory = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($WorkDirectory)
$OutputDirectory = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($OutputDirectory)
$manifestPath = Join-Path $WorkDirectory 'manifest.json'
$inputNames = @('config.json', 'index.json', 'history.json', 'application-library.json', 'quick-launch.json')
function Identity-Hash($Value) {
    $json = $Value | ConvertTo-Json -Depth 10 -Compress
    [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes($json)))
}
function Environment-Evidence([int]$RootProcessId) {
    $os = Get-CimInstance Win32_OperatingSystem
    $all = @(Get-CimInstance Win32_Process)
    $ids = @($RootProcessId)
    do {
        $children = @($all | Where-Object { $_.ParentProcessId -in $ids -and $_.ProcessId -notin $ids })
        $ids += @($children.ProcessId)
    } while ($children.Count)
    [pscustomobject]@{
        WindowsVersion = $os.Version; WindowsBuild = $os.BuildNumber
        Processor = @((Get-CimInstance Win32_Processor).Name)
        WebView2 = @($all | Where-Object { $_.ProcessId -in $ids -and $_.Name -eq 'msedgewebview2.exe' } | ForEach-Object {
            (Get-Item -LiteralPath $_.ExecutablePath).VersionInfo.FileVersion
        } | Sort-Object -Unique)
    }
}
function Fingerprint([string]$Directory) {
    $hashes = [ordered]@{}
    foreach ($name in $inputNames) {
        $path = Join-Path $Directory $name
        $hashes[$name] = $(if (Test-Path -LiteralPath $path) { (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash } else { $null })
    }
    [pscustomobject]$hashes
}
function Input-Metadata([string]$Directory) {
    $metadata = [ordered]@{}
    foreach ($name in $inputNames) {
        $path = Join-Path $Directory $name
        $metadata[$name] = $(if (Test-Path -LiteralPath $path) {
            $file = Get-Item -LiteralPath $path
            [pscustomobject]@{ Bytes = $file.Length; ModifiedUtcTicks = $file.LastWriteTimeUtc.Ticks }
        } else { $null })
    }
    [pscustomobject]$metadata
}
function Probe-Call($Control, [string]$Route, [string]$Method = 'Get') {
    Invoke-RestMethod -Uri ($Control.URL + $Route) -Method $Method -Headers @{ Authorization = 'Bearer ' + $Control.Token }
}
function Save-Events($Control, [string]$Name) {
    $state = Probe-Call $Control '/state'
    $state | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath (Join-Path $OutputDirectory ($Name + '-state.json')) -Encoding UTF8
    $source = Join-Path (Split-Path -Parent $script:controlPath) 'events.jsonl'
    if (Test-Path -LiteralPath $source) { Copy-Item -LiteralPath $source -Destination (Join-Path $OutputDirectory ($Name + '-events.jsonl')) -Force }
    $contextPath = Join-Path $OutputDirectory ($Name + '-context.json')
    if (Test-Path -LiteralPath $contextPath) {
        $context = Get-Content -LiteralPath $contextPath -Raw -Encoding UTF8 | ConvertFrom-Json
        $environment = Environment-Evidence $state.ProcessId
        $context | Add-Member -NotePropertyName Environment -NotePropertyValue $environment -Force
        $context | Add-Member -NotePropertyName EnvironmentId -NotePropertyValue (Identity-Hash $environment) -Force
        $context | Add-Member -NotePropertyName InputAfter -NotePropertyValue (Fingerprint $manifest.DataDirectory) -Force
        $context | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath $contextPath -Encoding UTF8
    }
    $state
}
if ($Action -eq 'Prepare') {
    if (Test-Path -LiteralPath $WorkDirectory) { throw 'Use a fresh WorkDirectory.' }
    if (-not $SeedSource) { throw 'SeedSource is required.' }
    New-Item -ItemType Directory -Path $WorkDirectory, $OutputDirectory -Force | Out-Null
    $local = Join-Path $WorkDirectory 'local'
    New-Item -ItemType Directory -Path $local -Force | Out-Null
    Copy-Item -LiteralPath $SeedSource -Destination (Join-Path $local 'Velo') -Recurse
    $configPath = Join-Path $local 'Velo/config.json'
    $config = Get-Content -LiteralPath $configPath -Raw -Encoding UTF8 | ConvertFrom-Json
    $config.auto_check_updates = $false
    $config | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath $configPath -Encoding UTF8
    $original = @(Get-CimInstance Win32_Process -Filter "Name LIKE 'velo-launcher%'")
    if ($original.Count -gt 1) { throw 'Multiple Velo processes found.' }
    $restore = $null
    if ($original.Count) {
        if ($original[0].ExecutablePath -ne 'C:\Program Files\HBLADEH\Velo\velo-launcher.exe') { throw 'Unexpected running executable.' }
        $live = Get-Process -Id $original[0].ProcessId
        $restore = [pscustomobject]@{ ProcessId = $live.Id; StartedTicks = $live.StartTime.ToUniversalTime().Ticks; Executable = $original[0].ExecutablePath; Arguments = @('--background') }
    }
    $exe = (Resolve-Path -LiteralPath (Join-Path $workspace 'build/bin/velo-launcher-latency.exe')).Path
    $file = Get-Item -LiteralPath $exe
    $manifest = [ordered]@{
        ReadyAtUtc = [DateTime]::UtcNow.ToString('o'); Executable = $exe
        ExecutableSHA256 = (Get-FileHash -LiteralPath $exe -Algorithm SHA256).Hash
        ExecutableBytes = $file.Length; ExecutableModifiedUtc = $file.LastWriteTimeUtc.ToString('o')
        LocalAppData = $local; DataDirectory = (Join-Path $local 'Velo'); Restore = $restore
        PreparedInputs = (Fingerprint (Join-Path $local 'Velo'))
        UserInputsBefore = (Fingerprint (Join-Path $env:LOCALAPPDATA 'Velo'))
        ProbeGoSHA256 = (Get-FileHash -LiteralPath (Join-Path $workspace 'latency_probe_windows.go') -Algorithm SHA256).Hash
        ProbeJSSHA256 = (Get-FileHash -LiteralPath (Join-Path $PSScriptRoot 'latency-probe.js') -Algorithm SHA256).Hash
        Method = 'Instrumented production; OS event lower bound to focused writable visible search input IPC acknowledgement; durations are upper bounds.'
    }
    $manifest | ConvertTo-Json -Depth 7 | Set-Content -LiteralPath $manifestPath -Encoding UTF8
    $manifest | ConvertTo-Json -Depth 7 | Set-Content -LiteralPath (Join-Path $OutputDirectory 'build-and-input.json') -Encoding UTF8
    if ($restore) {
        $current = Get-CimInstance Win32_Process -Filter "ProcessId=$($restore.ProcessId)"
        if ($current.ExecutablePath -ne $restore.Executable -or (Get-Process -Id $restore.ProcessId).StartTime.ToUniversalTime().Ticks -ne $restore.StartedTicks) { throw 'Original process identity changed.' }
        Stop-Process -Id $restore.ProcessId
    }
    'Prepared isolated timing profile.'
    exit
}
$manifest = Get-Content -LiteralPath $manifestPath -Raw -Encoding UTF8 | ConvertFrom-Json
if ($Action -eq 'Cold') {
    throw 'Use Arm -Kind ColdStartup, launch the prepared shortcut, then Capture. Scripted process launch cannot establish foreground input readiness.'
}
if ($Action -eq 'Arm') {
    if (@(Get-CimInstance Win32_Process -Filter "Name LIKE 'velo-launcher%'").Count) { throw 'A Velo instance is already running.' }
    $boot = (Get-CimInstance Win32_OperatingSystem).LastBootUpTime.ToUniversalTime()
    $bootID = $boot.ToString('o')
    if ($Kind -eq 'ColdStartup') {
        if (-not $manifest.ColdPreparedAtUtc -or -not $manifest.ColdInputs) { throw 'Run Finish before rebooting to freeze the next cold input identity.' }
        if ($boot -le ([DateTimeOffset]$manifest.ColdPreparedAtUtc).UtcDateTime) { throw 'ColdStartup requires a reboot after the input profile was frozen.' }
        if (-not $manifest.ColdInputMetadata -or (Identity-Hash (Input-Metadata $manifest.DataDirectory)) -ne (Identity-Hash $manifest.ColdInputMetadata)) { throw 'The frozen input file metadata changed; prepare again before a new reboot.' }
        $file = Get-Item -LiteralPath $manifest.Executable
        if ($file.Length -ne $manifest.ExecutableBytes -or $file.LastWriteTimeUtc -ne ([DateTimeOffset]$manifest.ExecutableModifiedUtc).UtcDateTime) { throw 'The frozen probe binary changed; prepare a new run before rebooting.' }
        $ledger = Join-Path $WorkDirectory 'boot-uses.jsonl'
        if (Test-Path -LiteralPath $ledger) {
            $uses = @(Get-Content -LiteralPath $ledger -Encoding UTF8 | ForEach-Object { $_ | ConvertFrom-Json } | Where-Object BootID -EQ $bootID)
            if ($uses.Count) { throw 'This boot already contains a probe launch; cached launches are warm.' }
        }
        $log = Join-Path $env:LOCALAPPDATA 'Velo/logs/velo.log'
        if (Test-Path -LiteralPath $log) {
            $uses = @(Get-Content -LiteralPath $log -Encoding UTF8 | ForEach-Object { try { $_ | ConvertFrom-Json } catch {} } | Where-Object msg -EQ 'application started' | Where-Object { ([DateTimeOffset]$_.time).UtcDateTime -ge $boot })
            if ($uses.Count) { throw 'Installed Velo has already run during this boot; cold condition rejected.' }
        }
    }
    $name = $Kind.ToLowerInvariant() + '-gui-' + [DateTime]::UtcNow.ToString('yyyyMMdd-HHmmss-fff')
    $dir = Join-Path $WorkDirectory $name
    New-Item -ItemType Directory -Path $dir | Out-Null
    $environment = @{ LOCALAPPDATA = $manifest.LocalAppData; VELO_LATENCY_DIR = $dir; VELO_LATENCY_KIND = $Kind; VELO_LATENCY_SHA256 = $manifest.ExecutableSHA256; VELO_LATENCY_BOOT_ID = $bootID }
    $environment | ConvertTo-Json | Set-Content -LiteralPath ($manifest.Executable + '.probe.json') -Encoding UTF8
    $before = $(if ($Kind -eq 'WarmStartup') { Fingerprint $manifest.DataDirectory } else { $manifest.ColdInputs })
    [pscustomobject]@{ Name = $name; Kind = $Kind; BootID = $bootID; ControlPath = (Join-Path $dir 'control.json'); InputBefore = $before; ArmedAtUtc = [DateTime]::UtcNow.ToString('o'); ColdPreparedAtUtc = $(if ($Kind -eq 'ColdStartup') { $manifest.ColdPreparedAtUtc } else { $null }) } | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $WorkDirectory 'armed.json') -Encoding UTF8
    [pscustomobject]@{ BootID = $bootID; LaunchedAtUtc = [DateTime]::UtcNow.ToString('o'); Kind = $Kind; GUIArmed = $true } | ConvertTo-Json -Compress | Add-Content -LiteralPath (Join-Path $WorkDirectory 'boot-uses.jsonl') -Encoding UTF8
    "Armed $name; launch Velo timing.lnk from the prepared Explorer folder."
    exit
}
if ($Action -eq 'Capture') {
    $armed = Get-Content -LiteralPath (Join-Path $WorkDirectory 'armed.json') -Raw -Encoding UTF8 | ConvertFrom-Json
    $script:controlPath = $armed.ControlPath
    if (-not (Test-Path -LiteralPath $script:controlPath)) { throw 'Launch the prepared shortcut before capturing.' }
    $control = Get-Content -LiteralPath $script:controlPath -Raw -Encoding UTF8 | ConvertFrom-Json
    $process = Get-Process -Id $control.ProcessId
    $current = Get-CimInstance Win32_Process -Filter "ProcessId=$($control.ProcessId)"
    if ($current.ExecutablePath -ne $manifest.Executable) { throw 'Unexpected GUI-launched executable.' }
    [pscustomobject]@{ Name = $armed.Name; ProcessId = $process.Id; StartedTicks = $process.StartTime.ToUniversalTime().Ticks; ControlPath = $script:controlPath; Executable = $manifest.Executable } | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $WorkDirectory 'active.json') -Encoding UTF8
    $watch = [Diagnostics.Stopwatch]::StartNew()
    do {
        $state = Probe-Call $control '/state'
        if ($watch.Elapsed.TotalSeconds -gt 15) { $null = Probe-Call $control '/expire' Post; break }
        if (-not $state.Records.Count) { Start-Sleep -Milliseconds 50 }
    } until ($state.Records.Count)
    $state = Save-Events $control $armed.Name
    $environment = Environment-Evidence $process.Id
    [pscustomobject]@{ Name = $armed.Name; Kind = $armed.Kind; BootID = $armed.BootID; ProcessId = $process.Id; ExecutableSHA256 = $manifest.ExecutableSHA256; InputBefore = $armed.InputBefore; Environment = $environment; EnvironmentId = (Identity-Hash $environment); ColdPreparedAtUtc = $armed.ColdPreparedAtUtc; LaunchMethod = 'Explorer shortcut; no scripted foreground activation' } | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $OutputDirectory ($armed.Name + '-context.json')) -Encoding UTF8
    $state.Records | Select-Object ID, Kind, UpperBoundMilliseconds, Valid, Reason, NativeActive, ForegroundProcessID | ConvertTo-Json
    if (-not $KeepLastRunning) {
        $watch.Restart()
        while ($state.Status.scanning -and $watch.Elapsed.TotalSeconds -lt 60) { Start-Sleep -Milliseconds 100; $state = Probe-Call $control '/state' }
        $null = Probe-Call $control '/quit' Post
        if (-not $process.WaitForExit(10000)) { throw 'Normal probe shutdown timed out.' }
    }
    if ((Get-FileHash -LiteralPath $manifest.Executable -Algorithm SHA256).Hash -ne $manifest.ExecutableSHA256) { throw 'Measured binary hash changed.' }
    exit
}
if ($Action -eq 'Warm') {
    if (@(Get-CimInstance Win32_Process -Filter "Name LIKE 'velo-launcher%'").Count) { throw 'A Velo instance is already running.' }
    $boot = (Get-CimInstance Win32_OperatingSystem).LastBootUpTime.ToUniversalTime()
    $bootID = $boot.ToString('o')
    $ledger = Join-Path $WorkDirectory 'boot-uses.jsonl'
    foreach ($iteration in 1..$Count) {
        $kind = 'WarmStartup'
        $name = ($Action.ToLowerInvariant() + '-' + [DateTime]::UtcNow.ToString('yyyyMMdd-HHmmss-fff'))
        $dir = Join-Path $WorkDirectory $name
        New-Item -ItemType Directory -Path $dir | Out-Null
        [pscustomobject]@{ BootID = $bootID; LaunchedAtUtc = [DateTime]::UtcNow.ToString('o'); Kind = $kind } | ConvertTo-Json -Compress | Add-Content -LiteralPath $ledger -Encoding UTF8
        $before = Fingerprint $manifest.DataDirectory
        $environment = @{
            LOCALAPPDATA = $manifest.LocalAppData; VELO_LATENCY_DIR = $dir; VELO_LATENCY_KIND = $kind
            VELO_LATENCY_SHA256 = $manifest.ExecutableSHA256; VELO_LATENCY_BOOT_ID = $bootID
        }
        # This is an interactive measurement; the search window must be shown.
        $process = Start-Process -FilePath $manifest.Executable -WorkingDirectory $workspace -Environment $environment -WindowStyle Normal -PassThru -RedirectStandardOutput (Join-Path $dir 'stdout.txt') -RedirectStandardError (Join-Path $dir 'stderr.txt')
        $script:controlPath = Join-Path $dir 'control.json'
        $watch = [Diagnostics.Stopwatch]::StartNew()
        do {
            Start-Sleep -Milliseconds 50
            if ($process.HasExited) { throw 'The intended probe process exited.' }
            if ($watch.Elapsed.TotalSeconds -gt 15) { throw 'Probe control endpoint did not start.' }
        } until (Test-Path -LiteralPath $script:controlPath)
        $control = Get-Content -LiteralPath $script:controlPath -Raw -Encoding UTF8 | ConvertFrom-Json
        $identity = [pscustomobject]@{ Name = $name; ProcessId = $process.Id; StartedTicks = $process.StartTime.ToUniversalTime().Ticks; ControlPath = $script:controlPath; Executable = $manifest.Executable }
        $identity | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $WorkDirectory 'active.json') -Encoding UTF8
        do {
            Start-Sleep -Milliseconds 50
            $state = Probe-Call $control '/state'
            if ($watch.Elapsed.TotalSeconds -gt 15) { $null = Probe-Call $control '/expire' Post; break }
        } until ($state.Records.Count -gt 0)
        $state = Save-Events $control $name
        $environment = Environment-Evidence $process.Id
        [pscustomobject]@{ Name = $name; Kind = $kind; BootID = $bootID; ProcessId = $process.Id; ExecutableSHA256 = $manifest.ExecutableSHA256; InputBefore = $before; Environment = $environment; EnvironmentId = (Identity-Hash $environment); InitialCount = $state.Status.count } | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $OutputDirectory ($name + '-context.json')) -Encoding UTF8
        if ($state.Records.Count) { $state.Records | Select-Object ID, Kind, UpperBoundMilliseconds, Valid, Reason | ConvertTo-Json }
        $keep = $KeepLastRunning -and $iteration -eq $Count
        if (-not $keep) {
            # Complete the scan before normal exit so subsequent warm input is fixed.
            $scanWatch = [Diagnostics.Stopwatch]::StartNew()
            while ($state.Status.scanning -and $scanWatch.Elapsed.TotalSeconds -lt 60) { Start-Sleep -Milliseconds 100; $state = Probe-Call $control '/state' }
            $null = Probe-Call $control '/quit' Post
            if (-not $process.WaitForExit(10000)) { throw 'Normal probe shutdown timed out.' }
        }
        if ((Get-FileHash -LiteralPath $manifest.Executable -Algorithm SHA256).Hash -ne $manifest.ExecutableSHA256) { throw 'Measured binary hash changed.' }
    }
    exit
}
if ($Action -eq 'Export') {
    $seen = [ordered]@{}
    foreach ($file in (Get-ChildItem -LiteralPath $OutputDirectory -Filter '*-events.jsonl' | Sort-Object Name)) {
        $contextPath = Join-Path $OutputDirectory (($file.BaseName -replace '-events$', '') + '-context.json')
        $context = $(if (Test-Path -LiteralPath $contextPath) { Get-Content -LiteralPath $contextPath -Raw -Encoding UTF8 | ConvertFrom-Json } else { $null })
        foreach ($line in (Get-Content -LiteralPath $file.FullName -Encoding UTF8)) {
            $record = $line | ConvertFrom-Json
            if ($record.Kind -notin @('WarmStartup', 'ColdStartup', 'HotkeyActivation')) { continue }
            $hash = $record.ExecutableSHA256
            if (-not $hash) {
                if ($context -and $context.PSObject.Properties.Name -contains 'ExecutableSHA256') { $hash = $context.ExecutableSHA256 }
                else { throw "Missing binary identity for $($file.Name); do not relabel historical records with the latest build hash." }
            }
            $identity = Identity-Hash @($hash, $record.ProcessID, $record.Kind, $record.ID, $record.StartedAtUTC, $record.AcknowledgedAtUTC)
            $evidence = $file.Name + '#' + $record.ID
            $recordJSON = $record | ConvertTo-Json -Depth 10 -Compress
            if ($seen.Contains($identity)) {
                if ($seen[$identity].RecordJSON -ne $recordJSON) { throw "Conflicting copies of observation $identity." }
                $evidence = $seen[$identity].Row.Evidence + ';' + $evidence
            }
            $inputs = $(if ($context -and $context.PSObject.Properties.Name -contains 'InputBefore') { $context.InputBefore } else { $null })
            $environmentId = $(if ($context -and $context.PSObject.Properties.Name -contains 'EnvironmentId') { $context.EnvironmentId } else { $null })
            $contextValid = $null -ne $inputs -and -not [string]::IsNullOrWhiteSpace($environmentId)
            $contextRank = [int]($null -ne $inputs) + 2 * [int](-not [string]::IsNullOrWhiteSpace($environmentId))
            if ($seen.Contains($identity) -and $seen[$identity].ContextRank -ge $contextRank) {
                $seen[$identity].Row.Evidence = $evidence
                continue
            }
            if ($record.Kind -eq 'HotkeyActivation') {
                $contextValid = $contextValid -and $context.PSObject.Properties.Name -contains 'InputAfter' -and (Identity-Hash $inputs) -eq (Identity-Hash $context.InputAfter)
            }
            $manualFocusRecovery = $record.Kind -ne 'HotkeyActivation' -and $context -and
                $context.PSObject.Properties.Name -contains 'ManualFocusRecovery' -and $context.ManualFocusRecovery -and $context.ProcessId -eq $record.ProcessID
            $eventBootID = $record.BootID
            $bootSource = $(if ($eventBootID) { 'Record' } else { 'Unavailable' })
            if (-not $eventBootID -and $context -and $context.PSObject.Properties.Name -contains 'BootID' -and
                $context.PSObject.Properties.Name -contains 'ProcessId' -and $context.PSObject.Properties.Name -contains 'Kind' -and
                $context.ProcessId -eq $record.ProcessID -and $context.Kind -eq $record.Kind -and $context.BootID) {
                $eventBootID = $context.BootID
                $bootSource = 'MatchingSessionContext'
            }
            $row = [pscustomobject]@{
                Kind = $record.Kind; ExecutableSHA256 = $hash
                EnvironmentId = $(if ($environmentId) { $environmentId } else { 'unverified-legacy-environment-' + $hash })
                InputId = $(if ($inputs) { Identity-Hash $inputs } else { 'unverified-legacy-input-' + $hash })
                Method = 'ConservativeUpperBound_OSClock_DOMFocusReady_IPCAck' + $(if ($manualFocusRecovery) { '_ManualFocusRecovery' } else { '' })
                Boundary = $(if ($record.Kind -eq 'HotkeyActivation') { 'KeyDownToEditable' } else { 'ProcessStartToEditable' })
                DurationMilliseconds = $record.UpperBoundMilliseconds.ToString('R', [Globalization.CultureInfo]::InvariantCulture)
                Valid = ($record.Valid -and $contextValid).ToString().ToLowerInvariant(); Evidence = $evidence
                TimestampUtc = ([DateTimeOffset]$record.AcknowledgedAtUTC).UtcDateTime.ToString('o')
                BootId = $(if ($eventBootID -is [DateTime]) { $eventBootID.ToUniversalTime().ToString('o') } else { $eventBootID })
                BootIdSource = $bootSource
                ProcessId = $record.ProcessID
                InjectedKeyboardEvent = $record.InjectedKeyboardEvent; ClockUncertaintyMilliseconds = $record.ClockUncertaintyMS
                ExclusionReason = $(if (-not $record.Valid) { $record.Reason } elseif (-not $contextValid) { 'Missing environment/input evidence, or hotkey profile changed.' } else { '' })
            }
            $seen[$identity] = [pscustomobject]@{ RecordJSON = $recordJSON; Row = $row; ContextRank = $contextRank }
        }
    }
    $rows = @($seen.Values | ForEach-Object Row)
    if (-not $rows.Count) { throw 'No raw observations to export.' }
    $rows | Export-Csv -LiteralPath (Join-Path $OutputDirectory 'desktop-latency.csv') -NoTypeInformation -Encoding UTF8
    $rows | Group-Object Kind | Select-Object Name, Count | ConvertTo-Json
    exit
}
if ($Action -in @('Snapshot', 'Close', 'Finish')) {
    $activePath = Join-Path $WorkDirectory 'active.json'
    if (Test-Path -LiteralPath $activePath) {
        $active = Get-Content -LiteralPath $activePath -Raw -Encoding UTF8 | ConvertFrom-Json
        $current = Get-CimInstance Win32_Process -Filter "ProcessId=$($active.ProcessId)"
        $identityMatches = $current -and $current.ExecutablePath -eq $active.Executable
        if ($identityMatches) {
            $live = Get-Process -Id $active.ProcessId
            $identityMatches = $live.StartTime.ToUniversalTime().Ticks -eq $active.StartedTicks
        }
        if ($identityMatches) {
            $script:controlPath = $active.ControlPath
            $control = Get-Content -LiteralPath $script:controlPath -Raw -Encoding UTF8 | ConvertFrom-Json
            $null = Save-Events $control $active.Name
            if ($Action -eq 'Snapshot') { exit }
            $null = Probe-Call $control '/quit' Post
            if (-not $live.WaitForExit(10000)) { throw 'Normal shutdown timed out.' }
        } elseif ($current) {
            Write-Warning 'The old probe PID has been reused; the unrelated process is left untouched.'
        }
    }
    if ($Action -eq 'Snapshot') { throw 'No live probe to snapshot.' }
    if ($Action -eq 'Close') { exit }
    if ((Get-FileHash -LiteralPath $manifest.Executable -Algorithm SHA256).Hash -ne $manifest.ExecutableSHA256) { throw 'Frozen probe binary changed; prepare a new measurement.' }
    # Keep the isolated profile for manual checks, but disarm the old session.
    # Otherwise a later launch can overwrite its control endpoint and raw log.
    @{ LOCALAPPDATA = $manifest.LocalAppData } | ConvertTo-Json |
        Set-Content -LiteralPath ($manifest.Executable + '.probe.json') -Encoding UTF8
    $coldInputs = Fingerprint $manifest.DataDirectory
    $manifest | Add-Member -NotePropertyName ColdInputs -NotePropertyValue $coldInputs -Force
    $manifest | Add-Member -NotePropertyName ColdInputMetadata -NotePropertyValue (Input-Metadata $manifest.DataDirectory) -Force
    $manifest | Add-Member -NotePropertyName ColdPreparedAtUtc -NotePropertyValue ([DateTime]::UtcNow.ToString('o')) -Force
    $manifest | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath $manifestPath -Encoding UTF8
    [pscustomobject]@{ PreparedAtUtc = $manifest.ColdPreparedAtUtc; ExecutableSHA256 = $manifest.ExecutableSHA256; Inputs = $coldInputs; InputId = (Identity-Hash $coldInputs) } | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $OutputDirectory 'next-cold-input.json') -Encoding UTF8
    $userAfter = Fingerprint (Join-Path $env:LOCALAPPDATA 'Velo')
    $unchanged = ($manifest.UserInputsBefore | ConvertTo-Json -Compress) -eq ($userAfter | ConvertTo-Json -Compress)
    $restored = $null
    if ($manifest.Restore -and -not @(Get-CimInstance Win32_Process -Filter "Name LIKE 'velo-launcher%'").Count) {
        $restored = Start-Process -FilePath $manifest.Restore.Executable -ArgumentList $manifest.Restore.Arguments -WindowStyle Hidden -PassThru
    }
    [pscustomobject]@{ FinishedAtUtc = [DateTime]::UtcNow.ToString('o'); UserInputsUnchangedBeforeRestore = $unchanged; RestoredProcessId = $(if ($restored) { $restored.Id } else { $null }) } | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $OutputDirectory 'completion.json') -Encoding UTF8
}
