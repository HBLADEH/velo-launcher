#Requires -Version 7.4
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$WorkDirectory,
    [Parameter(Mandatory)][string]$OutputDirectory,
    [ValidateRange(1, 20)][int]$Count = 3,
    [ValidateRange(30, 3600)][int]$LaunchTimeoutSeconds = 600,
    [switch]$Cold,
    [string]$StartupBackupPath
)
$ErrorActionPreference = 'Stop'
$collector = Join-Path $PSScriptRoot 'collect-latency.ps1'
try {
    if ($Cold -and $Count -ne 1) { throw 'A cold observation is exactly one first launch per reboot; pass Count=1.' }
    & $collector -Action Close -WorkDirectory $WorkDirectory -OutputDirectory $OutputDirectory
    foreach ($iteration in 1..$Count) {
        $kind = $(if ($Cold) { 'ColdStartup' } else { 'WarmStartup' })
        & $collector -Action Arm -Kind $kind -WorkDirectory $WorkDirectory -OutputDirectory $OutputDirectory
        $armed = Get-Content -LiteralPath (Join-Path $WorkDirectory 'armed.json') -Raw -Encoding UTF8 | ConvertFrom-Json
        Write-Host "$kind $iteration/${Count}: open launch\Velo timing.lnk. The app will close after capture; wait for it to close before opening again."
        $watch = [Diagnostics.Stopwatch]::StartNew()
        while (-not (Test-Path -LiteralPath $armed.ControlPath)) {
            if ($watch.Elapsed.TotalSeconds -ge $LaunchTimeoutSeconds) { throw 'No GUI launch observed within the configured waiting period.' }
            Start-Sleep -Milliseconds 100
        }
        & $collector -Action Capture -WorkDirectory $WorkDirectory -OutputDirectory $OutputDirectory
        if (-not $?) { throw 'GUI startup capture failed.' }
    }
} finally {
    try {
        & $collector -Action Finish -WorkDirectory $WorkDirectory -OutputDirectory $OutputDirectory
        & $collector -Action Export -WorkDirectory $WorkDirectory -OutputDirectory $OutputDirectory
        $summary = Join-Path $OutputDirectory ('latency-summary-' + [DateTime]::UtcNow.ToString('yyyyMMdd-HHmmss') + '.json')
        & (Join-Path $PSScriptRoot 'summarize-latency.ps1') -InputPath (Join-Path $OutputDirectory 'desktop-latency.csv') -OutputPath $summary
    } finally {
        if ($StartupBackupPath) {
            $result = & (Join-Path $PSScriptRoot 'manage-latency-startup.ps1') -Action Restore -BackupPath $StartupBackupPath | ConvertFrom-Json
            if (-not $?) { throw 'Startup restoration failed; retain the backup and restore before ending the measurement session.' }
            $result | Add-Member -NotePropertyName RestoredAtUtc -NotePropertyValue ([DateTime]::UtcNow.ToString('o'))
            $result | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $OutputDirectory 'startup-restore.json') -Encoding UTF8
            $cyclePath = Join-Path $WorkDirectory 'cold-cycle.json'
            if (Test-Path -LiteralPath $cyclePath) {
                $cycle = Get-Content -LiteralPath $cyclePath -Raw -Encoding UTF8 | ConvertFrom-Json
                $cycle.State = 'StartupRestored'
                $cycle.RestoreRequired = $false
                $cycle.ReadyForReboot = $false
                $cycle | Add-Member -NotePropertyName RestoredAtUtc -NotePropertyValue $result.RestoredAtUtc -Force
                $cycle | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $cyclePath -Encoding UTF8
            }
        }
    }
}
