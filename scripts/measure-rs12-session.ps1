#Requires -Version 7.4
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$SessionDirectory,
    [Parameter(Mandatory)][ValidateSet('visible', 'hidden-30s', 'hidden-5m')][string]$Phase,
    [ValidateRange(1, 3)][int]$Attempt = 1,
    [switch]$AllowScanWarnings,
    [string]$IdleEvidence
)
$ErrorActionPreference = 'Stop'
if ([string]::IsNullOrWhiteSpace($IdleEvidence)) { throw 'Explicit IdleEvidence is required: describe the observed completed scan and actual home/icon/English/Chinese warmup. A running process or ready marker is insufficient.' }
. (Join-Path $PSScriptRoot 'lib/resource-sample.ps1')
. (Join-Path $PSScriptRoot 'lib/native-observation.ps1')
$session = Read-VeloMetadataJson (Join-Path $SessionDirectory 'session.json')
$live = Get-CimInstance Win32_Process -Filter "ProcessId=$($session.ProcessId)"
if (-not $live -or $live.ExecutablePath -ne $session.Executable -or (Get-Process -Id $session.ProcessId).StartTime.ToUniversalTime().Ticks -ne $session.StartedTicks) { throw 'The recorded desktop session is no longer running.' }
$state = if ($Phase -eq 'visible') { 'Visible' } else { 'Hidden' }
$settle = switch ($Phase) { 'visible' { 3 }; 'hidden-30s' { 30 }; 'hidden-5m' { 300 } }
$arguments = @{
    ProcessId = $session.ProcessId; ApplicationKind = $(if ($session.Edition -eq 'wails') { 'Wails' } else { 'Slint' })
    DataMode = 'Business'; DataDirectory = $session.DataDirectory; ExpectedExecutable = $session.Executable
    IncludeAllDescendants = $true; WindowState = $state; MinimumStateSeconds = $settle; Seconds = 15; Samples = 3
    AllowScanWarnings = $AllowScanWarnings.IsPresent
    IdleConfirmed = $true; IdleEvidence = $IdleEvidence
    OutputPath = Join-Path $SessionDirectory ($Phase + $(if ($Attempt -gt 1) { '-attempt-' + $Attempt }) + '.json')
}
if ($session.Edition -eq 'wails') {
    $entries = @(Get-Content -LiteralPath (Join-Path $session.DataDirectory 'logs/velo.log') | ForEach-Object { try { $_ | ConvertFrom-Json -ErrorAction Stop } catch { } })
    $refresh = $entries | Where-Object { $_.msg -eq 'index refreshed' -and ([datetime]$_.time).ToUniversalTime().Ticks -ge $session.StartedTicks } | Select-Object -Last 1
    if (-not $refresh -or $refresh.error) { throw 'A successful refresh in this Wails session is required.' }
} else {
    if ($session.MeasurementLog) {
        $arguments.MeasurementLog = $session.MeasurementLog
        $native = Read-VeloEventLines $session.MeasurementLog | Where-Object { $_ -like 'event=native_window *' } | Select-Object -First 1
        $arguments.WindowHandle = [regex]::Match($native, 'hwnd=([0-9A-F]+)').Groups[1].Value
    } else {
        if (-not $session.Uninstrumented) { throw 'Missing observer log for an instrumented session.' }
        $arguments.OperatorBusinessConfirmed = $true
        $arguments.WindowHandle = Get-VeloSlintWindowHandle $session.ProcessId
    }
}
$null = & (Join-Path $PSScriptRoot 'measure-resources.ps1') @arguments
$report = Read-VeloMetadataJson $arguments.OutputPath
if (-not $report.AllSamplesEligible) { throw 'Ineligible observation retained; repeat with a fresh session.' }
Write-Output "Eligible $($session.Edition) / ${Phase}: 3 x 15-second business samples saved."
