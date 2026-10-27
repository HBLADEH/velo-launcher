#Requires -Version 7.4
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$WorkDirectory,
    [Parameter(Mandatory)][string]$OutputDirectory,
    [switch]$Cold,
    [switch]$HotkeysOnly,
    [ValidateRange(1, 50)][int]$HotkeySamples = 20
)
$ErrorActionPreference = 'Stop'
if ($Cold -and $HotkeysOnly) { throw 'Cold and HotkeysOnly are separate collection modes.' }
$collector = Join-Path $PSScriptRoot 'collect-latency.ps1'
function Call-Probe($Control, [string]$Route, [string]$Method = 'Get') {
    Invoke-RestMethod -Uri ($Control.URL + $Route) -Method $Method -Headers @{ Authorization = 'Bearer ' + $Control.Token }
}
try {
    if (-not $HotkeysOnly) { & $collector -Action Close -WorkDirectory $WorkDirectory -OutputDirectory $OutputDirectory }
    if ($Cold) {
        # Arm performs the reboot/first-launch checks without hashing the binary
        # or reading application input files before the measured cold launch.
        & $collector -Action Arm -Kind ColdStartup -WorkDirectory $WorkDirectory -OutputDirectory $OutputDirectory
        if (-not $?) { throw 'Cold condition was rejected.' }
        Write-Host 'Open launch\Velo timing.lnk from the prepared working directory, then return here.'
        $null = Read-Host 'Press Enter after opening the shortcut'
        & $collector -Action Capture -WorkDirectory $WorkDirectory -OutputDirectory $OutputDirectory
    } else {
        Write-Host 'Run this script in your normal interactive desktop terminal.'
        Write-Host "It will wait for $HotkeySamples physical Alt+Space presses."
        if (-not $HotkeysOnly) { & $collector -Action Warm -Count 3 -KeepLastRunning -WorkDirectory $WorkDirectory -OutputDirectory $OutputDirectory }
        $active = Get-Content -LiteralPath (Join-Path $WorkDirectory 'active.json') -Raw -Encoding UTF8 | ConvertFrom-Json
        $control = Get-Content -LiteralPath $active.ControlPath -Raw -Encoding UTF8 | ConvertFrom-Json
        $scanWatch = [Diagnostics.Stopwatch]::StartNew()
        do {
            $state = Call-Probe $control '/state'
            if (-not $state.Status.scanning) { break }
            Start-Sleep -Milliseconds 100
        } while ($scanWatch.Elapsed.TotalSeconds -lt 60)
        if ($state.Status.scanning) { throw 'Wait for indexing to finish before measuring hotkey activations.' }
        $acceptedWarm = 0
        foreach ($contextFile in (Get-ChildItem -LiteralPath $OutputDirectory -Filter 'warm-*-state.json')) {
            $warmState = Get-Content -LiteralPath $contextFile.FullName -Raw -Encoding UTF8 | ConvertFrom-Json
            $acceptedWarm += @($warmState.Records | Where-Object { $_.Kind -eq 'WarmStartup' -and $_.Valid }).Count
        }
        if (-not $HotkeysOnly -and $acceptedWarm -lt 3) { Write-Warning 'Warm startup has fewer than three confirmed samples; the final report will keep it incomplete.' }
        $success = 0
        $timeouts = 0
        foreach ($attempt in 1..60) {
            if ($success -ge $HotkeySamples) { break }
            $id = 'physical-' + $attempt.ToString('00')
            $null = Call-Probe $control ('/prepare?id=' + $id) Post
            Start-Sleep -Seconds 2
            Write-Host "Alt+Space: valid $success/$HotkeySamples, attempt $attempt. Do not press Enter or launch a result."
            $watch = [Diagnostics.Stopwatch]::StartNew()
            do {
                Start-Sleep -Milliseconds 50
                $state = Call-Probe $control '/state'
                $record = $state.Records | Where-Object ID -EQ $id | Select-Object -Last 1
            } until ($record -or $watch.Elapsed.TotalSeconds -ge 30)
            if (-not $record) {
                $null = Call-Probe $control '/expire' Post
                Write-Warning "Attempt $attempt timed out; the failure is preserved."
                $timeouts++
                if ($timeouts -ge 3) { throw 'No key arrived in three consecutive observation windows; rerun when ready.' }
            } elseif ($record.Valid -and -not $record.InjectedKeyboardEvent) {
                $success++
                $timeouts = 0
                Write-Host ("Upper bound: {0:N3} ms; injected keyboard={1}" -f $record.UpperBoundMilliseconds, $record.InjectedKeyboardEvent)
            } else {
                Write-Warning $(if ($record.InjectedKeyboardEvent) { 'An injected key cannot count toward this physical-key run.' } else { $record.Reason })
            }
            Start-Sleep -Milliseconds 750
        }
        if ($success -lt $HotkeySamples) { throw 'Fewer than the requested confirmed activations; preserve failures and investigate before accepting the data.' }
    }
} finally {
    & $collector -Action Finish -WorkDirectory $WorkDirectory -OutputDirectory $OutputDirectory
    & $collector -Action Export -WorkDirectory $WorkDirectory -OutputDirectory $OutputDirectory
    $summary = Join-Path $OutputDirectory ('latency-summary-' + [DateTime]::UtcNow.ToString('yyyyMMdd-HHmmss') + '.json')
    & (Join-Path $PSScriptRoot 'summarize-latency.ps1') -InputPath (Join-Path $OutputDirectory 'desktop-latency.csv') -OutputPath $summary
}
