#Requires -Version 7.4
[CmdletBinding()]
param(
    [ValidateSet('software', 'femtovg')][string]$Renderer = 'software',
    [switch]$Build,
    [string]$Hotkey = 'Ctrl+Alt+K'
)
$ErrorActionPreference = 'Stop'
$repository = Split-Path -Parent $PSScriptRoot
if ($Build) { & (Join-Path $PSScriptRoot 'verify-rust.ps1') -Renderer $Renderer -SkipChecks }
$source = Join-Path $repository "target/artifacts/$Renderer/velo-slint.exe"
if (-not (Test-Path -LiteralPath $source)) { throw 'Build the preview first, or pass -Build.' }
# Run a copy so an open preview never locks the next build output.
$destination = Join-Path $repository "target/previews/$Renderer"
$executable = Join-Path $destination 'velo-slint.exe'
$data = Join-Path $repository '.dev-data/manual-preview'
$running = Get-CimInstance Win32_Process | Where-Object { $_.ExecutablePath -eq $executable }
if ($running) { throw 'Exit the current preview from its tray menu before replacing it.' }
New-Item -ItemType Directory -Path $destination, $data -Force | Out-Null
Copy-Item -LiteralPath $source -Destination $executable -Force
foreach ($name in 'build-identity.json', 'verification.json') {
    Copy-Item -LiteralPath (Join-Path (Split-Path -Parent $source) $name) -Destination $destination -Force
}
$info = [Diagnostics.ProcessStartInfo]::new($executable)
$info.UseShellExecute = $false
$info.CreateNoWindow = $true
$info.WindowStyle = 'Hidden'
$info.ArgumentList.Add('--instance-id=manual-preview')
$info.ArgumentList.Add('--data-dir=' + $data)
$info.ArgumentList.Add('--hotkey=' + $Hotkey)
[Diagnostics.Process]::Start($info).Dispose()
Write-Output "Preview started; hotkey=$Hotkey; data=$data"
