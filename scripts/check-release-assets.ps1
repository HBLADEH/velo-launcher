#Requires -Version 7.4
[CmdletBinding()]
param([Parameter(Mandatory)][string]$Directory)
$ErrorActionPreference = 'Stop'
$Directory = (Resolve-Path -LiteralPath $Directory).Path
$checksums = @{}
foreach ($line in Get-Content -LiteralPath (Join-Path $Directory 'SHA256SUMS.txt')) {
    if ([string]::IsNullOrWhiteSpace($line)) { continue }
    if ($line -notmatch '^([0-9a-fA-F]{64})\s+\*?([^/\\:]+)$') { throw 'Invalid checksum entry or non-local asset name.' }
    $hash = $Matches[1]
    $name = $Matches[2]
    if ($name -in @('.', '..') -or $checksums.ContainsKey($name)) { throw 'Duplicate or invalid release asset name.' }
    $checksums[$name] = $hash
    $path = Join-Path $Directory $name
    if (-not (Test-Path -LiteralPath $path -PathType Leaf) -or (Get-FileHash -LiteralPath $path).Hash -ne $hash) {
        throw "Missing asset or SHA-256 mismatch: $name"
    }
}
$required = @('velo-launcher-amd64-installer.exe', 'velo-rust-x64-installer.exe', 'velo-rust-femtovg-x64-installer.exe', 'release-manifest.json')
foreach ($name in $required) {
    if (-not $checksums.ContainsKey($name)) { throw "Required release asset is not covered by checksums: $name" }
}
$manifest = Get-Content -LiteralPath (Join-Path $Directory 'release-manifest.json') -Raw | ConvertFrom-Json
if ($manifest.Version -isnot [string] -or $manifest.Version -notmatch '\A[0-9]+\.[0-9]+\.[0-9]+\z') {
    throw 'Release manifest must declare a numbered version.'
}
$licenses = "velo-rust-$($manifest.Version)-licenses.zip"
if (-not $checksums.ContainsKey($licenses)) { throw "Required release asset is not covered by checksums: $licenses" }
if ($checksums['velo-launcher-amd64-installer.exe'] -ne $checksums['velo-rust-x64-installer.exe']) {
    throw 'The legacy installer alias must contain exactly the Software installer bytes.'
}
if (Test-Path -LiteralPath (Join-Path $Directory 'velo-launcher.exe')) { throw 'A raw executable must not be offered to the legacy portable updater.' }
$files = @(Get-ChildItem -LiteralPath $Directory -File | Where-Object Name -ne 'SHA256SUMS.txt')
if (@($files | Where-Object { -not $checksums.ContainsKey($_.Name) }).Count) { throw 'An asset is missing from SHA256SUMS.txt.' }
Write-Output "Release assets verified: $($checksums.Count) files; legacy alias equals Software. Upload the alias first."
