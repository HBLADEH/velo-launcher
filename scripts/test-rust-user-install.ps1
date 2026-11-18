#Requires -Version 7.4
[CmdletBinding()]
param(
    [Parameter(Mandatory)][ValidateSet('software','femtovg')][string]$Renderer,
    [Parameter(Mandatory)][string]$OutputDirectory,
    [Parameter(Mandatory)][string]$Makensis
)
$ErrorActionPreference = 'Stop'
$repository = Split-Path -Parent $PSScriptRoot
$target = (Resolve-Path -LiteralPath (Join-Path $repository 'target')).Path
$OutputDirectory = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($OutputDirectory)
if (-not $OutputDirectory.StartsWith($target + '\', [StringComparison]::OrdinalIgnoreCase) -or (Test-Path -LiteralPath $OutputDirectory)) {
    throw 'Use a fresh user-install test directory inside target.'
}
$registration = 'Software\Microsoft\Windows\CurrentVersion\Uninstall\HBLADEHVeloSlintPreview'
$shortcut = Join-Path ([Environment]::GetFolderPath('Programs')) 'Velo Rust Preview.lnk'
function Read-Registration([Microsoft.Win32.RegistryView]$View) {
    $base = [Microsoft.Win32.RegistryKey]::OpenBaseKey([Microsoft.Win32.RegistryHive]::CurrentUser, $View)
    try {
        $key = $base.OpenSubKey($registration)
        if ($key) {
            try {
                $values = [ordered]@{}
                foreach ($name in $key.GetValueNames()) { $values[$name] = $key.GetValue($name) }
                return $values
            } finally { $key.Dispose() }
        }
    } finally { $base.Dispose() }
}
# Refuse to take over an existing preview. Tests execute the unrestricted
# registration branch but keep every program/data write in this task's folder.
if ((Read-Registration Registry64) -or (Read-Registration Registry32) -or (Test-Path -LiteralPath $shortcut)) {
    throw 'An existing preview registration or shortcut must be preserved; user-install test refused.'
}
if (@(Get-CimInstance Win32_Process -Filter "name='velo-slint.exe' OR name='velo-launcher.exe'").Count) { throw 'Exit Velo before the user-install test.' }
New-Item -ItemType Directory -Path $OutputDirectory | Out-Null
$preview = Join-Path $OutputDirectory 'preview'
& (Join-Path $PSScriptRoot 'package-rust-preview.ps1') -Renderer $Renderer -OutputDirectory $preview
$build = Join-Path $OutputDirectory 'installer'
& (Join-Path $PSScriptRoot 'package-rust-installer.ps1') -PreviewContent (Join-Path $preview 'content') -OutputDirectory $build -Makensis $Makensis
$installer = (Get-ChildItem -LiteralPath $build -Filter '*installer.exe').FullName
$install = Join-Path $OutputDirectory '每用户 安装'
$exe = Join-Path $install 'velo-slint.exe'
$cases = [Collections.Generic.List[object]]::new()
function Invoke-Silent([string]$Program, [string]$Arguments, [int]$Expected) {
    $process = Start-Process -FilePath $Program -ArgumentList $Arguments -WindowStyle Hidden -PassThru
    try {
        if (-not $process.WaitForExit(30000)) { throw 'Actual user installer/uninstaller timed out.' }
        if ($process.ExitCode -ne $Expected) { throw "Actual user installer exit $($process.ExitCode), expected $Expected." }
    } finally { $process.Dispose() }
}
$installed = $false
try {
    Invoke-Silent $installer ("/S /D=$install") 0
    $installed = $true
    $expectedHash = (Get-FileHash -LiteralPath "$preview/content/velo-slint.exe").Hash
    if ((Get-FileHash -LiteralPath $exe).Hash -ne $expectedHash) { throw 'Actual user installation changed the verified executable.' }
    $values = Read-Registration Registry64
    $package = Get-Content -Raw -LiteralPath "$preview/content/package.json" | ConvertFrom-Json
    if (-not $values -or
        $values.DisplayName -ne "Velo Rust Preview ($Renderer)" -or $values.DisplayVersion -ne $package.Version -or
        $values.UninstallString -ne ('"' + (Join-Path $install 'uninstall.exe') + '"') -or
        $values.NoModify -ne 1 -or $values.NoRepair -ne 1) { throw 'Actual x64 per-user uninstall registration differs.' }
    $cases.Add(@{ Name='actual-HKCU-x64-registration'; Passed=$true })
    $shell = New-Object -ComObject WScript.Shell
    try {
        $link = $shell.CreateShortcut($shortcut)
        try {
            if ($link.TargetPath -ne $exe -or $link.Arguments -ne ('--instance-id=preview --data-dir="' + (Join-Path $env:LOCALAPPDATA 'VeloSlintPreview') + '"')) {
                throw 'Actual Start Menu shortcut lost its isolated instance/data arguments.'
            }
        } finally { [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($link) }
    } finally { [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($shell) }
    $cases.Add(@{ Name='actual-start-menu-target-and-isolation'; Passed=$true })
    $log = Join-Path $OutputDirectory 'installed-smoke.log'
    $process = Start-Process -FilePath $exe -ArgumentList '--smoke-test','--diagnostics' -WindowStyle Hidden -PassThru -RedirectStandardError $log
    try {
        if (-not $process.WaitForExit(15000) -or $process.ExitCode -ne 0 -or (Get-Content -Raw -LiteralPath $log) -notmatch 'event=smoke_result passed=true') {
            throw 'Actual registered executable window smoke failed.'
        }
    } finally {
        if (-not $process.HasExited) { Stop-Process -Id $process.Id }
        $process.Dispose()
    }
    $cases.Add(@{ Name='registered-program-window-smoke'; Passed=$true })
    $sentinel = Join-Path $install 'user-owned.keep'
    [IO.File]::WriteAllText($sentinel, 'User-owned content must survive actual registered uninstall.')
    $before = (Get-FileHash -LiteralPath $sentinel).Hash
    $registryBefore = $values | ConvertTo-Json -Compress
    $shortcutBefore = (Get-FileHash -LiteralPath $shortcut).Hash
    $locked = [IO.File]::Open($exe, [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::Read)
    try {
        Invoke-Silent $installer ("/S /D=$install") 23
        Invoke-Silent (Join-Path $install 'uninstall.exe') ("/S _?=$install") 23
    } finally { $locked.Dispose() }
    if ((Get-FileHash -LiteralPath $exe).Hash -ne $expectedHash -or
        ((Read-Registration Registry64) | ConvertTo-Json -Compress) -ne $registryBefore -or
        (Get-FileHash -LiteralPath $shortcut).Hash -ne $shortcutBefore) { throw 'Locked install/uninstall changed payload or registration.' }
    $cases.Add(@{ Name='locked-upgrade-and-uninstall-preserve-registration'; Passed=$true })
    Invoke-Silent $installer ("/S /D=$install") 0
    if ((Get-FileHash -LiteralPath $exe).Hash -ne $expectedHash) { throw 'Registered upgrade changed executable identity.' }
    $cases.Add(@{ Name='actual-registered-upgrade'; Passed=$true })
    Invoke-Silent (Join-Path $install 'uninstall.exe') ("/S _?=$install") 0
    if ((Read-Registration Registry64) -or (Read-Registration Registry32) -or (Test-Path -LiteralPath $shortcut) -or
        (Test-Path -LiteralPath $exe) -or (Get-FileHash -LiteralPath $sentinel).Hash -ne $before) { throw 'Registered uninstall did not remove only owned resources.' }
    $installed = $false
    $cases.Add(@{ Name='actual-uninstall-cleans-registration-and-keeps-unknown-file'; Passed=$true })
} finally {
    if ($installed -and (Test-Path -LiteralPath (Join-Path $install 'uninstall.exe'))) {
        Invoke-Silent (Join-Path $install 'uninstall.exe') ("/S _?=$install") 0
    }
}
[ordered]@{
    RecordedAtUtc=[datetime]::UtcNow.ToString('o'); Renderer=$Renderer; Passed=$true; Cases=@($cases.ToArray())
    RegistryAndShortcutBranchExecuted=$true; RegistryView=64; RegistryAndShortcutCleaned=$true
    InstallerSHA256=(Get-FileHash -LiteralPath $installer).Hash; ExecutableSHA256=$expectedHash
    FormalApplicationReplaced=$false; DefaultPreviewDataWritten=$false; AdministrativeInstallValidated=$false; UACValidated=$false
    Scope='Actual unrestricted per-user NSIS registration/shortcut branches, task-local /D target, synthetic window smoke. No formal application or preview user data opened.'
} | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $OutputDirectory 'results.json') -Encoding utf8NoBOM
'Actual per-user installation, registration, shortcut, upgrade and uninstall passed; task entries cleaned.'
