param(
    [Parameter(Mandatory)][string]$Executable,
    [Parameter(Mandatory)][string]$Helper,
    [Parameter(Mandatory)][string]$OutputDirectory
)
$ErrorActionPreference = 'Stop'
$Executable = (Resolve-Path -LiteralPath $Executable).Path
$Helper = (Resolve-Path -LiteralPath $Helper).Path
$OutputDirectory = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($OutputDirectory)
if (Test-Path -LiteralPath $OutputDirectory) { throw 'Business UI verification needs a fresh directory.' }
New-Item -ItemType Directory -Path $OutputDirectory | Out-Null
$fixture = Join-Path $OutputDirectory 'fixture'
& $Helper --prepare-ui $fixture
if ($LASTEXITCODE -ne 0) { throw 'Failed to create synthetic business UI fixture.' }
$log = Join-Path $OutputDirectory 'business.stderr.log'
$instance = 'rs10-ui-' + [guid]::NewGuid().ToString('N')
$process = Start-Process -FilePath $Executable -ArgumentList '--business-smoke-test', '--diagnostics', '--reduce-motion',
    "--instance-id=$instance", ('--data-dir="' + (Join-Path $fixture 'data') + '"'), '--hotkey=Ctrl+Alt+Shift+F11', '--query="RS10 启动验证"' `
    -WindowStyle Hidden -PassThru -RedirectStandardError $log
try {
    if (-not $process.WaitForExit(60000)) { throw 'Business UI verification timed out.' }
    $process.Refresh()
    $text = Get-Content -LiteralPath $log -Raw
    Write-Output $text
    if ($process.ExitCode -ne 0 -or $text -notmatch 'event=business_smoke_result passed=true' -or
        $text -notmatch 'event=lifecycle_quit' -or $text -notmatch 'event=desktop_stopped tray_removed=true') {
        throw "Business UI verification failed (exit $($process.ExitCode))."
    }
    foreach ($name in @('business-light.png', 'business-dark.png')) {
        if (-not (Test-Path -LiteralPath (Join-Path $fixture "data/$name"))) { throw "Missing business snapshot: $name" }
    }
} finally {
    if (-not $process.HasExited) { Stop-Process -Id $process.Id -ErrorAction SilentlyContinue }
}
