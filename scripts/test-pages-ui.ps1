param(
    [Parameter(Mandatory)][string]$Executable,
    [Parameter(Mandatory)][string]$Helper,
    [Parameter(Mandatory)][string]$OutputDirectory,
    [string]$LegacyCopySource
)
$ErrorActionPreference = 'Stop'
$Executable = (Resolve-Path -LiteralPath $Executable).Path
$Helper = (Resolve-Path -LiteralPath $Helper).Path
$OutputDirectory = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($OutputDirectory)
if (Test-Path -LiteralPath $OutputDirectory) { throw 'Complete pages verification needs a fresh directory.' }
New-Item -ItemType Directory -Path $OutputDirectory | Out-Null
$fixture = Join-Path $OutputDirectory 'fixture'
& $Helper --prepare-ui $fixture
if ($LASTEXITCODE -ne 0) { throw 'Failed to prepare complete pages fixture.' }
$dataDirectory = Join-Path $fixture 'data'
if ($LegacyCopySource) {
    $source = (Resolve-Path -LiteralPath $LegacyCopySource).Path
    $sourceHashes = @{}
    foreach ($name in @('config.json','history.json','application-library.json','quick-launch.json','index.json')) {
        if (Test-Path -LiteralPath (Join-Path $source $name)) { $sourceHashes[$name] = (Get-FileHash -LiteralPath (Join-Path $source $name)).Hash }
    }
    # Keep all copied old fields/IDs/unknown data. Add only the synthetic helper
    # directory needed for repeatable callback checks, in the disposable copy.
    $nativeHelper = Join-Path (Split-Path -Parent $Helper) 'rs11_windows.exe'
    & $nativeHelper --prepare-copy $source $dataDirectory (Join-Path $fixture 'apps')
    if ($LASTEXITCODE -ne 0) { throw 'Failed to create an isolated old data copy.' }
}
$manual = Join-Path $fixture 'manual'
New-Item -ItemType Directory -Path $manual | Out-Null
Copy-Item -LiteralPath $Helper -Destination (Join-Path $manual 'RS11 自定义应用.exe')
$instance = 'rs11-ui-' + [guid]::NewGuid().ToString('N')
foreach ($mode in @('pages-smoke-test','pages-restart-test')) {
    if ($mode -eq 'pages-restart-test') {
        # Simulate a pinned application disappearing inside this disposable fixture.
        $shortcut = Join-Path $fixture 'apps/RS10 启动验证.lnk'
        Move-Item -LiteralPath $shortcut -Destination "$shortcut.offline"
    }
    $log = Join-Path $OutputDirectory ($mode + '.stderr.log')
    $process = Start-Process -FilePath $Executable -ArgumentList "--$mode", '--diagnostics',
        "--instance-id=$instance", ('--data-dir="' + $dataDirectory + '"'), '--hotkey=Ctrl+Alt+Shift+F11', '--query="RS10 启动验证"' `
        -WindowStyle Hidden -PassThru -RedirectStandardError $log
    try {
        $deadline = [DateTime]::UtcNow.AddSeconds(100)
        while (-not $process.WaitForExit(1000)) {
            if ([DateTime]::UtcNow -gt $deadline) { throw 'Complete pages verification timed out.' }
        }
        $process.Refresh()
        $text = Get-Content -LiteralPath $log -Raw
        Write-Output $text
        if ($process.ExitCode -ne 0 -or $text -notmatch 'event=pages_smoke_result passed=true' -or $text -notmatch 'event=desktop_stopped tray_removed=true') { throw "Complete pages verification failed (exit $($process.ExitCode))." }
    } finally { if (-not $process.HasExited) { Stop-Process -Id $process.Id -ErrorAction SilentlyContinue } }
}
$saved = Get-Content -LiteralPath (Join-Path $dataDirectory 'config.json') -Raw | ConvertFrom-Json
if ($saved.result_layout -ne 'grid' -or $saved.max_results -ne 20 -or $saved.auto_check_updates) { throw 'Settings did not survive restart or an unsaved draft was persisted.' }
if (-not (Test-Path -LiteralPath (Join-Path $dataDirectory 'backup-created.png'))) { throw 'Missing backup screenshot.' }
if (-not (Test-Path -LiteralPath (Join-Path $manual 'RS11 自定义应用.exe'))) { throw 'Removing the custom record changed the source file.' }
Write-Output 'RS-11 settings, grid, import, pin, backup, restart and confirmation integration passed.'
if ($LegacyCopySource) {
    foreach ($name in $sourceHashes.Keys) {
        if ((Get-FileHash -LiteralPath (Join-Path $source $name)).Hash -ne $sourceHashes[$name]) { throw 'Original old data changed during copy verification.' }
    }
    Write-Output 'RS-11 old data copy passed; all original source file hashes are unchanged.'
}
