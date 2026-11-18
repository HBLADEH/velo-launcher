#Requires -Version 7.4
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$OutputDirectory,
    [Parameter(Mandatory)][string]$Makensis
)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'lib/rust-build-identity.ps1')
$repository = Split-Path -Parent $PSScriptRoot
$policy = Get-Content -LiteralPath (Join-Path $repository 'windows/release-policy.json') -Raw | ConvertFrom-Json
if (-not $policy.G4Passed -or -not $policy.PublicReleaseApproved -or -not $policy.RedistributionCleared -or
    $policy.ReleaseTag -notmatch '^v?\d+\.\d+\.\d+$') { throw 'An approved numbered Rust release is required.' }
$version = $policy.ReleaseTag -replace '^v', ''
$target = (Resolve-Path -LiteralPath (Join-Path $repository 'target')).Path
$OutputDirectory = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($OutputDirectory)
if (-not $OutputDirectory.StartsWith($target + '\', [StringComparison]::OrdinalIgnoreCase) -or
    (Test-Path -LiteralPath $OutputDirectory)) { throw 'Use a fresh release output inside target.' }
$renderers = @('software', 'femtovg')
# Either renderer can invalidate the release. Check both before staging anything.
foreach ($renderer in $renderers) {
    Read-VeloVerifiedBuild -Repository $repository -Renderer $renderer -Product -Release | Out-Null
}
New-Item -ItemType Directory -Path $OutputDirectory | Out-Null
$assets = Join-Path $OutputDirectory 'assets'
New-Item -ItemType Directory -Path $assets | Out-Null
$builds = @(foreach ($renderer in $renderers) {
    $directory = Join-Path $OutputDirectory $renderer
    & (Join-Path $PSScriptRoot 'package-rust-product.ps1') -Renderer $renderer -OutputDirectory $directory -Makensis $Makensis -Release | Out-Host
    $receipt = Get-Content -LiteralPath (Join-Path $directory 'installer.json') -Raw | ConvertFrom-Json
    $identity = Get-Content -LiteralPath (Join-Path $directory 'package/content/build-identity.json') -Raw | ConvertFrom-Json
    Copy-Item -LiteralPath (Join-Path $directory $receipt.InstallerFile) -Destination $assets
    [ordered]@{
        Renderer = $renderer; Installer = $receipt.InstallerFile
        InstallerSHA256 = $receipt.InstallerSHA256; ExecutableSHA256 = $receipt.ExecutableSHA256
        SourceCommit = $identity.WorkspaceCommit; SourceInputs = $identity.Sources
        Rustc = $identity.Rustc; SlintVersion = $identity.SlintVersion
        MsvcToolsVersion = $identity.MsvcToolsVersion; WindowsSdkVersion = $identity.WindowsSdkVersion
    }
})
if ($builds[0].SourceCommit -ne $builds[1].SourceCommit) { throw 'Both renderers must bind the same source commit.' }
# Old Go clients choose the first installer. Upload this Software alias first;
# its name also precedes the Rust assets on GitHub's public asset list.
$legacyInstaller = 'velo-launcher-amd64-installer.exe'
$software = $builds | Where-Object Renderer -eq 'software'
Copy-Item -LiteralPath (Join-Path $assets $software.Installer) -Destination (Join-Path $assets $legacyInstaller)
$notices = Join-Path $OutputDirectory 'notices'
New-Item -ItemType Directory -Path $notices | Out-Null
Copy-Item -LiteralPath (Join-Path $repository 'LICENSE') -Destination $notices
foreach ($renderer in $renderers) {
    $rendererNotices = Join-Path $notices $renderer
    New-Item -ItemType Directory -Path $rendererNotices | Out-Null
    foreach ($name in 'licenses','dependency-inventory.json') {
        Copy-Item -LiteralPath (Join-Path $OutputDirectory "$renderer/package/content/$name") -Destination $rendererNotices -Recurse
    }
}
Compress-Archive -Path (Join-Path $notices '*') -DestinationPath (Join-Path $assets "velo-rust-$version-licenses.zip")
[ordered]@{
    Version = $version; Tag = $policy.ReleaseTag; Runtime = 'rust'; Architecture = 'x64'
    DefaultRenderer = $policy.DefaultRenderer; AcceptanceScope = $policy.Scope
    G4Evidence = $policy.Evidence; LicenseReview = $policy.LicenseReview
    ReleaseEvidence = $policy.ReleaseEvidence
    PublicReleaseApproved = $true; RedistributionCleared = $true
    PortableDistribution = $false; Builds = $builds
    LegacyUpdate = [ordered]@{ Installer=$legacyInstaller; Renderer='software'; UploadFirst=$legacyInstaller; VerifiedGoVersion='0.9.11'; PortableAutomaticMigration=$false }
} | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $assets 'release-manifest.json') -Encoding utf8NoBOM
Get-ChildItem -LiteralPath $assets -File | Sort-Object Name | ForEach-Object {
    '{0}  {1}' -f (Get-FileHash -LiteralPath $_.FullName).Hash.ToLowerInvariant(), $_.Name
} | Set-Content -LiteralPath (Join-Path $assets 'SHA256SUMS.txt') -Encoding utf8NoBOM
& (Join-Path $PSScriptRoot 'check-release-assets.ps1') -Directory $assets
Write-Output "Prepared Rust $($policy.ReleaseTag) release assets. Final installer acceptance and publishing are separate steps."
