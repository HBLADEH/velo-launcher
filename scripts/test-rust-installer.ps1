#Requires -Version 7.4
[CmdletBinding()]
param(
    [Parameter(Mandatory)][ValidateSet('software','femtovg')][string]$Renderer,
    [Parameter(Mandatory)][string]$OutputDirectory,
    [Parameter(Mandatory)][string]$Makensis
)
$ErrorActionPreference='Stop'
$repository=Split-Path -Parent $PSScriptRoot
$target=(Resolve-Path -LiteralPath (Join-Path $repository 'rust/target')).Path
$OutputDirectory=$ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($OutputDirectory)
if(-not $OutputDirectory.StartsWith($target+'\',[StringComparison]::OrdinalIgnoreCase) -or (Test-Path -LiteralPath $OutputDirectory)){throw 'Use a fresh installer test directory inside rust/target.'}
New-Item -ItemType Directory -Path $OutputDirectory|Out-Null
$preview=Join-Path $OutputDirectory 'preview'
& (Join-Path $PSScriptRoot 'package-rust-preview.ps1') -Renderer $Renderer -OutputDirectory $preview
$install=Join-Path $OutputDirectory '安装 空格'
$build=Join-Path $OutputDirectory 'installer'
& (Join-Path $PSScriptRoot 'package-rust-installer.ps1') -PreviewContent (Join-Path $preview 'content') -OutputDirectory $build -Makensis $Makensis -ValidationInstallDirectory $install
$installer=(Get-ChildItem -LiteralPath $build -Filter '*installer.exe').FullName
function Invoke-Installer([string]$arguments,[int]$expected){
    $process=Start-Process -FilePath $installer -ArgumentList $arguments -WindowStyle Hidden -PassThru
    if(-not $process.WaitForExit(30000)){throw 'Installer did not exit.'}
    if($process.ExitCode -ne $expected){throw "Installer exit $($process.ExitCode), expected $expected."}
    $process.Dispose()
}
$cases=@()
Invoke-Installer '/S' 21
if(Test-Path -LiteralPath $install){throw 'Validation installer wrote without the exact /D target.'}
$cases+=@{Name='wrong-target-refused';Passed=$true}
Invoke-Installer ("/S /D=$install") 0
$exe=Join-Path $install 'velo-slint.exe'
$hash=(Get-FileHash -LiteralPath $exe).Hash
if($hash -ne (Get-FileHash -LiteralPath "$preview/content/velo-slint.exe").Hash){throw 'Installed executable differs.'}
foreach($rs14File in Get-ChildItem -LiteralPath "$preview/content" -Recurse -File){
    $rs14Relative=[IO.Path]::GetRelativePath("$preview/content",$rs14File.FullName)
    if((Get-FileHash -LiteralPath (Join-Path $install $rs14Relative)).Hash -ne (Get-FileHash -LiteralPath $rs14File.FullName).Hash){throw 'Installed payload file differs from verified preview.'}
}
$sentinel=Join-Path $install 'user-owned.keep'
Set-Content -LiteralPath $sentinel -Value 'Keep unknown files during uninstall.' -Encoding utf8NoBOM
$cases+=@{Name='install-path-with-chinese-and-spaces';Passed=$true}
$smokeLog=Join-Path $OutputDirectory 'installed-smoke.log'
$smoke=Start-Process -FilePath $exe -ArgumentList '--smoke-test','--diagnostics' -WindowStyle Hidden -PassThru -RedirectStandardError $smokeLog
if(-not $smoke.WaitForExit(15000)){Stop-Process -Id $smoke.Id;throw 'Installed payload smoke timed out.'}
$smoke.Refresh()
if($smoke.ExitCode -ne 0 -or (Get-Content -LiteralPath $smokeLog -Raw) -notmatch 'event=smoke_result passed=true'){throw 'Installed payload did not render successfully.'}
$smoke.Dispose()
$cases+=@{Name='installed-executable-window-smoke';Passed=$true}
$userData=Join-Path $install 'user-data'
New-Item -ItemType Directory -Path $userData|Out-Null
$userHashes=@{}
foreach($userName in 'config.json','history.json','index.json','application-library.json','backup.json'){
    $userPath=Join-Path $userData $userName
    [IO.File]::WriteAllText($userPath,"{`"fixture`":`"$userName`",`"unknown`":12345678901234567890123456789}`r`n")
    $userHashes[$userName]=(Get-FileHash -LiteralPath $userPath).Hash
}
$locked=[IO.File]::Open($exe,[IO.FileMode]::Open,[IO.FileAccess]::Read,[IO.FileShare]::Read)
try{Invoke-Installer ("/S /D=$install") 23}finally{$locked.Dispose()}
if((Get-FileHash -LiteralPath $exe).Hash -ne $hash){throw 'Locked executable refusal changed the installed program.'}
$cases+=@{Name='locked-program-refused-without-change';Passed=$true}
$locked=[IO.File]::Open($exe,[IO.FileMode]::Open,[IO.FileAccess]::Read,[IO.FileShare]::Read)
try {
    # _?= runs the real uninstaller in place so its actual exit code, rather
    # than the code of its temporary-copy launcher, is observable.
    $uninstallLocked=Start-Process -FilePath (Join-Path $install 'uninstall.exe') -ArgumentList ("/S _?=$install") -WindowStyle Hidden -PassThru
    try {
        if(-not $uninstallLocked.WaitForExit(30000) -or $uninstallLocked.ExitCode -ne 23){throw 'Uninstaller did not reject the locked program.'}
    } finally { $uninstallLocked.Dispose() }
} finally { $locked.Dispose() }
if((Get-FileHash -LiteralPath $exe).Hash -ne $hash -or -not(Test-Path -LiteralPath "$install/preview-owner.txt")){throw 'Locked uninstall changed the installation.'}
$cases+=@{Name='locked-uninstall-refused-before-payload-removal';Passed=$true}
# Different bytes prove that the failed installation restores the old program,
# instead of reporting success because the replacement was already identical.
$oldBytes=[IO.File]::ReadAllBytes($exe)
[IO.File]::WriteAllBytes($exe,($oldBytes+[byte[]](1,2,3,4)))
$before=(Get-FileHash -LiteralPath $exe).Hash
Invoke-Installer ("/S /FAILAFTERBACKUP /D=$install") 24
if((Get-FileHash -LiteralPath $exe).Hash -ne $before -or (Test-Path -LiteralPath "$install/velo-slint.previous.exe")){throw 'Failed installation did not restore the exact previous executable.'}
$cases+=@{Name='failure-after-backup-restores-exact-old-bytes';Passed=$true}
Invoke-Installer ("/S /D=$install") 0
if((Get-FileHash -LiteralPath $exe).Hash -ne $hash){throw 'Upgrade replacement failed.'}
$cases+=@{Name='upgrade-replaces-old-payload';Passed=$true}
$uninstaller=Join-Path $install 'uninstall.exe'
$process=Start-Process -FilePath $uninstaller -ArgumentList '/S' -WindowStyle Hidden -PassThru
if(-not $process.WaitForExit(30000)){throw 'Uninstaller did not exit.'}
$process.Dispose()
$deadline=[datetime]::UtcNow.AddSeconds(20)
while((Test-Path -LiteralPath $exe) -and [datetime]::UtcNow -lt $deadline){Start-Sleep -Milliseconds 200}
if((Test-Path -LiteralPath $exe) -or -not (Test-Path -LiteralPath $sentinel)){throw 'Uninstall removed unknown data or kept installed executable.'}
foreach($userName in $userHashes.Keys){if((Get-FileHash -LiteralPath (Join-Path $userData $userName)).Hash -ne $userHashes[$userName]){throw 'Upgrade, failure recovery or uninstall changed synthetic user data.'}}
$cases+=@{Name='uninstall-preserves-unknown-files';Passed=$true}
[ordered]@{RecordedAtUtc=[datetime]::UtcNow.ToString('o');Renderer=$Renderer;Passed=$true;Cases=$cases;InstallerSHA256=(Get-FileHash -LiteralPath $installer).Hash;
    RegistryOrShortcutWrites=$false;UACValidated=$false;GoRustPackageMigrationValidated=$false;G4Passed=$false;
    PreservedUserDataFiles=$userHashes.Count;InstalledPayloadSmokePassed=$true;
    Scope='Actual pinned NSIS installer/uninstaller in one compile-time-constrained directory. Shared per-user payload replacement and failure recovery logic; no formal installation or data used.'
}|ConvertTo-Json -Depth 6|Set-Content -LiteralPath (Join-Path $OutputDirectory 'results.json') -Encoding utf8NoBOM
'Eight actual NSIS preview install/upgrade/failure/smoke/uninstall cases passed.'
