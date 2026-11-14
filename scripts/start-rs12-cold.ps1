#Requires -Version 7.4
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$CaseDirectory,
    [Parameter(Mandatory)][switch]$FirstVeloLaunchThisBootConfirmed
)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'lib/resource-sample.ps1')
$CaseDirectory = (Resolve-Path -LiteralPath $CaseDirectory).Path
$case = Read-VeloMetadataJson (Join-Path $CaseDirectory 'case.json')
$boot = (Get-CimInstance Win32_OperatingSystem).LastBootUpTime.ToUniversalTime()
if (([DateTime]$case.PreparedAtUtc).ToUniversalTime() -ge $boot) { throw 'This case was prepared during this boot. Reboot independently before using it as a cold condition.' }
if (-not $FirstVeloLaunchThisBootConfirmed) { throw 'An operator must confirm that no Velo candidate ran earlier this boot.' }
if (@(Get-CimInstance Win32_Process -Filter "Name LIKE 'velo-launcher%' OR Name LIKE 'velo-slint%'").Count) { throw 'Another Velo process is running; this cold condition is unusable.' }
$started = Join-Path $CaseDirectory 'started.json'
if (Test-Path -LiteralPath $started) { throw 'This case has already been attempted; keep its result and prepare another case.' }
# All prepared sets in this workspace share one boot ledger. Preparing another
# set must not allow a second candidate to be labelled cold in the same boot.
$root = Join-Path (Split-Path -Parent $PSScriptRoot) '.reasonix/rs12-cold-boot-ledger'
New-Item -ItemType Directory -Path $root -Force | Out-Null
$bootID = $boot.Ticks.ToString()
$claim = Join-Path $root ("boot-$bootID.json")
# Create-new excludes a second candidate across every case in this prepared set.
$claimFile = [IO.FileStream]::new($claim, [IO.FileMode]::CreateNew, [IO.FileAccess]::Write, [IO.FileShare]::Read)
try {
    $bytes = [Text.Encoding]::UTF8.GetBytes((@{ BootUtc = $boot.ToString('o'); Case = $case.Case; OperatorConfirmedFirstLaunch = $true } | ConvertTo-Json))
    $claimFile.Write($bytes)
} finally { $claimFile.Dispose() }
$info = [Diagnostics.ProcessStartInfo]::new($case.Executable)
$info.UseShellExecute = $false
$info.CreateNoWindow = $true
$info.WindowStyle = 'Hidden'
if ($case.Edition -eq 'wails') {
    $info.Environment['LOCALAPPDATA'] = $case.LocalAppData
    if ($case.PSObject.Properties['ActualInputProbe'] -and $case.ActualInputProbe) {
        $info.Environment['VELO_LATENCY_DIR'] = $case.ProbeDirectory
        $info.Environment['VELO_LATENCY_ACTUAL_INPUT'] = '1'
        $info.Environment['VELO_LATENCY_KIND'] = 'ColdStartup'
        $info.Environment['VELO_LATENCY_SHA256'] = $case.SHA256
        $info.Environment['VELO_LATENCY_BOOT_ID'] = $bootID
    }
}
else {
    foreach ($argument in @(('--instance-id=' + $case.InstanceId), ('--data-dir=' + $case.DataDirectory), '--measurement-business', ('--measurement-log=' + $case.MeasurementLog))) { $info.ArgumentList.Add($argument) }
}
# Do not hash/map the binary or read application data before its measured launch.
$process = [Diagnostics.Process]::Start($info)
[ordered]@{ Case = $case.Case; Edition = $case.Edition; ProcessId = $process.Id; StartedTicks = $process.StartTime.ToUniversalTime().Ticks; BootUtc = $boot.ToString('o'); BootID = $bootID; OperatorConfirmedFirstLaunch = $true; DurationMilliseconds = $null; Valid = $false; Reason = 'Pending actual accepted character and independent cold-condition/calibration evidence.' } |
    ConvertTo-Json | Set-Content -LiteralPath $started -Encoding utf8NoBOM
Write-Output 'Enter a character in the actual search box, preserve the desktop timing evidence, then exit normally. No input is synthesized.'
while (-not $process.WaitForExit(1000)) { }
[ordered]@{ Case = $case.Case; ExitCode = $process.ExitCode; SHA256 = (Get-FileHash -LiteralPath $case.Executable).Hash; MatchesPreparedSHA256 = (Get-FileHash -LiteralPath $case.Executable).Hash -eq $case.SHA256; ValidLatencySample = $false; Note = 'Post-exit identity only. Actual input timing, cold preflight and method uncertainty must be reviewed separately.' } |
    ConvertTo-Json | Set-Content -LiteralPath (Join-Path $CaseDirectory 'exit.json') -Encoding utf8NoBOM
$process.Dispose()
