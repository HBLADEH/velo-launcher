#Requires -Version 7.4
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$PreviewContent,
    [Parameter(Mandatory)][string]$OutputDirectory,
    [Parameter(Mandatory)][string]$Makensis,
    [string]$ValidationInstallDirectory
)
$ErrorActionPreference='Stop'
$repository=Split-Path -Parent $PSScriptRoot
$target=(Resolve-Path -LiteralPath (Join-Path $repository 'rust/target')).Path
$PreviewContent=(Resolve-Path -LiteralPath $PreviewContent).Path
$Makensis=(Resolve-Path -LiteralPath $Makensis).Path
$toolchain=Get-Content -LiteralPath (Join-Path $repository 'rust/windows/nsis-toolchain.json') -Raw|ConvertFrom-Json
if((Get-FileHash -LiteralPath $Makensis).Hash -ne $toolchain.compilerSHA256 -or (& $Makensis /VERSION) -ne "v$($toolchain.version)"){throw 'NSIS compiler does not match the pinned identity.'}
$OutputDirectory=$ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($OutputDirectory)
if(-not $OutputDirectory.StartsWith($target+'\',[StringComparison]::OrdinalIgnoreCase) -or (Test-Path -LiteralPath $OutputDirectory)){throw 'Use a fresh installer output inside rust/target.'}
if($ValidationInstallDirectory){
    $ValidationInstallDirectory=$ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($ValidationInstallDirectory)
    if(-not $ValidationInstallDirectory.StartsWith($target+'\',[StringComparison]::OrdinalIgnoreCase)){throw 'Validation install directory must stay inside rust/target.'}
}
$package=Get-Content -LiteralPath (Join-Path $PreviewContent 'package.json') -Raw|ConvertFrom-Json
$identity=Get-Content -LiteralPath (Join-Path $PreviewContent 'build-identity.json') -Raw|ConvertFrom-Json
$receipt=Get-Content -LiteralPath (Join-Path $PreviewContent 'verification.json') -Raw|ConvertFrom-Json
if(-not $package.InternalValidationOnly -or -not $receipt.ChecksPassed -or -not $receipt.SmokeTestPassed -or
    $package.Renderer -ne $identity.Renderer -or $package.ExecutableSHA256 -ne $receipt.ExecutableSHA256 -or
    (Get-FileHash -LiteralPath (Join-Path $PreviewContent 'build-identity.json')).Hash -ne $receipt.BuildIdentitySHA256){throw 'Installer requires a verified internal preview.'}
$listed=[Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
foreach($line in Get-Content -LiteralPath (Join-Path $PreviewContent 'SHA256SUMS.txt')){
    if($line -notmatch '^([0-9a-f]{64})  (.+)$'){throw 'Malformed preview checksum.'}
    $expected=$Matches[1];$name=$Matches[2]
    $path=[IO.Path]::GetFullPath((Join-Path $PreviewContent $name))
    if(-not $path.StartsWith($PreviewContent+'\',[StringComparison]::OrdinalIgnoreCase) -or -not $listed.Add($path) -or (Get-FileHash -LiteralPath $path).Hash -ne $expected){throw 'Preview file identity differs from checksums.'}
}
$files=@(Get-ChildItem -LiteralPath $PreviewContent -Recurse -File|Where-Object Name -ne 'SHA256SUMS.txt')
if($listed.Count -ne $files.Count -or @($files|Where-Object{-not $listed.Contains($_.FullName)}).Count){throw 'Unlisted preview payload file.'}
foreach($source in $identity.Sources){if((Get-FileHash -LiteralPath (Join-Path "$repository/rust" $source.Path)).Hash -ne $source.SHA256){throw 'Source changed after preview verification.'}}
$version=([Diagnostics.FileVersionInfo]::GetVersionInfo((Join-Path $PreviewContent 'velo-slint.exe'))).ProductVersion
if($version -notmatch '^\d+\.\d+\.\d+$' -or $package.Version -ne $version){throw 'Windows and package versions differ.'}
New-Item -ItemType Directory -Path $OutputDirectory|Out-Null
function Escape-Nsis([string]$value){$value.Replace('$','$$').Replace('"','$\"')}
$uninstall=Join-Path $OutputDirectory 'uninstall-files.nsh'
$delete=@(Get-ChildItem -LiteralPath $PreviewContent -Recurse -File|ForEach-Object{'Delete "$INSTDIR\'+(Escape-Nsis ([IO.Path]::GetRelativePath($PreviewContent,$_.FullName)))+'"'})
$delete+=@(Get-ChildItem -LiteralPath $PreviewContent -Recurse -Directory|Sort-Object {$_.FullName.Length} -Descending|ForEach-Object{'RMDir "$INSTDIR\'+(Escape-Nsis ([IO.Path]::GetRelativePath($PreviewContent,$_.FullName)))+'"'})
$delete|Set-Content -LiteralPath $uninstall -Encoding utf8NoBOM
$installer=Join-Path $OutputDirectory "velo-slint-internal-$($package.Renderer)-x64-installer.exe"
$arguments=@('/WX','/V3',('/DCONTENT='+$PreviewContent),('/DOUTPUT='+$installer),('/DVERSION='+$version),('/DRENDERER='+$package.Renderer),('/DUNINSTALL_FILES='+$uninstall))
if($ValidationInstallDirectory){$arguments+=('/DVALIDATION_INSTALL_DIR='+$ValidationInstallDirectory)}
$arguments+=(Join-Path $repository 'rust/windows/preview-installer.nsi')
& $Makensis @arguments *> (Join-Path $OutputDirectory 'build.log')
if($LASTEXITCODE){throw 'NSIS preview compilation failed; inspect build.log.'}
[ordered]@{RecordedAtUtc=[datetime]::UtcNow.ToString('o');Version=$version;Renderer=$package.Renderer;ExecutableSHA256=$package.ExecutableSHA256;
    InstallerSHA256=(Get-FileHash -LiteralPath $installer).Hash;CompilerSHA256=$toolchain.compilerSHA256;
    ValidationInstallDirectory=$ValidationInstallDirectory;RegistryAndShortcutsEnabled=(-not $ValidationInstallDirectory);
    InternalValidationOnly=$true;RedistributionCleared=$false;DefaultRenderer=$package.DefaultRenderer;G4Passed=$false
}|ConvertTo-Json|Set-Content -LiteralPath (Join-Path $OutputDirectory 'installer.json') -Encoding utf8NoBOM
'Built pinned NSIS per-user internal preview; no installer was executed.'
