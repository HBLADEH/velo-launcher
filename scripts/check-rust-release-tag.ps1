#Requires -Version 7.4
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$Tag,
    [Parameter(Mandatory)][ValidatePattern('\A[0-9a-f]{40}\z')][string]$EventSha
)

$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $true
$repository = Split-Path -Parent $PSScriptRoot
if ($Tag -cnotmatch '\Av(?<version>(?:0|[1-9][0-9]*)(?:\.(?:0|[1-9][0-9]*)){2})\z') {
    throw 'Release tags must use vMAJOR.MINOR.PATCH.'
}
$version = $Matches.version
$tagRef = "refs/tags/$Tag"
$commit = & git -C $repository rev-parse --verify 'HEAD^{commit}'
if ((& git -C $repository cat-file -t $tagRef) -ne 'tag') { throw 'Push an annotated release tag.' }
$tagCommit = & git -C $repository rev-parse --verify "${tagRef}^{commit}"
$eventCommit = & git -C $repository rev-parse --verify "${EventSha}^{commit}"
if ($commit -ne $tagCommit -or $commit -ne $eventCommit) { throw 'Checkout, release tag and push event must name the same commit.' }
# Checkout fetch-depth: 0 supplies refs/remotes/origin/main without credentials.
$mergeBase = & git -C $repository merge-base $commit refs/remotes/origin/main
if ($mergeBase -ne $commit) { throw 'Release tag must point to a commit reachable from origin/main.' }

$cargo = Get-Content -LiteralPath (Join-Path $repository 'Cargo.toml') -Raw
$package = [regex]::Match($cargo, '(?ms)^\[package\][^\r\n]*\r?\n(.*?)(?=^\[|\z)').Groups[1].Value
$cargoVersion = [regex]::Match($package, '(?m)^version = "([0-9]+\.[0-9]+\.[0-9]+)"\r?$').Groups[1].Value
$policy = Get-Content -LiteralPath (Join-Path $repository 'windows/release-policy.json') -Raw | ConvertFrom-Json
if ($version -cne $cargoVersion -or $Tag -cne $policy.ReleaseTag) { throw 'Tag, Cargo package version and release policy must match.' }
foreach ($field in 'G4Passed', 'RedistributionCleared', 'PublicReleaseApproved') {
    if ($policy.$field -isnot [bool] -or -not $policy.$field) { throw "Release requires explicit $field approval." }
}
if (-not (Test-Path -LiteralPath (Join-Path $repository "docs/releases/$version.md") -PathType Leaf)) {
    throw "The tagged commit must include docs/releases/$version.md."
}
Write-Output $commit
