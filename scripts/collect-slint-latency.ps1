#Requires -Version 7.4
[CmdletBinding()]
param(
    [Parameter(Mandatory)][ValidateSet('Arm', 'Capture', 'Snapshot', 'Export')][string]$Action,
    [Parameter(Mandatory)][string]$WorkDirectory,
    [Parameter(Mandatory)][string]$OutputDirectory,
    [string]$Executable
)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'lib/resource-sample.ps1')
$WorkDirectory = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($WorkDirectory)
$OutputDirectory = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($OutputDirectory)
$manifestPath = Join-Path $WorkDirectory 'slint-manifest.json'
function Hash-Value($Value) {
    [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes(($Value | ConvertTo-Json -Depth 12 -Compress))))
}
function Read-Events([string]$Path) {
    @(Get-Content -LiteralPath $Path -Encoding UTF8 | ForEach-Object {
        $fields = [ordered]@{}
        foreach ($match in [regex]::Matches($_, '(\w+)=([^ ]+)')) { $fields[$match.Groups[1].Value] = $match.Groups[2].Value }
        [pscustomobject]$fields
    })
}
if ($Action -eq 'Arm') {
    if (Test-Path -LiteralPath $manifestPath) { throw 'Use a fresh WorkDirectory; do not overwrite a prior Slint session.' }
    if (-not $Executable) { throw 'Slint Arm requires Executable.' }
    $Executable = (Resolve-Path -LiteralPath $Executable).Path
    $identity = Read-VeloRustBuildIdentity -Executable $Executable
    New-Item -ItemType Directory -Path $WorkDirectory, $OutputDirectory -Force | Out-Null
    $os = Get-CimInstance Win32_OperatingSystem
    $environment = [ordered]@{ Windows = $os.Version; Build = $os.BuildNumber; Boot = $os.LastBootUpTime.ToUniversalTime().ToString('o'); CPU = @((Get-CimInstance Win32_Processor).Name) }
    $inputIdentity = @($identity.Sources | Where-Object { $_.Path -eq 'src/fixtures.rs' -or $_.Path -like 'ui/assets/*' })
    [ordered]@{
        Executable = $Executable; ExecutableSHA256 = $identity.ExecutableSHA256; BuildIdentity = $identity
        Environment = $environment; EnvironmentId = (Hash-Value $environment); InputId = (Hash-Value $inputIdentity)
        Method = 'ComputerUseAssistedUpperBound'; Kind = 'WarmStartup'
        Note = 'One Capture starts one process. Use Computer Use to enter a character without clicking the search box; close the window, then Snapshot. The duration includes operator/tool waiting and is only an observed upper bound.'
    } | ConvertTo-Json -Depth 12 | Set-Content -LiteralPath $manifestPath -Encoding UTF8
    Get-Content -LiteralPath $manifestPath -Raw
    return
}
$manifest = Read-VeloMetadataJson $manifestPath
if ($manifest.ExecutableSHA256 -ne (Get-FileHash -LiteralPath $manifest.Executable).Hash) { throw 'Frozen Slint executable changed.' }
if ($Action -eq 'Capture') {
    if (@(Get-CimInstance Win32_Process -Filter "Name LIKE 'velo-slint%'").Count) { throw 'Close the previous Slint test window before starting another capture.' }
    $name = 'warm-' + [DateTime]::UtcNow.ToString('yyyyMMddTHHmmssfffffff')
    $logPath = Join-Path $OutputDirectory ($name + '-events.log')
    $process = Start-Process -FilePath $manifest.Executable -ArgumentList '--isolated', ('--measurement-log="' + $logPath + '"') -WindowStyle Hidden -PassThru
    [ordered]@{ Name = $name; ProcessId = $process.Id; StartedTicks = $process.StartTime.ToUniversalTime().Ticks; LogPath = $logPath } |
        ConvertTo-Json | Set-Content -LiteralPath (Join-Path $WorkDirectory 'slint-active.json') -Encoding UTF8
    [pscustomobject]@{ ProcessId = $process.Id; LogPath = $logPath; Pending = 'Actual search-box input, then normal window close and Snapshot.' }
    return
}
if ($Action -eq 'Snapshot') {
    $active = Read-VeloMetadataJson (Join-Path $WorkDirectory 'slint-active.json')
    $current = Get-Process -Id $active.ProcessId -ErrorAction SilentlyContinue
    if ($current -and $current.StartTime.ToUniversalTime().Ticks -eq $active.StartedTicks) { throw 'Close the measurement window first so evidence is no longer changing.' }
    $events = @(Read-Events $active.LogPath)
    $accepted = @($events | Where-Object event -EQ 'input_accepted') | Select-Object -First 1
    $native = @($events | Where-Object event -EQ 'native_window') | Select-Object -First 1
    $valid = $null -ne $accepted -and $null -ne $native -and $accepted.focused -eq 'true' -and $accepted.native_focused -eq 'true' -and
        $accepted.visible -eq 'true' -and $accepted.composing -eq 'false' -and @($events | Where-Object { $_.pid -ne [string]$active.ProcessId -or $_.dropped -ne '0' }).Count -eq 0
    $at = $(if ($accepted) { [DateTime]::UnixEpoch.AddTicks([long]$accepted.unix_us * 10) } else { [DateTime]::UtcNow })
    $duration = $(if ($valid) { ($at.Ticks - $active.StartedTicks) / 10000.0 } else { $null })
    if ($duration -le 0) { $valid = $false; $duration = $null }
    $stages = @($events | Where-Object { $_.event -in @('main_started','component_created','native_window','first_redraw_requested','first_opengl_frame','show_returned','input_focus','native_focus','input_accepted') })
    [ordered]@{
        Kind = 'WarmStartup'; Boundary = 'ProcessStartToEditable'; Valid = $valid.ToString().ToLowerInvariant()
        DurationMilliseconds = $duration; TimestampUtc = $at.ToString('o'); BootId = ''
        ExecutableSHA256 = $manifest.ExecutableSHA256; EnvironmentId = (Hash-Value @($manifest.EnvironmentId, $(if ($native) { $native.scale } else { 'unobserved' })))
        InputId = $manifest.InputId; Method = $manifest.Method; Evidence = (Split-Path -Leaf $active.LogPath)
        ProcessId = $active.ProcessId; Reason = $(if ($valid) { 'Upper bound includes Computer Use activation and waiting; not a calibrated startup benchmark.' } else { 'No actual non-composing character accepted with native and component focus, or evidence was incomplete.' })
        Stages = $stages
    } | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $OutputDirectory ($active.Name + '-observation.json')) -Encoding UTF8
}
if ($Action -in @('Snapshot', 'Export')) {
    $rows = @(Get-ChildItem -LiteralPath $OutputDirectory -Filter '*-observation.json' | ForEach-Object { Read-VeloMetadataJson $_.FullName })
    if (-not $rows.Count) { throw 'No Slint input observations to export.' }
    $rows | Select-Object Kind, Boundary, Valid, DurationMilliseconds, TimestampUtc, BootId, ExecutableSHA256, EnvironmentId, InputId, Method, Evidence, ProcessId, Reason |
        Export-Csv -LiteralPath (Join-Path $OutputDirectory 'desktop-latency.csv') -NoTypeInformation -Encoding UTF8
    $rows | Select-Object ProcessId, Valid, DurationMilliseconds, Reason
}
