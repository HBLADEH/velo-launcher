$ErrorActionPreference = 'Stop'
$testRoot = Join-Path ([IO.Path]::GetTempPath()) ('velo-startup-cleanup-' + [guid]::NewGuid().ToString('N'))
try {
    New-Item -ItemType Directory -Path $testRoot | Out-Null
    Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'collect-gui-startups.ps1') -Destination (Join-Path $testRoot 'collect-gui-startups.ps1')
    @'
param($Action, $Kind, $WorkDirectory, $OutputDirectory)
if ($Action -eq 'Arm') { throw 'fixture capture failure' }
if ($Action -eq 'Finish') { throw 'fixture cleanup failure' }
'@ | Set-Content -LiteralPath (Join-Path $testRoot 'collect-latency.ps1') -Encoding UTF8
    @'
param($Action, $BackupPath)
if ($Action -ne 'Restore') { throw 'fixture received an unexpected startup action' }
$BackupPath | Set-Content -LiteralPath (Join-Path $PSScriptRoot 'restore-called.txt') -Encoding UTF8
'{"Action":"Restore","StartupValuePresent":true}'
'@ | Set-Content -LiteralPath (Join-Path $testRoot 'manage-latency-startup.ps1') -Encoding UTF8
    $failed = $false
    try {
        & (Join-Path $testRoot 'collect-gui-startups.ps1') -Cold -Count 1 -WorkDirectory $testRoot -OutputDirectory $testRoot -StartupBackupPath 'fixture-backup.json'
    } catch { $failed = $_.Exception.Message -eq 'fixture cleanup failure' }
    if (-not $failed -or (Get-Content -LiteralPath (Join-Path $testRoot 'restore-called.txt') -Raw).Trim() -ne 'fixture-backup.json' -or
        -not (Test-Path -LiteralPath (Join-Path $testRoot 'startup-restore.json'))) {
        throw 'Startup restoration must run even when capture and ordinary cleanup both fail.'
    }
    'Startup cleanup regression passed: capture and cleanup failures still run the restoration step; no real registry or app was used.'
} finally {
    if (Test-Path -LiteralPath $testRoot) {
        $resolved = (Resolve-Path -LiteralPath $testRoot).Path
        $temp = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\') + '\'
        if (-not $resolved.StartsWith($temp, [StringComparison]::OrdinalIgnoreCase) -or (Split-Path -Leaf $resolved) -notlike 'velo-startup-cleanup-*') { throw 'Unexpected cleanup target.' }
        Remove-Item -LiteralPath $resolved -Recurse -Force
    }
}
