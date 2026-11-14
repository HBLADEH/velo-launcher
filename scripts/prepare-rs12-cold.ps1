#Requires -Version 7.4
[CmdletBinding()]
param([Parameter(Mandatory)][string]$PreparedDirectory, [Parameter(Mandatory)][string]$OutputDirectory)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'lib/resource-sample.ps1')
$PreparedDirectory = (Resolve-Path -LiteralPath $PreparedDirectory).Path
$OutputDirectory = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($OutputDirectory)
if (Test-Path -LiteralPath $OutputDirectory) { throw 'Use a new cold-case directory; earlier cases must not be overwritten.' }
$manifest = Read-VeloMetadataJson (Join-Path $PreparedDirectory 'manifest.json')
$actualInput = [bool]$manifest.PSObject.Properties['SeedDirectory']
$seed = if ($actualInput) { $manifest.SeedDirectory } else { Join-Path $PreparedDirectory 'seed' }
New-Item -ItemType Directory -Path $OutputDirectory | Out-Null
foreach ($round in 1..3) {
    # Every entry below requires its own later boot, even within the same round.
    foreach ($edition in @('wails', 'software', 'femtovg')) {
        $name = "$edition-$round"
        $case = Join-Path $OutputDirectory $name
        New-Item -ItemType Directory -Path (Join-Path $case 'local') -Force | Out-Null
        $useActualProbe = $actualInput
        $sourceExe = if ($useActualProbe) { $manifest.Builds.$edition.Executable } else { Join-Path $PreparedDirectory "binaries/$edition/$(if ($edition -eq 'wails') { 'velo-launcher.exe' } else { 'velo-slint.exe' })" }
        Copy-Item -LiteralPath (Split-Path -Parent $sourceExe) -Destination (Join-Path $case 'binary') -Recurse
        Copy-Item -LiteralPath $seed -Destination (Join-Path $case 'local/Velo') -Recurse
        $exe = Join-Path $case ('binary/' + [IO.Path]::GetFileName($sourceExe))
        $sha = if ($useActualProbe) { $manifest.Builds.$edition.SHA256 } elseif ($edition -eq 'wails') { $manifest.Builds.wails.SHA256 } else { $manifest.Builds.$edition.ExecutableSHA256 }
        if ((Get-FileHash -LiteralPath $exe).Hash -ne $sha) { throw 'Cold-case binary copy mismatch.' }
        $inputs = [ordered]@{}
        foreach ($file in 'config.json','index.json','history.json','application-library.json') { $inputs[$file] = (Get-FileHash -LiteralPath (Join-Path $case ('local/Velo/' + $file))).Hash }
        [ordered]@{
            Case = $name; Edition = $edition; PreparedAtUtc = [DateTime]::UtcNow.ToString('o'); SHA256 = $sha
            Executable = $exe; DataDirectory = Join-Path $case 'local/Velo'; LocalAppData = Join-Path $case 'local'
            InstanceId = 'rs12-cold-' + $name; MeasurementLog = $(if ($useActualProbe -and $edition -eq 'wails') { Join-Path $case 'probe/accepted-input.log' } else { Join-Path $case 'measurement-events.log' })
            ActualInputProbe = $useActualProbe; ProbeDirectory = Join-Path $case 'probe'
            ProbeNote = $(if ($edition -eq 'wails' -and -not $useActualProbe) { 'Official Wails reference does not log accepted input; this case can check process/window presence but never yields a latency-valid cold sample.' } else { $null })
            InputId = [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes(($inputs | ConvertTo-Json -Compress))))
            Scope = 'Prepared before reboot. One case per independent boot. Launcher only captures actual process creation/input observations; it does not establish cold conditions or calibrate physical latency by itself.'
        } | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $case 'case.json') -Encoding utf8NoBOM
    }
}
Write-Output "Nine isolated future cold cases prepared in $OutputDirectory. No reboot or login setting was changed."
