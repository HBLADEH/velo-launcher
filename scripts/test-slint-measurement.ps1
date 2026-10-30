#Requires -Version 7.4
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
. (Join-Path $PSScriptRoot 'lib/resource-sample.ps1')
function Assert($Condition, [string]$Message) { if (-not $Condition) { throw $Message } }
$testRoot = Join-Path ([IO.Path]::GetTempPath()) ('velo-slint-measurement-' + [guid]::NewGuid().ToString('N'))
try {
    New-Item -ItemType Directory -Path $testRoot | Out-Null
    $exe = Join-Path $testRoot 'velo-slint.exe'
    'synthetic executable bytes' | Set-Content -LiteralPath $exe
    $identityPath = Join-Path $testRoot 'build-identity.json'
    $identity = [pscustomobject]@{
        ExecutableSHA256 = (Get-FileHash -LiteralPath $exe).Hash; ExecutableBytes = (Get-Item -LiteralPath $exe).Length
        Renderer = 'software'; Backend = 'winit'; SlintFeatures = @('renderer-software'); WinitBackendFeatures = @('renderer-software')
        Sources = @([pscustomobject]@{ Path = 'src/fixtures.rs'; SHA256 = ('a' * 64) })
    }
    $identity | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $identityPath
    Assert ((Read-VeloRustBuildIdentity $exe).Renderer -eq 'software') 'Accept a matching single-renderer build.'
    $identity.SlintFeatures = @('renderer-software','renderer-femtovg')
    $identity | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $identityPath
    $rejected = $false
    try { $null = Read-VeloRustBuildIdentity $exe } catch { $rejected = $true }
    Assert $rejected 'Reject a build that could silently fall back to another renderer.'
    $identity.SlintFeatures = @('renderer-software')
    $identity.ExecutableBytes++
    $identity | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $identityPath
    $rejected = $false
    try { $null = Read-VeloRustBuildIdentity $exe } catch { $rejected = $true }
    Assert $rejected 'Reject mismatching executable identity.'

    # Export is exercised without launching or interacting with a real process.
    $collector = Join-Path $PSScriptRoot 'collect-slint-latency.ps1'
    $start = [DateTime]::UnixEpoch.AddSeconds(100)
    $log = Join-Path $testRoot 'fixture-events.log'
    [ordered]@{ Executable = $exe; ExecutableSHA256 = (Get-FileHash -LiteralPath $exe).Hash; EnvironmentId = 'fixture-environment'; InputId = 'fixture-input'; Method = 'ComputerUseAssistedUpperBound' } |
        ConvertTo-Json | Set-Content -LiteralPath (Join-Path $testRoot 'slint-manifest.json')
    [ordered]@{ Name = 'fixture'; ProcessId = 2147483647; StartedTicks = $start.Ticks; LogPath = $log } |
        ConvertTo-Json | Set-Content -LiteralPath (Join-Path $testRoot 'slint-active.json')
    $native = 'event=native_window pid=2147483647 unix_us=100010000 elapsed_us=1000 dropped=0 hwnd=ABCD scale=1.25'
    $input = 'event=input_accepted pid=2147483647 unix_us=100020000 elapsed_us=11000 dropped=0 utf8_bytes=1 focused=true native_focused=true visible=true composing=false'
    @($native, $input) | Set-Content -LiteralPath $log
    $null = & $collector -Action Snapshot -WorkDirectory $testRoot -OutputDirectory $testRoot
    $row = Import-Csv -LiteralPath (Join-Path $testRoot 'desktop-latency.csv')
    Assert ($row.Valid -eq 'true' -and [double]$row.DurationMilliseconds -eq 20 -and $row.Boundary -eq 'ProcessStartToEditable') 'Use actual accepted input, process creation time and the correct boundary.'
    Assert ($row.Method -eq 'ComputerUseAssistedUpperBound' -and $row.Reason -like '*not a calibrated startup*') 'Preserve the tool waiting limitation.'
    foreach ($replacement in @('native_focused=false', 'composing=true', 'dropped=1', 'pid=123')) {
        $key = $replacement.Split('=')[0]
        $bad = [regex]::Replace($input, ('\b' + $key + '=[^ ]+'), $replacement)
        @($native, $bad) | Set-Content -LiteralPath $log
        $null = & $collector -Action Snapshot -WorkDirectory $testRoot -OutputDirectory $testRoot
        $row = Import-Csv -LiteralPath (Join-Path $testRoot 'desktop-latency.csv')
        Assert ($row.Valid -eq 'false') "Reject invalid input evidence: $replacement"
    }
    $native | Set-Content -LiteralPath $log
    $null = & $collector -Action Snapshot -WorkDirectory $testRoot -OutputDirectory $testRoot
    $row = Import-Csv -LiteralPath (Join-Path $testRoot 'desktop-latency.csv')
    Assert ($row.Valid -eq 'false' -and -not $row.DurationMilliseconds) 'A window/ready marker alone must not be accepted as editable input.'
    $input | Set-Content -LiteralPath $log
    $null = & $collector -Action Snapshot -WorkDirectory $testRoot -OutputDirectory $testRoot
    $row = Import-Csv -LiteralPath (Join-Path $testRoot 'desktop-latency.csv')
    Assert ($row.Valid -eq 'false' -and -not $row.DurationMilliseconds) 'Retain a failed observation when no native window was ever observed, including under StrictMode.'
    Write-Output 'Slint measurement checks passed (build identity, feature isolation, actual input boundary, focus/composition, loss/PID rejection and missing-input failure).'
} finally {
    if (Test-Path -LiteralPath $testRoot) {
        $resolved = (Resolve-Path -LiteralPath $testRoot).Path
        $temp = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\') + '\'
        if (-not $resolved.StartsWith($temp, [StringComparison]::OrdinalIgnoreCase) -or (Split-Path -Leaf $resolved) -notlike 'velo-slint-measurement-*') { throw 'Unexpected cleanup target.' }
        Remove-Item -LiteralPath $resolved -Recurse -Force
    }
}
