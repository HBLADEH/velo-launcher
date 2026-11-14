#Requires -Version 7.4
[CmdletBinding()]
param(
    [Parameter(Mandatory)][ValidateSet('software','femtovg')][string]$Renderer,
    [Parameter(Mandatory)][string]$OutputPath
)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'lib/rust-license-policy.ps1')
$repository = Split-Path -Parent $PSScriptRoot
$artifacts = Join-Path $repository "rust/target/artifacts/$Renderer"
$OutputPath = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($OutputPath)
if (Test-Path -LiteralPath $OutputPath) { throw 'Preserve the previous license audit; use a fresh output.' }
$metadataPath = Join-Path $artifacts 'cargo-metadata.json'
$metadata = Get-Content -Raw -LiteralPath $metadataPath | ConvertFrom-Json
$treePath = Join-Path $artifacts 'cargo-tree.txt'
$tree = Get-Content -Raw -LiteralPath $treePath
$supplements = Get-Content -Raw -LiteralPath (Join-Path $repository 'rust/licenses/dependency-supplements.json') | ConvertFrom-Json
$resolved = @($metadata.resolve.nodes.id)
$packages = @(foreach ($package in $metadata.packages | Where-Object { $_.id -in $resolved -and $_.name -ne 'velo-slint' } | Sort-Object name,version) {
    $directory = Split-Path -Parent $package.manifest_path
    $files = @(Get-VeloRustLicenseFiles $package)
    $embedded = @(Get-VeloRustEmbeddedNotices $package)
    $extra = @($supplements.Entries | Where-Object { $_.Name -eq $package.name -and $_.Version -eq $package.version } | ForEach-Object Files)
    if (-not $files.Count -and -not $extra.Count) { throw "Dependency license text absent: $($package.name)" }
    foreach ($text in $extra) {
        if ((Get-FileHash -LiteralPath (Join-Path "$repository/rust" $text.Path)).Hash -ne $text.SHA256) { throw 'Supplemental notice differs from provenance.' }
    }
    $active = [regex]::IsMatch($tree, '(?m)(?:^|[\s─])' + [regex]::Escape($package.name + ' v' + $package.version) + '(?:\s|$)')
    $native = @(Get-ChildItem -LiteralPath $directory -Recurse -File | Where-Object { $_.Extension -in '.c','.h','.cpp','.cc','.cxx','.S','.asm','.dll','.lib','.a','.o','.exe' })
    $nativeScope = switch ($package.name) {
        'cc' { 'Build-time compiler detection sources; absent from these active renderer trees.' }
        'cexpr' { 'Parser test headers; absent from these active renderer trees.' }
        'clang-sys' { 'Test header; absent from these active renderer trees.' }
        'libloading' { 'DLLs under tests; test binaries are not in the application payload.' }
        'skia-bindings' { 'Optional Skia bindings; absent from the active renderer tree. No Skia binaries or vendor sources in the preview payload.' }
        'webbrowser' { 'iOS test header; not part of Windows application payload.' }
        'windows_x86_64_msvc' { 'MIT/Apache Windows import library; MIT selected, system DLLs supplied by Windows.' }
        default { if ($native.Count) { throw "Unreviewed native dependency sources: $($package.name)" }; $null }
    }
    if ($package.name -in 'cc','cexpr','clang-sys','skia-bindings' -and $active) { throw 'Previously optional native dependency became active; repeat vendor review.' }
    [ordered]@{
        Name=$package.name; Version=$package.version; SPDX=$package.license
        SelectedLicense=(Get-VeloRustLicenseSelection $package.license); InActiveCargoTree=$active
        LicenseFiles=@($files | ForEach-Object { [ordered]@{ Path=[IO.Path]::GetRelativePath($directory,$_.FullName).Replace('\','/'); SHA256=(Get-FileHash -LiteralPath $_.FullName).Hash } })
        SupplementalLicenseFiles=$extra
        EmbeddedNotices=@($embedded | Select-Object SourcePath,SourceSHA256,NoticeSHA256)
        NativeFiles=@($native | ForEach-Object { [IO.Path]::GetRelativePath($directory,$_.FullName).Replace('\','/') })
        NativeReview=$nativeScope
    }
})
$unknownRejected = $false
try { $null = Get-VeloRustLicenseSelection 'Unreviewed-Example-1.0' } catch {
    if ($_.Exception.Message -notlike 'Unreviewed dependency license expression:*') { throw }
    $unknownRejected = $true
}
if (-not $unknownRejected) { throw 'License choice policy did not reject unknown expression.' }
[ordered]@{
    RecordedAtUtc=[datetime]::UtcNow.ToString('o'); Renderer=$Renderer
    ExecutableSHA256=(Get-FileHash -LiteralPath (Join-Path $artifacts 'velo-slint.exe')).Hash
    CargoMetadataSHA256=(Get-FileHash -LiteralPath $metadataPath).Hash; CargoTreeSHA256=(Get-FileHash -LiteralPath $treePath).Hash
    ResolvedDependencies=$packages.Count; ActiveTreeDependencies=@($packages | Where-Object InActiveCargoTree).Count
    UnknownLicenseRejected=$unknownRejected; LicenseOptionAndNestedVendorReviewCompleted=$true
    SlintOption='LicenseRef-Slint-Royalty-free-2.0; desktop integration with actual Settings/About AboutSlint widget retained'
    SourceDistributionApproved=$false; PublicReleaseApproved=$false
    Scope='Technical review of locked Windows exe dependencies and collected notices. Resolved optional/build packages remain conservatively included; absent active-tree branches are not claimed as linked.'
    Packages=$packages
} | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath $OutputPath -Encoding utf8NoBOM
'Locked license options, nested notices and native-source scope audited.'
