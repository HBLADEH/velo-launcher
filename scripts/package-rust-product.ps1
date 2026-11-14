#Requires -Version 7.4
[CmdletBinding()]
param(
    [Parameter(Mandatory)][ValidateSet('software','femtovg')][string]$Renderer,
    [Parameter(Mandatory)][string]$OutputDirectory,
    [Parameter(Mandatory)][string]$Makensis,
    [switch]$Release
)
$ErrorActionPreference='Stop'
$repository=Split-Path -Parent $PSScriptRoot
$target=(Resolve-Path -LiteralPath (Join-Path $repository 'rust/target')).Path
$OutputDirectory=$ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($OutputDirectory)
if (-not $OutputDirectory.StartsWith($target+'\',[StringComparison]::OrdinalIgnoreCase) -or (Test-Path -LiteralPath $OutputDirectory)) { throw 'Use a fresh product output inside rust/target.' }
$Makensis=(Resolve-Path -LiteralPath $Makensis).Path
$toolchain=Get-Content -LiteralPath (Join-Path $repository 'rust/windows/nsis-toolchain.json') -Raw | ConvertFrom-Json
if ((Get-FileHash -LiteralPath $Makensis).Hash -ne $toolchain.compilerSHA256 -or (& $Makensis /VERSION) -ne "v$($toolchain.version)") { throw 'Pinned NSIS identity required.' }
New-Item -ItemType Directory -Path $OutputDirectory | Out-Null
$package=Join-Path $OutputDirectory 'package'
& (Join-Path $PSScriptRoot 'package-rust-preview.ps1') -Renderer $Renderer -Product -Release:$Release -OutputDirectory $package
$content=Join-Path $package 'content'
$metadata=Get-Content -LiteralPath (Join-Path $content 'package.json') -Raw | ConvertFrom-Json
$suffix = if ($Renderer -eq 'femtovg') { '-femtovg' } else { '' }
$name = if ($Release) { "velo-rust$suffix-x64-installer.exe" } else { "velo-rust-candidate-$Renderer-x64-installer.exe" }
$output=Join-Path $OutputDirectory $name
& $Makensis /WX /V3 /INPUTCHARSET UTF8 "/DCONTENT=$content" "/DOUTPUT=$output" "/DVERSION=$($metadata.Version)" "/DRENDERER=$Renderer" (Join-Path $repository 'rust/windows/product-installer.nsi') *> (Join-Path $OutputDirectory 'build.log')
if ($LASTEXITCODE) { throw 'Product NSIS compilation failed; inspect build.log.' }
[ordered]@{ AtUtc=[datetime]::UtcNow.ToString('o'); ProductIdentity=$true; Renderer=$Renderer; Version=$metadata.Version;
    InstallerSHA256=(Get-FileHash -LiteralPath $output).Hash; ExecutableSHA256=$metadata.ExecutableSHA256;
    RegistryKey='HKLM64/Software/Microsoft/Windows/CurrentVersion/Uninstall/HBLADEHVelo';
    InstallerFile=$name; ReleaseTag=$metadata.ReleaseTag;
    AutomaticInPlaceTakeoverImplemented=$true; SystemScope=$true; InternalValidationOnly=(-not $Release);
    RedistributionCleared=$Release.IsPresent; PublicReleaseApproved=$Release.IsPresent; G4Passed=$Release.IsPresent; InstallerExecuted=$false
} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $OutputDirectory 'installer.json') -Encoding utf8NoBOM
"Built system installer; release=$($Release.IsPresent); no installer executed."
