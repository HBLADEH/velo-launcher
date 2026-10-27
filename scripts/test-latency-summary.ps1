$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$csvPath = Join-Path ([IO.Path]::GetTempPath()) ('velo-latency-test-' + [guid]::NewGuid().ToString('N') + '.csv')
try {
    $rows = @(1..20 | ForEach-Object {
        [pscustomobject]@{
            Kind = 'HotkeyActivation'; ExecutableSHA256 = ('a' * 64); EnvironmentId = 'test-env'; InputId = 'test-input'
            Method = 'synthetic-regression-fixture'; Boundary = 'KeyDownToEditable'; DurationMilliseconds = "$_"
            Valid = 'true'; Evidence = 'fixture-only'; TimestampUtc = '2026-10-27T00:00:00Z'; BootId = ''
        }
    })
    $invalid = $rows[0].PSObject.Copy()
    $invalid.Valid = 'false'; $invalid.DurationMilliseconds = '9999'
    $other = $rows[0].PSObject.Copy()
    $other.ExecutableSHA256 = 'b' * 64
    @($rows + @($invalid, $other)) | Export-Csv -LiteralPath $csvPath -NoTypeInformation -Encoding UTF8
    $report = & (Join-Path $PSScriptRoot 'summarize-latency.ps1') -InputPath $csvPath | ConvertFrom-Json
    $group = $report.Groups | Where-Object ExecutableSHA256 -EQ ('a' * 64)
    if ($group.ValidSamples -ne 20 -or $group.InvalidSamples -ne 1 -or $group.MedianMilliseconds -ne 10.5 -or $group.P95Milliseconds -ne 19 -or -not $group.SampleCountSatisfied) { throw 'Invalid sample exclusion, median, P95 or sample count regression.' }
    if ($report.Groups.Count -ne 2 -or $report.AllSampleCountsSatisfied) { throw 'Do not mix executable hashes or treat missing startup measurements as complete.' }
    $rows[0].DurationMilliseconds = 'NaN'
    $rows | Export-Csv -LiteralPath $csvPath -NoTypeInformation -Encoding UTF8
    $rejected = $false
    try { $null = & (Join-Path $PSScriptRoot 'summarize-latency.ps1') -InputPath $csvPath } catch { $rejected = $true }
    if (-not $rejected) { throw 'Non-finite measurements must be rejected.' }
    $rows[0].DurationMilliseconds = '1'
    foreach ($row in $rows) { $row.Kind = 'ColdStartup'; $row.Boundary = 'ProcessStartToEditable'; $row.BootId = 'same-boot' }
    $rows | Export-Csv -LiteralPath $csvPath -NoTypeInformation -Encoding UTF8
    $rejected = $false
    try { $null = & (Join-Path $PSScriptRoot 'summarize-latency.ps1') -InputPath $csvPath } catch { $rejected = $true }
    if (-not $rejected) { throw 'Cached relaunches on the same boot must not become cold startup samples.' }
    $scenarioRows = @($rows | ForEach-Object { $copy = $_.PSObject.Copy(); $copy.Kind = 'HotkeyActivation'; $copy.Boundary = 'KeyDownToEditable'; $copy.ExecutableSHA256 = 'b' * 64; $copy })
    foreach ($kind in @('ColdStartup', 'WarmStartup')) {
        foreach ($n in 1..3) {
            $copy = $rows[0].PSObject.Copy(); $copy.Kind = $kind; $copy.Boundary = 'ProcessStartToEditable'; $copy.BootId = "boot-$n"
            $scenarioRows += $copy
        }
    }
    $scenarioRows | Export-Csv -LiteralPath $csvPath -NoTypeInformation -Encoding UTF8
    $report = & (Join-Path $PSScriptRoot 'summarize-latency.ps1') -InputPath $csvPath | ConvertFrom-Json
    if ($report.MissingKinds.Count -ne 0 -or $report.AllSampleCountsSatisfied -or $report.Coverage.Count -ne 2) { throw 'All kinds on different builds must not imply one complete measurement scenario.' }
    'Kind,ExecutableSHA256,EnvironmentId,InputId,Method,Boundary,DurationMilliseconds,Valid,Evidence,TimestampUtc,BootId' | Set-Content -LiteralPath $csvPath -Encoding UTF8
    $report = & (Join-Path $PSScriptRoot 'summarize-latency.ps1') -InputPath $csvPath | ConvertFrom-Json
    if ($report.Groups.Count -ne 0 -or $report.MissingKinds.Count -ne 3 -or $report.AllSampleCountsSatisfied) { throw 'An empty template must preserve absent measurements.' }
    Write-Output 'Latency summary regression checks passed (median, P95, invalid rows, separate builds, missing groups, non-finite values and cold grouping).'
} finally {
    if (Test-Path -LiteralPath $csvPath) { Remove-Item -LiteralPath $csvPath }
}
