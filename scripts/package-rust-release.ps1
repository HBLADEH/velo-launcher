#Requires -Version 7.4
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$OutputDirectory,
    [Parameter(Mandatory)][string]$Makensis
)
$ErrorActionPreference = 'Stop'
$repository = Split-Path -Parent $PSScriptRoot
$policy = Get-Content -LiteralPath (Join-Path $repository 'rust/windows/release-policy.json') -Raw | ConvertFrom-Json
if (-not $policy.G4Passed -or -not $policy.PublicReleaseApproved -or -not $policy.RedistributionCleared -or
    $policy.ReleaseTag -notmatch '^\d+\.\d+\.\d+$') { throw 'An approved numbered Rust release is required.' }
$target = (Resolve-Path -LiteralPath (Join-Path $repository 'rust/target')).Path
$OutputDirectory = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($OutputDirectory)
if (-not $OutputDirectory.StartsWith($target + '\', [StringComparison]::OrdinalIgnoreCase) -or
    (Test-Path -LiteralPath $OutputDirectory)) { throw 'Use a fresh release output inside rust/target.' }
New-Item -ItemType Directory -Path $OutputDirectory | Out-Null
$assets = Join-Path $OutputDirectory 'assets'
New-Item -ItemType Directory -Path $assets | Out-Null
$builds = @(foreach ($renderer in 'software','femtovg') {
    $directory = Join-Path $OutputDirectory $renderer
    & (Join-Path $PSScriptRoot 'package-rust-product.ps1') -Renderer $renderer -OutputDirectory $directory -Makensis $Makensis -Release | Out-Host
    $receipt = Get-Content -LiteralPath (Join-Path $directory 'installer.json') -Raw | ConvertFrom-Json
    $identity = Get-Content -LiteralPath (Join-Path $directory 'package/content/build-identity.json') -Raw | ConvertFrom-Json
    if ($identity.WorkspaceDirty) { throw 'Public release builds must bind a clean source commit.' }
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
$notices = Join-Path $OutputDirectory 'notices'
New-Item -ItemType Directory -Path $notices | Out-Null
Copy-Item -LiteralPath (Join-Path $repository 'LICENSE') -Destination $notices
foreach ($renderer in 'software','femtovg') {
    $rendererNotices = Join-Path $notices $renderer
    New-Item -ItemType Directory -Path $rendererNotices | Out-Null
    foreach ($name in 'licenses','dependency-inventory.json') {
        Copy-Item -LiteralPath (Join-Path $OutputDirectory "$renderer/package/content/$name") -Destination $rendererNotices -Recurse
    }
}
Compress-Archive -Path (Join-Path $notices '*') -DestinationPath (Join-Path $assets "velo-rust-$($policy.ReleaseTag)-licenses.zip")
[ordered]@{
    Version = $policy.ReleaseTag; Tag = $policy.ReleaseTag; Runtime = 'rust'; Architecture = 'x64'
    DefaultRenderer = $policy.DefaultRenderer; AcceptanceScope = $policy.Scope
    G4Evidence = $policy.Evidence; LicenseReview = $policy.LicenseReview
    PublicReleaseApproved = $true; RedistributionCleared = $true
    PortableDistribution = $false; Builds = $builds
} | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $assets 'release-manifest.json') -Encoding utf8NoBOM
Get-ChildItem -LiteralPath $assets -File | Sort-Object Name | ForEach-Object {
    '{0}  {1}' -f (Get-FileHash -LiteralPath $_.FullName).Hash.ToLowerInvariant(), $_.Name
} | Set-Content -LiteralPath (Join-Path $assets 'SHA256SUMS.txt') -Encoding utf8NoBOM
Write-Output "Prepared Rust $($policy.ReleaseTag) release assets. Final installer acceptance and publishing are separate steps."
