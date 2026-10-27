[CmdletBinding()]
param([Parameter(Mandatory)][string]$InputPath, [string]$OutputPath)
$ErrorActionPreference = 'Stop'
if ($OutputPath -and (Test-Path -LiteralPath $OutputPath)) { throw 'OutputPath already exists; use a new filename.' }
$rows = @(Import-Csv -LiteralPath $InputPath -Encoding UTF8)
$boundaries = @{ ColdStartup = 'ProcessStartToEditable'; WarmStartup = 'ProcessStartToEditable'; HotkeyActivation = 'KeyDownToEditable' }
foreach ($row in $rows) {
    if ($row.Kind -notin $boundaries.Keys -or $row.Boundary -ne $boundaries[$row.Kind]) { throw 'Use a supported Kind and its end-to-end timing boundary.' }
    if ($row.Valid -notin @('true', 'false')) { throw 'Valid must be true or false.' }
    foreach ($field in @('EnvironmentId', 'InputId', 'Method', 'Evidence', 'TimestampUtc')) {
        if ([string]::IsNullOrWhiteSpace($row.$field)) { throw "Missing $field." }
    }
    if ($row.ExecutableSHA256 -notmatch '^[0-9a-fA-F]{64}$') { throw 'Every row requires the measured executable SHA-256.' }
    if ($row.Valid -eq 'true') {
        $duration = 0.0
        if (-not [double]::TryParse($row.DurationMilliseconds, [Globalization.NumberStyles]::Float, [Globalization.CultureInfo]::InvariantCulture, [ref]$duration) -or
            [double]::IsNaN($duration) -or [double]::IsInfinity($duration) -or $duration -le 0) { throw 'Valid samples require a positive finite duration in milliseconds.' }
        if ($row.Kind -eq 'ColdStartup' -and [string]::IsNullOrWhiteSpace($row.BootId)) { throw 'ColdStartup requires the reboot/controlled cold-condition evidence ID in BootId.' }
    }
}
$groups = @($rows | Group-Object Kind, ExecutableSHA256, EnvironmentId, InputId, Method, Boundary | ForEach-Object {
    $first = $_.Group[0]
    $valid = @($_.Group | Where-Object Valid -EQ 'true')
    if ($first.Kind -eq 'ColdStartup' -and @($valid | Group-Object BootId | Where-Object Count -GT 1).Count) { throw 'Multiple cold launches share one BootId; cached relaunches belong in WarmStartup.' }
    $values = @($valid | ForEach-Object { [double]::Parse($_.DurationMilliseconds, [Globalization.CultureInfo]::InvariantCulture) } | Sort-Object)
    $median = $null; $p95 = $null
    if ($values.Count) {
        $middle = [int][math]::Floor($values.Count / 2)
        $median = $(if ($values.Count % 2) { $values[$middle] } else { ($values[$middle - 1] + $values[$middle]) / 2 })
        # Nearest-rank P95: sorted[ceil(0.95*n)-1].
        $p95 = $values[[int][math]::Ceiling(0.95 * $values.Count) - 1]
    }
    $minimum = $(if ($first.Kind -eq 'HotkeyActivation') { 20 } else { 3 })
    [pscustomobject]@{
        Kind = $first.Kind; ExecutableSHA256 = $first.ExecutableSHA256
        EnvironmentId = $first.EnvironmentId; InputId = $first.InputId
        Method = $first.Method; Boundary = $first.Boundary
        ValidSamples = $valid.Count; InvalidSamples = $_.Count - $valid.Count
        MedianMilliseconds = $median; P95Milliseconds = $p95
        MinimumSamples = $minimum; SampleCountSatisfied = $valid.Count -ge $minimum
    }
})
$coverage = @($groups | Group-Object ExecutableSHA256, EnvironmentId, InputId, Method | ForEach-Object {
    $scenario = $_.Group
    $missing = @($boundaries.Keys | Where-Object { $_ -notin @($scenario | ForEach-Object Kind) })
    [pscustomobject]@{
        ExecutableSHA256 = $scenario[0].ExecutableSHA256
        EnvironmentId = $scenario[0].EnvironmentId; InputId = $scenario[0].InputId; Method = $scenario[0].Method
        MissingKinds = $missing
        AllSampleCountsSatisfied = $missing.Count -eq 0 -and @($scenario | Where-Object { -not $_.SampleCountSatisfied }).Count -eq 0
    }
})
$missing = @($boundaries.Keys | Where-Object { $_ -notin @($groups | ForEach-Object Kind) })
$report = [pscustomobject]@{
    SchemaVersion = 1
    InputPath = (Resolve-Path -LiteralPath $InputPath).Path
    InputSHA256 = (Get-FileHash -LiteralPath $InputPath -Algorithm SHA256).Hash
    PercentileMethod = 'Nearest rank: sorted[ceil(0.95*n)-1]; median averages the middle pair when n is even.'
    Groups = $groups
    Coverage = $coverage
    MissingKinds = $missing
    AllSampleCountsSatisfied = $coverage.Count -gt 0 -and @($coverage | Where-Object { -not $_.AllSampleCountsSatisfied }).Count -eq 0
    Note = 'Summarizes supplied desktop evidence only; does not measure latency or establish that a cold condition/input-readiness claim is true.'
}
$json = $report | ConvertTo-Json -Depth 6
if ($OutputPath) { $json | Set-Content -LiteralPath $OutputPath -Encoding UTF8 }
$json
