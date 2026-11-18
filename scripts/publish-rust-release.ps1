#Requires -Version 7.4
[CmdletBinding()]
param(
    [Parameter(Mandatory)][ValidatePattern('\Av[0-9]+\.[0-9]+\.[0-9]+\z')][string]$Tag,
    [Parameter(Mandatory)][ValidatePattern('\A[0-9a-f]{40}\z')][string]$Commit,
    [Parameter(Mandatory)][string]$Directory,
    [switch]$ValidateOnly,
    [switch]$StageDraft,
    [ValidatePattern('\A[0-9a-f]{64}\z')][string]$DraftChecksumsSHA256
)

$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $true
$repository = Split-Path -Parent $PSScriptRoot
$repo = 'HBLADEH/velo-launcher'
$version = $Tag.Substring(1)
$notes = Join-Path $repository "docs/releases/$version.md"
if (-not (Test-Path -LiteralPath $notes -PathType Leaf)) { throw "Release notes are missing for $Tag." }
if ($DraftChecksumsSHA256 -and ($StageDraft -or $ValidateOnly)) { throw 'Use the staged draft digest only when publishing.' }
if ($StageDraft -and $ValidateOnly) { throw 'Choose either local verification or draft staging.' }
if (-not $ValidateOnly -and -not $env:GH_TOKEN) { throw 'Set GH_TOKEN for GitHub release operations.' }

function Assert-RemoteTag {
    $remoteCommit = & gh api "repos/$repo/commits/refs/tags/$Tag" --jq .sha
    if ($remoteCommit -ne $Commit) { throw 'Remote release tag no longer points to the verified commit.' }
}

if ($DraftChecksumsSHA256) {
    Assert-RemoteTag
    if (Test-Path -LiteralPath $Directory) { throw 'Use a fresh directory for staged release assets.' }
    & gh release download $Tag --repo $repo --dir $Directory
    $stagedChecksums = Join-Path $Directory 'SHA256SUMS.txt'
    if ((Get-FileHash -LiteralPath $stagedChecksums -Algorithm SHA256).Hash -ne $DraftChecksumsSHA256) {
        throw 'Staged release checksum list does not match the digest in the tag annotation.'
    }
}

$Directory = (Resolve-Path -LiteralPath $Directory).Path
& (Join-Path $PSScriptRoot 'check-release-assets.ps1') -Directory $Directory

$alias = 'velo-launcher-amd64-installer.exe'
$installers = @{ software = 'velo-rust-x64-installer.exe'; femtovg = 'velo-rust-femtovg-x64-installer.exe' }
$names = @($alias, $installers.software, $installers.femtovg, "velo-rust-$version-licenses.zip", 'release-manifest.json', 'SHA256SUMS.txt')
$files = @(Get-ChildItem -LiteralPath $Directory -Force)
if (@($files | Where-Object PSIsContainer).Count -or (Compare-Object $names @($files.Name) -CaseSensitive)) {
    throw 'Expected exactly six release assets.'
}
$manifest = Get-Content -LiteralPath (Join-Path $Directory 'release-manifest.json') -Raw | ConvertFrom-Json
if ($manifest.Tag -cne $Tag -or $manifest.Version -cne $version -or
    $manifest.PublicReleaseApproved -isnot [bool] -or -not $manifest.PublicReleaseApproved -or
    $manifest.RedistributionCleared -isnot [bool] -or -not $manifest.RedistributionCleared -or
    (Compare-Object @('software','femtovg') @($manifest.Builds.Renderer) -CaseSensitive)) {
    throw 'Release manifest does not match the approved tag and renderers.'
}

$digests = @{}
foreach ($file in $files) {
    $digests[$file.Name] = (Get-FileHash -LiteralPath $file.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
}
foreach ($build in $manifest.Builds) {
    if ($build.SourceCommit -ne $Commit -or $build.Installer -cne $installers[$build.Renderer] -or
        $build.InstallerSHA256 -ne $digests[$build.Installer]) { throw 'An installer is not bound to the tagged commit and manifest.' }
}
if ($ValidateOnly) {
    Write-Output 'Local release package validated; no GitHub release operations were attempted.'
    return
}
function Read-VerifiedRelease {
    $release = & gh release view $Tag --repo $repo --json isDraft,tagName,targetCommitish,assets | ConvertFrom-Json
    if ($release.tagName -cne $Tag -or (Compare-Object $names @($release.assets.name) -CaseSensitive)) {
        throw 'GitHub Release does not contain the complete verified asset set.'
    }
    if (($DraftChecksumsSHA256 -or $StageDraft) -and $release.targetCommitish -ne $Commit) {
        throw 'Release draft targets a different commit.'
    }
    foreach ($asset in $release.assets) {
        if ($asset.state -ne 'uploaded' -or $asset.digest -ne "sha256:$($digests[$asset.name])") {
            throw "Uploaded asset failed SHA-256 verification: $($asset.name)"
        }
    }
    $release
}

if (-not $DraftChecksumsSHA256) {
    $tagOptions = @('--verify-tag')
    if ($StageDraft) { $tagOptions = @('--target', $Commit) }
    else { Assert-RemoteTag }
    # Unlike the published-release endpoint, this also returns existing drafts.
    $existingTags = @(& gh api "repos/$repo/releases?per_page=100" --paginate --jq '.[].tag_name')
    if ($Tag -in $existingTags) { throw 'A release or draft already exists for this tag. Do not overwrite it.' }
    & gh release create $Tag --repo $repo @tagOptions --draft --title "Velo $Tag" --notes-file $notes
    # Upload separately so the older Go updater sees the Software alias first.
    & gh release upload $Tag (Join-Path $Directory $alias) --repo $repo
    $otherFiles = @($names | Select-Object -Skip 1 | ForEach-Object { Join-Path $Directory $_ })
    & gh release upload $Tag @otherFiles --repo $repo
}

if (-not (Read-VerifiedRelease).isDraft) { throw 'Expected the verified Release to still be a draft.' }
if ($StageDraft) {
    Write-Output "Velo-Release-Draft: $($digests['SHA256SUMS.txt'])"
    return
}
Assert-RemoteTag
try { & gh release edit $Tag --repo $repo --draft=false }
catch [System.Management.Automation.NativeCommandExitException] {
    # GitHub may have committed the request before the client lost the response.
    Write-Warning "Publish command returned an error; checking the Release state: $($_.Exception.Message)"
}
if ((Read-VerifiedRelease).isDraft) { throw 'GitHub still reports a draft. Inspect it before rerunning the publish job.' }
