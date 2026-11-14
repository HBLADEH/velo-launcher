#Requires -Version 7.4
[CmdletBinding()]
param(
    [Parameter(Mandatory)][ValidateSet('software','femtovg')][string]$Renderer,
    [Parameter(Mandatory)][string]$OutputDirectory
)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'lib/rust-license-policy.ps1')
$repository = Split-Path -Parent $PSScriptRoot
$root = (Resolve-Path -LiteralPath (Join-Path $repository 'rust/target')).Path
$OutputDirectory = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($OutputDirectory)
$parent = (Resolve-Path -LiteralPath (Split-Path -Parent $OutputDirectory)).Path
if (-not $OutputDirectory.StartsWith($root + '\', [StringComparison]::OrdinalIgnoreCase) -or
    ($parent -ne $root -and -not $parent.StartsWith($root + '\', [StringComparison]::OrdinalIgnoreCase))) {
    throw 'Preview verification must stay inside rust/target.'
}
if (Test-Path -LiteralPath $OutputDirectory) { throw 'Use a fresh verification directory.' }
New-Item -ItemType Directory -Path $OutputDirectory | Out-Null
$artifacts = Join-Path $root "artifacts/$Renderer"
$fixture = Join-Path $OutputDirectory 'fixture'
New-Item -ItemType Directory -Path $fixture | Out-Null
$files = @('velo-slint.exe','build-identity.json','verification.json','cargo-metadata.json')
foreach ($name in $files) { Copy-Item -LiteralPath (Join-Path $artifacts $name) -Destination $fixture }
$cases = @(
    @{ Name='skipped-checks'; Field='ChecksPassed'; Value=$false },
    @{ Name='missing-smoke'; Field='SmokeTestPassed'; Value=$false },
    @{ Name='changed-executable'; Field='ExecutableSHA256'; Value=('0' * 64) },
    @{ Name='changed-metadata'; Field='CargoMetadataSHA256'; Value=('0' * 64) },
    @{ Name='changed-source'; Field=$null; Value=$null }
)
$results = @(foreach ($case in $cases) {
    Copy-Item -LiteralPath (Join-Path $artifacts 'build-identity.json') -Destination $fixture -Force
    $receipt = Get-Content -LiteralPath (Join-Path $artifacts 'verification.json') -Raw | ConvertFrom-Json
    if ($case.Field) { $receipt.($case.Field) = $case.Value }
    else {
        $identity = Get-Content -LiteralPath (Join-Path $fixture 'build-identity.json') -Raw | ConvertFrom-Json
        $identity.Sources[0].SHA256 = '0' * 64
        $identity | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath (Join-Path $fixture 'build-identity.json') -Encoding utf8NoBOM
        $receipt.BuildIdentitySHA256 = (Get-FileHash -LiteralPath (Join-Path $fixture 'build-identity.json')).Hash
    }
    $receipt | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $fixture 'verification.json') -Encoding utf8NoBOM
    $destination = Join-Path $OutputDirectory $case.Name
    $rejected = $false
    try { & (Join-Path $PSScriptRoot 'package-rust-preview.ps1') -Renderer $Renderer -ArtifactDirectory $fixture -OutputDirectory $destination }
    catch {
        if ($_.Exception.Message -notmatch 'matching successful full verification|Source changed after verification') { throw }
        $rejected = $true
    }
    if (-not $rejected -or (Test-Path -LiteralPath $destination)) { throw "Invalid preview was staged: $($case.Name)" }
    [ordered]@{ Name=$case.Name; RejectedBeforeOutput=$true }
})
$valid = Join-Path $OutputDirectory 'valid'
& (Join-Path $PSScriptRoot 'package-rust-preview.ps1') -Renderer $Renderer -OutputDirectory $valid
$archive = Get-ChildItem -LiteralPath $valid -Filter '*.zip' -File
if (@($archive).Count -ne 1) { throw 'Expected one preview archive.' }
$zip = [IO.Compression.ZipFile]::OpenRead($archive.FullName)
try {
    $reader = [IO.StreamReader]::new($zip.GetEntry('SHA256SUMS.txt').Open())
    try { $lines = $reader.ReadToEnd() -split '\r?\n' | Where-Object { $_ } } finally { $reader.Dispose() }
    foreach ($line in $lines) {
        if ($line -notmatch '^([0-9a-f]{64})  (.+)$') { throw 'Malformed preview checksum.' }
        $expected, $name = $Matches[1], $Matches[2]
        $entry = $zip.GetEntry($name)
        if (-not $entry) { throw "Missing archive entry: $name" }
        $stream = $entry.Open()
        try { $hash = [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($stream)) } finally { $stream.Dispose() }
        if ($hash -ne $expected) { throw "Archive entry changed: $name" }
    }
    $reader = [IO.StreamReader]::new($zip.GetEntry('package.json').Open())
    try { $package = $reader.ReadToEnd() | ConvertFrom-Json } finally { $reader.Dispose() }
    if (-not $package.InternalValidationOnly -or $package.RedistributionCleared -or $package.G4Passed -or $package.Renderer -ne $Renderer) {
        throw 'Preview scope or renderer identity was misclassified.'
    }
    $policy = Get-Content -Raw -LiteralPath (Join-Path $repository 'rust/windows/release-policy.json') | ConvertFrom-Json
    if ($package.DefaultRenderer -ne $policy.DefaultRenderer) { throw 'Package default renderer differs from accepted policy.' }
    if (@($package.MissingLicenseTexts).Count -ne 0 -or $package.Version -notmatch '^\d+\.\d+\.\d+$') { throw 'Preview lacks license text coverage or synchronized version metadata.' }
    $reader = [IO.StreamReader]::new($zip.GetEntry('dependency-inventory.json').Open())
    try { $inventory = @($reader.ReadToEnd() | ConvertFrom-Json) } finally { $reader.Dispose() }
    foreach ($dependency in $inventory) {
        if (-not $dependency.SelectedLicense) { throw 'Dependency license choice missing.' }
        foreach ($nested in @($dependency.NestedLicenseTexts) + @($dependency.EmbeddedNotices)) {
            $entry = $zip.GetEntry("licenses/$($dependency.Name)-$($dependency.Version)/$($nested.Path)")
            if (-not $entry) { throw 'Nested dependency notice missing from ZIP.' }
            $stream = $entry.Open()
            try { $nestedHash = [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($stream)) } finally { $stream.Dispose() }
            if ($nestedHash -ne $nested.SHA256) { throw 'Nested dependency notice differs from inventory.' }
        }
    }
    foreach ($name in 'fontdb','regex-syntax','tracing-core') {
        $dependency = $inventory | Where-Object Name -EQ $name
        if (@($dependency.NestedLicenseTexts).Count -lt 1) { throw "Required nested notice missing: $name" }
    }
    if (@(($inventory | Where-Object Name -EQ 'khronos_api').EmbeddedNotices).Count -lt 1) { throw 'Khronos registry copyright/permission notices missing.' }
} finally { $zip.Dispose() }
[ordered]@{
    RecordedAtUtc=[DateTime]::UtcNow.ToString('o'); Renderer=$Renderer; Passed=$true
    NegativeCases=$results; ArchiveEntriesVerified=@($lines).Count
    ArchiveSHA256=(Get-FileHash -LiteralPath $archive.FullName).Hash
    MissingRootLicenseTexts=@($package.MissingRootLicenseTexts).Count
    MissingLicenseTexts=@($package.MissingLicenseTexts).Count
    NestedLicenseTexts=$package.NestedLicenseTexts; LicenseOptionsRecorded=$package.LicenseOptionsRecorded
    EmbeddedNotices=$package.EmbeddedNotices
    RedistributionCleared=$package.RedistributionCleared; G4Passed=$package.G4Passed
} | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $OutputDirectory 'result.json') -Encoding utf8NoBOM
Write-Output "Preview verification passed: $Renderer; five invalid inputs rejected; $(@($lines).Count) archive checksums verified."
