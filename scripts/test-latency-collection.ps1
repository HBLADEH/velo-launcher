$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$testRoot = Join-Path ([IO.Path]::GetTempPath()) ('velo-latency-export-' + [guid]::NewGuid().ToString('N'))
$collector = Join-Path $PSScriptRoot 'collect-latency.ps1'
try {
    New-Item -ItemType Directory -Path $testRoot | Out-Null
    '{}' | Set-Content -LiteralPath (Join-Path $testRoot 'manifest.json') -Encoding UTF8
    $inputs = [pscustomobject]@{ 'config.json' = 'frozen-config'; 'index.json' = 'frozen-index' }
    $context = [pscustomobject]@{ ExecutableSHA256 = ('a' * 64); InputBefore = $inputs; InputAfter = $inputs; EnvironmentId = 'synthetic-fixture-environment' }
    $record = [pscustomobject]@{
        ID = 'fixture-01'; Kind = 'HotkeyActivation'; ProcessID = 123; ExecutableSHA256 = ('a' * 64)
        StartedAtUTC = '2026-10-27T00:00:00Z'; AcknowledgedAtUTC = '2026-10-27T00:00:00.020Z'
        UpperBoundMilliseconds = 20.0; BootID = 'fixture-boot'; Valid = $true; Reason = ''
        InjectedKeyboardEvent = $false; ClockUncertaintyMS = 17.0
    }
    foreach ($name in @('one', 'copy')) {
        $record | ConvertTo-Json -Compress | Set-Content -LiteralPath (Join-Path $testRoot ($name + '-events.jsonl')) -Encoding UTF8
        $context | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $testRoot ($name + '-context.json')) -Encoding UTF8
    }
    $null = & $collector -Action Export -WorkDirectory $testRoot -OutputDirectory $testRoot
    $rows = @(Import-Csv -LiteralPath (Join-Path $testRoot 'desktop-latency.csv'))
    if ($rows.Count -ne 1 -or $rows[0].Valid -ne 'true' -or $rows[0].InputId -notmatch '^[A-F0-9]{64}$' -or $rows[0].Evidence -notlike '*copy-events.jsonl*one-events.jsonl*') {
        throw 'Identical raw events must count once, retain both references and carry an input fingerprint.'
    }
    if ($rows[0].TimestampUtc -ne '2026-10-27T00:00:00.0200000Z' -or $rows[0].ProcessId -ne 123) { throw 'UTC timestamp precision and process identity must survive CSV export.' }
    $legacyRoot = Join-Path $testRoot 'legacy-timeout'
    New-Item -ItemType Directory -Path $legacyRoot | Out-Null
    '{}' | Set-Content -LiteralPath (Join-Path $legacyRoot 'manifest.json') -Encoding UTF8
    $legacy = $record.PSObject.Copy()
    $legacy.ID = 'startup'; $legacy.Kind = 'ColdStartup'; $legacy.Valid = $false; $legacy.BootID = ''; $legacy.Reason = 'fixture timeout'
    $legacy | ConvertTo-Json -Compress | Set-Content -LiteralPath (Join-Path $legacyRoot 'legacy-events.jsonl') -Encoding UTF8
    $legacyContext = [pscustomobject]@{ Kind = 'ColdStartup'; ProcessId = 123; BootID = '2026-10-27T00:00:00Z'; InputBefore = $inputs; EnvironmentId = 'synthetic-fixture-environment' }
    $legacyContext | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $legacyRoot 'legacy-context.json') -Encoding UTF8
    $null = & $collector -Action Export -WorkDirectory $legacyRoot -OutputDirectory $legacyRoot
    $legacyRow = Import-Csv -LiteralPath (Join-Path $legacyRoot 'desktop-latency.csv')
    if ($legacyRow.BootId -ne '2026-10-27T00:00:00.0000000Z' -or $legacyRow.BootIdSource -ne 'MatchingSessionContext' -or $legacyRow.Valid -ne 'false') { throw 'Legacy timeouts require matching session boot identity without becoming valid.' }
    $legacyContext.ProcessId = 999
    $legacyContext | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $legacyRoot 'legacy-context.json') -Encoding UTF8
    $null = & $collector -Action Export -WorkDirectory $legacyRoot -OutputDirectory $legacyRoot
    $legacyRow = Import-Csv -LiteralPath (Join-Path $legacyRoot 'desktop-latency.csv')
    if ($legacyRow.BootId -or $legacyRow.BootIdSource -ne 'Unavailable') { throw 'Do not borrow another process boot identity.' }
    $firstInput = $rows[0].InputId
    $context.InputBefore = [pscustomobject]@{ 'config.json' = 'other-config'; 'index.json' = 'frozen-index' }
    foreach ($name in @('one', 'copy')) {
        $context | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $testRoot ($name + '-context.json')) -Encoding UTF8
    }
    $null = & $collector -Action Export -WorkDirectory $testRoot -OutputDirectory $testRoot
    $changed = @(Import-Csv -LiteralPath (Join-Path $testRoot 'desktop-latency.csv'))
    if ($changed[0].InputId -eq $firstInput -or $changed[0].Valid -ne 'false') { throw 'Changed input must change its group and exclude unstable hotkey evidence.' }
    $record.Valid = $false
    $record | ConvertTo-Json -Compress | Set-Content -LiteralPath (Join-Path $testRoot 'one-events.jsonl') -Encoding UTF8
    $rejected = $false
    try { $null = & $collector -Action Export -WorkDirectory $testRoot -OutputDirectory $testRoot } catch { $rejected = $_.Exception.Message -like 'Conflicting copies*' }
    if (-not $rejected) { throw 'Conflicting copies of the same observation must be rejected.' }
    $rejected = $false
    try { $null = & $collector -Action Cold -WorkDirectory $testRoot -OutputDirectory $testRoot } catch { $rejected = $_.Exception.Message -like 'Use Arm*' }
    if (-not $rejected) { throw 'The legacy cold path must fail before touching app data or launching a process.' }
    # Arm with relative caller paths without starting any real process.
    function Get-CimInstance {
        param([string]$ClassName, [string]$Filter)
        if ($ClassName -eq 'Win32_Process') { return }
        if ($ClassName -eq 'Win32_OperatingSystem') { return [pscustomobject]@{ LastBootUpTime = [DateTime]::UtcNow.AddHours(-1) } }
        throw 'Unexpected CIM query in the relative-path fixture.'
    }
    [pscustomobject]@{ Executable = (Join-Path $testRoot 'fixture.exe'); ExecutableSHA256 = ('a' * 64); LocalAppData = $testRoot; DataDirectory = $testRoot } |
        ConvertTo-Json | Set-Content -LiteralPath (Join-Path $testRoot 'manifest.json') -Encoding UTF8
    Push-Location (Split-Path -Parent $testRoot)
    try {
        $relative = Split-Path -Leaf $testRoot
        $null = & $collector -Action Arm -Kind WarmStartup -WorkDirectory $relative -OutputDirectory $relative
    } finally { Pop-Location }
    $sidecar = Get-Content -LiteralPath (Join-Path $testRoot 'fixture.exe.probe.json') -Raw | ConvertFrom-Json
    $armed = Get-Content -LiteralPath (Join-Path $testRoot 'armed.json') -Raw | ConvertFrom-Json
    if (-not [IO.Path]::IsPathFullyQualified($sidecar.VELO_LATENCY_DIR) -or -not [IO.Path]::IsPathFullyQualified($armed.ControlPath)) { throw 'Relative caller paths must become absolute before GUI launch.' }
    [pscustomobject]@{ ProcessId = 9179; Executable = (Join-Path $testRoot 'fixture.exe'); StartedTicks = 1 } |
        ConvertTo-Json | Set-Content -LiteralPath (Join-Path $testRoot 'active.json') -Encoding UTF8
    function Get-CimInstance {
        param([string]$ClassName, [string]$Filter)
        if ($ClassName -eq 'Win32_Process') { return [pscustomobject]@{ ProcessId = 9179; ExecutablePath = 'C:\unrelated-fixture.exe' } }
        throw 'Unexpected CIM query in the stale-PID fixture.'
    }
    $null = & $collector -Action Close -WorkDirectory $testRoot -OutputDirectory $testRoot -WarningAction SilentlyContinue
    'fixture binary' | Set-Content -LiteralPath (Join-Path $testRoot 'fixture.exe') -Encoding UTF8
    $finishManifest = [pscustomobject]@{
        Executable = (Join-Path $testRoot 'fixture.exe'); ExecutableSHA256 = (Get-FileHash -LiteralPath (Join-Path $testRoot 'fixture.exe')).Hash
        LocalAppData = $testRoot; DataDirectory = $testRoot; Restore = $null
        UserInputsBefore = [pscustomobject]@{ 'config.json' = $null; 'index.json' = $null; 'history.json' = $null; 'application-library.json' = $null; 'quick-launch.json' = $null }
    }
    $finishManifest | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $testRoot 'manifest.json') -Encoding UTF8
    $previousLocal = $env:LOCALAPPDATA
    try {
        $env:LOCALAPPDATA = $testRoot
        $null = & $collector -Action Finish -WorkDirectory $testRoot -OutputDirectory $testRoot -WarningAction SilentlyContinue
    } finally { $env:LOCALAPPDATA = $previousLocal }
    $disarmed = Get-Content -LiteralPath (Join-Path $testRoot 'fixture.exe.probe.json') -Raw | ConvertFrom-Json
    $prepared = Get-Content -LiteralPath (Join-Path $testRoot 'manifest.json') -Raw | ConvertFrom-Json
    if ($disarmed.LOCALAPPDATA -ne $testRoot -or $disarmed.PSObject.Properties.Name -contains 'VELO_LATENCY_DIR' -or -not $prepared.ColdPreparedAtUtc) { throw 'Finish must disarm the old session while retaining profile isolation and cold preparation.' }
    'Latency collection regression checks passed (deduplication, source references, input identity, unstable inputs, conflicting evidence, legacy cold refusal, absolute GUI paths, stale-PID isolation and profile-safe disarming).'
} finally {
    if (Test-Path -LiteralPath $testRoot) {
        $resolved = (Resolve-Path -LiteralPath $testRoot).Path
        $temp = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\') + '\'
        if (-not $resolved.StartsWith($temp, [StringComparison]::OrdinalIgnoreCase) -or (Split-Path -Leaf $resolved) -notlike 'velo-latency-export-*') { throw 'Unexpected cleanup target.' }
        Remove-Item -LiteralPath $resolved -Recurse -Force
    }
}
