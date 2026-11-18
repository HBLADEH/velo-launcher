#Requires -Version 7.4
param(
    [Parameter(Mandatory)][string]$Executable,
    [Parameter(Mandatory)][string]$EverythingExecutable,
    [Parameter(Mandatory)][string]$OutputDirectory
)
$ErrorActionPreference = 'Stop'
$Executable = (Resolve-Path -LiteralPath $Executable).Path
$EverythingExecutable = (Resolve-Path -LiteralPath $EverythingExecutable).Path
$OutputDirectory = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($OutputDirectory)
if (Test-Path -LiteralPath $OutputDirectory) { throw 'Use a fresh Everything verification directory.' }
# Refuse to change or exit an existing default Everything client.
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class VeloEverythingEndpointProbe {
    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    public static extern IntPtr FindWindow(string className, string title);
}
'@
if ([VeloEverythingEndpointProbe]::FindWindow('EVERYTHING_TASKBAR_NOTIFICATION', $null) -ne [IntPtr]::Zero) {
    throw 'A default Everything client is already running. This fixture requires an unused default endpoint.'
}
New-Item -ItemType Directory -Path $OutputDirectory | Out-Null
$copiedExecutable = Join-Path $OutputDirectory 'velo-under-test.exe'
Copy-Item -LiteralPath $Executable -Destination $copiedExecutable
$Executable = $copiedExecutable
$fixtures = Join-Path $OutputDirectory 'synthetic-files'
New-Item -ItemType Directory -Path $fixtures | Out-Null
New-Item -ItemType Directory -Path (Join-Path $fixtures '合同归档') | Out-Null
foreach ($index in 1..125) {
    [IO.File]::WriteAllText((Join-Path $fixtures ('Velo文件验证-{0:D3} 合同.txt' -f $index)), 'Synthetic Velo test file.')
}
$ini = Join-Path $OutputDirectory 'Everything.ini'
@(
    '[Everything]', 'app_data=0', 'run_as_admin=0', 'run_in_background=1',
    'show_tray_icon=0', 'check_for_updates_on_startup=0',
    'auto_include_fixed_volumes=0', 'auto_include_removable_volumes=0',
    'ntfs_volume_guids=', 'ntfs_volume_paths=', 'ntfs_volume_includes=',
    ('folders=' + $fixtures), 'folder_update_types=0', 'folder_monitor_changes=0'
) | Set-Content -LiteralPath $ini -Encoding utf8NoBOM
$data = Join-Path $OutputDirectory 'data'
New-Item -ItemType Directory -Path $data | Out-Null
@{
    version = 1; hotkey = 'Ctrl+Alt+F9'; theme = 'light'; space_launch = $true
    scan_program_files = $false; auto_check_updates = $false; refresh_minutes = 1440
    everything = @{ enabled = $true; executable = $EverythingExecutable }
} | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $data 'config.json') -Encoding utf8NoBOM
$engine = $null
$launcher = $null
$samples = [Collections.Generic.List[object]]::new()
$log = Join-Path $OutputDirectory 'component.stderr.log'
try {
    $engine = Start-Process -FilePath $EverythingExecutable -ArgumentList @('-startup','-nodb','-config',('"' + $ini + '"')) -WindowStyle Hidden -PassThru
    $readyDeadline = [DateTime]::UtcNow.AddSeconds(15)
    while ([VeloEverythingEndpointProbe]::FindWindow('EVERYTHING_TASKBAR_NOTIFICATION', $null) -eq [IntPtr]::Zero) {
        if ($engine.HasExited -or [DateTime]::UtcNow -gt $readyDeadline) { throw 'Isolated Everything endpoint did not become ready.' }
        Start-Sleep -Milliseconds 100
    }
    $instance = 'everything-ui-' + [guid]::NewGuid().ToString('N')
    $launcher = Start-Process -FilePath $Executable -ArgumentList @(
        '--everything-smoke-test', '--reduce-motion', '--diagnostics', '--hotkey=Ctrl+Alt+F9',
        ('--instance-id=' + $instance), ('--data-dir="' + $data + '"')
    ) -WindowStyle Hidden -PassThru -RedirectStandardError $log
    $deadline = [DateTime]::UtcNow.AddSeconds(90)
    while (-not $launcher.WaitForExit(200)) {
        $launcher.Refresh()
        $engine.Refresh()
        if (-not $launcher.HasExited -and -not $engine.HasExited) {
            $samples.Add([ordered]@{
                utc = [DateTime]::UtcNow.ToString('O')
                veloPrivateBytes = $launcher.PrivateMemorySize64; veloWorkingSetBytes = $launcher.WorkingSet64
                everythingPrivateBytes = $engine.PrivateMemorySize64; everythingWorkingSetBytes = $engine.WorkingSet64
            })
        }
        if ([DateTime]::UtcNow -gt $deadline) { throw 'Everything UI verification timed out.' }
    }
    $launcher.Refresh()
    $text = Get-Content -Raw -LiteralPath $log
    if ($launcher.ExitCode -ne 0 -or $text -notmatch 'event=everything_smoke_result passed=true' -or
        $text -notmatch 'event=desktop_stopped tray_removed=true') {
        throw 'Everything UI verification failed; inspect the isolated stderr log.'
    }
    foreach ($picture in 'files-light.png','files-dark.png','files-restored.png','everything-settings.png') {
        if (-not (Test-Path -LiteralPath (Join-Path $data $picture))) { throw "Missing screenshot: $picture" }
    }
    $saved = Get-Content -Raw -LiteralPath (Join-Path $data 'config.json') | ConvertFrom-Json
    if ($saved.everything.enabled) { throw 'Disabling Everything did not persist.' }
    [ordered]@{
        passed = $true; utc = [DateTime]::UtcNow.ToString('O')
        executableSHA256 = (Get-FileHash -LiteralPath $Executable -Algorithm SHA256).Hash
        everythingSHA256 = (Get-FileHash -LiteralPath $EverythingExecutable -Algorithm SHA256).Hash
        everythingVersion = (Get-Item -LiteralPath $EverythingExecutable).VersionInfo.FileVersion
        sampleScope = 'Instrumented component test with a 125-file synthetic index; not an idle-memory benchmark.'
        samples = $samples.ToArray()
    } | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $OutputDirectory 'verification.json') -Encoding utf8NoBOM
    Write-Output 'Everything live IPC, mode/input guards, paging, hide/settings restoration and screenshots passed.'
} finally {
    if ($launcher -and -not $launcher.HasExited) { Stop-Process -Id $launcher.Id -ErrorAction SilentlyContinue }
    if ($engine -and -not $engine.HasExited) { Stop-Process -Id $engine.Id -ErrorAction SilentlyContinue }
}
