#Requires -Version 7.4
[CmdletBinding()]
param(
    [Parameter(Mandatory)][ValidateSet('femtovg','software')][string]$Renderer,
    [Parameter(Mandatory)][string]$OutputDirectory,
    [ValidateRange(0,100)][int]$Cycles = 100,
    [switch]$VisibleOnly
)
$ErrorActionPreference = 'Stop'
$workspace = Split-Path -Parent $PSScriptRoot
$OutputDirectory = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($OutputDirectory)
if (Test-Path -LiteralPath $OutputDirectory) { throw 'Use a fresh output directory.' }
if (@(Get-CimInstance Win32_Process -Filter "Name LIKE 'velo-slint%'").Count) { throw 'Close other Slint test windows before sampling.' }
New-Item -ItemType Directory -Path $OutputDirectory | Out-Null
$exe = Join-Path $workspace "rust/target/artifacts/$Renderer/velo-slint.exe"
$log = Join-Path $OutputDirectory 'measurement-events.log'
$info = [Diagnostics.ProcessStartInfo]::new($exe)
$info.UseShellExecute = $false
$info.CreateNoWindow = $true
$info.RedirectStandardInput = $true
$info.RedirectStandardOutput = $true
$info.RedirectStandardError = $true
$info.ArgumentList.Add('--measurement-stdio')
$info.ArgumentList.Add('--measurement-log=' + $log)
$process = [Diagnostics.Process]::Start($info)
$stderr = $process.StandardError.ReadToEndAsync()
function Command([string]$Text) {
    $process.StandardInput.WriteLine($Text)
    $process.StandardInput.Flush()
    $pending = $process.StandardOutput.ReadLineAsync()
    if (-not $pending.Wait(10000)) { throw "Slint measurement command timed out: $Text" }
    $line = $pending.Result
    if ($line -notlike "event=measurement_state command=$Text *") { throw "Unexpected measurement acknowledgment: $line" }
    if ($Text -eq 'hide' -and $line -notmatch 'visible=false presented=false') { throw 'The native window did not hide.' }
    if ($Text -eq 'show' -and $line -notmatch 'visible=true presented=true') { throw 'The native window did not show.' }
}
function Sample([string]$Name, [string]$State, [int]$Settle) {
    Command status
    Write-Output "Sampling $Renderer / $Name (settle=$Settle s, 3 x 15 s)"
    $null = & (Join-Path $PSScriptRoot 'measure-resources.ps1') -ProcessId $process.Id -ApplicationKind Slint -WindowHandle $script:hwnd `
        -ExpectedExecutable $exe -WindowState $State -MinimumStateSeconds $Settle -Seconds 15 -Samples 3 -IdleConfirmed `
        -IdleEvidence 'Fixed embedded 28-item fixture; all home rows pre-rendered; no scan or background business tasks; finite entry/hover animations settled; no screenshots during sampling.' `
        -OutputPath (Join-Path $OutputDirectory ($Name + '.json'))
    $report = Get-Content -LiteralPath (Join-Path $OutputDirectory ($Name + '.json')) -Raw | ConvertFrom-Json
    if (-not $report.AllSamplesEligible) { throw "Ineligible resource sample: $Name" }
}
try {
    Command status
    Start-Sleep -Milliseconds 1000
    $native = @(Get-Content -LiteralPath $log | Where-Object { $_ -like 'event=native_window *' }) | Select-Object -First 1
    $script:hwnd = [regex]::Match($native, 'hwnd=([0-9A-F]+)').Groups[1].Value
    if (-not $script:hwnd) { throw 'No observed native HWND.' }
    Command warm-bottom
    Start-Sleep -Milliseconds 1000
    Command warm-top
    Sample 'visible' Visible 3
    if (-not $VisibleOnly) {
        Command hide
        Sample 'hidden-30s' Hidden 30
        # The second settling period is independently observed for a full five
        # minutes, so the label is an at-least-five-minute hidden observation.
        Sample 'hidden-5m' Hidden 300
        for ($cycle = 0; $cycle -lt $Cycles; $cycle++) {
            Command show
            Start-Sleep -Milliseconds 200
            Command hide
            Start-Sleep -Milliseconds 50
        }
        Command show
        Sample 'visible-after-cycles' Visible 3
    }
    Command quit
    if (-not $process.WaitForExit(10000)) { throw 'Slint measurement shutdown timed out.' }
    $stderr.GetAwaiter().GetResult() | Set-Content -LiteralPath (Join-Path $OutputDirectory 'stderr.log') -Encoding UTF8
    [ordered]@{ Renderer = $Renderer; ProcessId = $process.Id; HWND = $script:hwnd; Cycles = $(if ($VisibleOnly) { 0 } else { $Cycles }); ExitCode = $process.ExitCode; CompletedAtUtc = [DateTime]::UtcNow.ToString('o') } |
        ConvertTo-Json | Set-Content -LiteralPath (Join-Path $OutputDirectory 'completion.json') -Encoding UTF8
    if ($process.ExitCode -ne 0) { throw 'Slint measurement process failed.' }
} finally {
    $process.StandardInput.Dispose()
    if (-not $process.HasExited -and -not $process.WaitForExit(5000)) { $process.Kill() }
    $process.Dispose()
}
