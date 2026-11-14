#Requires -Version 7.4
[CmdletBinding()]
param([Parameter(Mandatory)][string]$SessionDirectory)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'lib/resource-sample.ps1')
$session = Read-VeloMetadataJson (Join-Path $SessionDirectory 'session.json')
$live = Get-CimInstance Win32_Process -Filter "ProcessId=$($session.ProcessId)"
if (-not $live -or $live.ExecutablePath -ne $session.Executable -or
    (Get-Process -Id $session.ProcessId).StartTime.ToUniversalTime().Ticks -ne $session.StartedTicks) { throw 'Desktop process identity changed.' }
$arguments = @{
    ProcessId = $session.ProcessId; ApplicationKind = $(if ($session.Edition -eq 'wails') { 'Wails' } else { 'Slint' })
    DataMode = 'Business'; DataDirectory = $session.DataDirectory; ExpectedExecutable = $session.Executable
    IncludeAllDescendants = $true; WindowState = 'Unknown'; Seconds = 60
}
if ($session.Edition -ne 'wails') {
    $arguments.MeasurementLog = $session.MeasurementLog
    $native = Read-VeloEventLines $session.MeasurementLog | Where-Object { $_ -like 'event=native_window *' } | Select-Object -First 1
    $arguments.WindowHandle = [regex]::Match($native, 'hwnd=([0-9A-F]+)').Groups[1].Value
}
# Read-only operation/idle observations may include visibility transitions and
# operator input. Do not supply IdleConfirmed or turn them into idle baselines.
foreach ($chunk in @(@{Name='operation-idle-20m'; Samples=20}, @{Name='operation-idle-10m'; Samples=10})) {
    Write-Output "Observing $($session.Edition) / $($chunk.Name); normal desktop interaction remains enabled."
    $null = & (Join-Path $PSScriptRoot 'measure-resources.ps1') @arguments -Samples $chunk.Samples -OutputPath (Join-Path $SessionDirectory ($chunk.Name + '.json'))
}
$reports = @('operation-idle-20m', 'operation-idle-10m' | ForEach-Object { Read-VeloMetadataJson (Join-Path $SessionDirectory ($_ + '.json')) })
$samples = @($reports | ForEach-Object Samples)
$duration = ($samples.SampleSeconds | Measure-Object -Sum).Sum
if ($samples.Count -ne 30 -or $duration -lt 1800 -or @($reports | Where-Object { -not $_.ExecutableFileStable }).Count) { throw 'Operation/idle observation is incomplete.' }
[ordered]@{ Completed = $true; SHA256 = $session.SHA256; Samples = $samples.Count; ObservedSampleSeconds = $duration; Scope = 'Read-only one-second polls in 30 one-minute samples; includes operator activity and idle. Not an idle baseline or physical-key count/latency proof.'; EvidenceSHA256 = @('operation-idle-20m', 'operation-idle-10m' | ForEach-Object { (Get-FileHash -LiteralPath (Join-Path $SessionDirectory ($_ + '.json'))).Hash }) } |
    ConvertTo-Json | Set-Content -LiteralPath (Join-Path $SessionDirectory 'operation-idle-completion.json')
Write-Output 'Thirty-minute read-only desktop operation/idle observation saved.'
