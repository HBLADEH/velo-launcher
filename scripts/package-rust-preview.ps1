#Requires -Version 7.4
[CmdletBinding()]
param(
    [Parameter(Mandatory)][ValidateSet('software','femtovg')][string]$Renderer,
    [Parameter(Mandatory)][string]$OutputDirectory,
    [string]$ArtifactDirectory,
    [switch]$Product,
    [switch]$Release
)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'lib/rust-license-policy.ps1')
. (Join-Path $PSScriptRoot 'lib/rust-build-identity.ps1')
$repository = Split-Path -Parent $PSScriptRoot
$rustRoot = $repository
$releasePolicy = Get-Content -Raw -LiteralPath (Join-Path $rustRoot 'windows/release-policy.json') | ConvertFrom-Json
if ($Release -and (-not $Product -or -not $releasePolicy.G4Passed -or
    -not $releasePolicy.PublicReleaseApproved -or -not $releasePolicy.RedistributionCleared)) {
    throw 'Public packaging requires an approved product release policy.'
}
if ($releasePolicy.DefaultRenderer -notin 'software','femtovg' -or $releasePolicy.AlternateRenderer -notin 'software','femtovg' -or
    $releasePolicy.DefaultRenderer -eq $releasePolicy.AlternateRenderer) { throw 'Invalid renderer release policy.' }
$build = Read-VeloVerifiedBuild -Repository $repository -Renderer $Renderer -ArtifactDirectory $ArtifactDirectory -Product:$Product -Release:$Release
$executable = $build.Executable
$OutputDirectory = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($OutputDirectory)
$target = (Resolve-Path -LiteralPath (Join-Path $rustRoot 'target')).Path
$parent = (Resolve-Path -LiteralPath (Split-Path -Parent $OutputDirectory)).Path
if (-not $OutputDirectory.StartsWith($target + '\', [StringComparison]::OrdinalIgnoreCase) -or
    ($parent -ne $target -and -not $parent.StartsWith($target + '\', [StringComparison]::OrdinalIgnoreCase))) {
    throw 'Stage preview archives inside target with an existing parent.'
}
if (Test-Path -LiteralPath $OutputDirectory) { throw 'Use a fresh preview output directory.' }
New-Item -ItemType Directory -Path $OutputDirectory | Out-Null
$content = Join-Path $OutputDirectory 'content'
New-Item -ItemType Directory -Path $content | Out-Null
Copy-Item -LiteralPath $executable -Destination $content
Copy-Item -LiteralPath (Join-Path $repository 'LICENSE') -Destination $content
Copy-Item -LiteralPath $build.IdentityPath, $build.VerificationPath -Destination $content
$licenseRoot = Join-Path $content 'licenses'
New-Item -ItemType Directory -Path $licenseRoot | Out-Null
Copy-Item -LiteralPath (Join-Path $rustRoot 'licenses') -Destination (Join-Path $licenseRoot 'project') -Recurse
Copy-Item -LiteralPath (Join-Path $rustRoot 'ui/assets/fluent/LICENSE.txt') -Destination (Join-Path $licenseRoot 'Fluent-Icons-MIT.txt')
$metadata = Get-Content -LiteralPath $build.MetadataPath -Raw | ConvertFrom-Json
$supplements = Get-Content -LiteralPath (Join-Path $rustRoot 'licenses/dependency-supplements.json') -Raw | ConvertFrom-Json
foreach ($entry in $supplements.Entries) {
    foreach ($text in $entry.Files) {
        $source = [IO.Path]::GetFullPath((Join-Path $rustRoot $text.Path))
        if (-not $source.StartsWith((Join-Path $rustRoot 'licenses') + '\', [StringComparison]::OrdinalIgnoreCase) -or
            (Get-FileHash -LiteralPath $source).Hash -ne $text.SHA256) { throw 'Supplemental license text differs from its provenance.' }
    }
}
$resolved = @($metadata.resolve.nodes.id)
$inventory = @(foreach ($package in $metadata.packages | Where-Object { $_.id -in $resolved -and $_.name -ne 'velo-slint' } | Sort-Object name,version) {
    $directory = Split-Path -Parent $package.manifest_path
    $selectedLicense = Get-VeloRustLicenseSelection $package.license
    $files = @(Get-VeloRustLicenseFiles $package)
    $rootFiles = @($files | Where-Object DirectoryName -EQ $directory)
    $nestedFiles = @($files | Where-Object DirectoryName -NE $directory)
    $embedded = @(Get-VeloRustEmbeddedNotices $package)
    $name = "$($package.name)-$($package.version)"
    $supplement = @($supplements.Entries | Where-Object { $_.Name -eq $package.name -and $_.Version -eq $package.version })
    $supplementTexts = @($supplement | ForEach-Object { $_.Files } | Where-Object { $_ })
    if ($files.Count) {
        $destination = Join-Path $licenseRoot $name
        New-Item -ItemType Directory -Path $destination | Out-Null
        foreach ($file in $files) {
            $relative = [IO.Path]::GetRelativePath($directory, $file.FullName)
            $copy = Join-Path $destination $relative
            New-Item -ItemType Directory -Path (Split-Path -Parent $copy) -Force | Out-Null
            Copy-Item -LiteralPath $file.FullName -Destination $copy
        }
    }
    $embeddedInventory = @(foreach ($notice in $embedded) {
        $relative = 'embedded/' + $notice.SourcePath + '.txt'
        $copy = Join-Path (Join-Path $licenseRoot $name) $relative
        New-Item -ItemType Directory -Path (Split-Path -Parent $copy) -Force | Out-Null
        [IO.File]::WriteAllText($copy, $notice.Text, [Text.UTF8Encoding]::new($false))
        [ordered]@{ Path=$relative; SHA256=$notice.NoticeSHA256; SourcePath=$notice.SourcePath; SourceSHA256=$notice.SourceSHA256 }
    })
    [ordered]@{
        Name = $package.name; Version = $package.version; SPDX = $package.license; Repository = $package.repository
        SelectedLicense = $selectedLicense
        RootLicenseTexts = @($rootFiles | ForEach-Object { [ordered]@{ Name = $_.Name; SHA256 = (Get-FileHash -LiteralPath $_.FullName).Hash } })
        RootLicenseTextsFound = $rootFiles.Count -gt 0
        NestedLicenseTexts = @($nestedFiles | ForEach-Object { [ordered]@{ Path = [IO.Path]::GetRelativePath($directory, $_.FullName).Replace('\','/'); SHA256 = (Get-FileHash -LiteralPath $_.FullName).Hash } })
        EmbeddedNotices = $embeddedInventory
        SupplementalLicenseTexts = $supplementTexts
        LicenseTextsFound = $files.Count -gt 0 -or $supplementTexts.Count -gt 0
    }
})
$inventory | ConvertTo-Json -Depth 7 | Set-Content -LiteralPath (Join-Path $content 'dependency-inventory.json') -Encoding utf8NoBOM
$missing = @($inventory | Where-Object { -not $_.RootLicenseTextsFound } | ForEach-Object { "$($_.Name)@$($_.Version)" })
$missingTexts = @($inventory | Where-Object { -not $_.LicenseTextsFound } | ForEach-Object { "$($_.Name)@$($_.Version)" })
$version = ([Diagnostics.FileVersionInfo]::GetVersionInfo($executable)).ProductVersion
if ($Release -and ($missingTexts.Count -or $releasePolicy.ReleaseTag -notmatch '^v?\d+\.\d+\.\d+$' -or
    $version -ne ($releasePolicy.ReleaseTag -replace '^v', ''))) {
    throw 'Release version or collected dependency licenses do not match the approved policy.'
}
@'
# Velo Rust internal preview

This is an internal validation archive, not a product release. The default
renderer for the tested Windows 11 x64 environment is {{DEFAULT_RENDERER}}.
Product G4 acceptance is recorded separately. The launcher uses an independent instance
and data directory; exit any other Velo before opening it to avoid hotkey conflicts.

Run RUN-PREVIEW.ps1 with PowerShell. Do not point it at production data. Changing
renderers uses the same preview data and requires the previous process to exit.
To return to the installed release, exit the preview and start it normally.
The preview does not migrate or replace the installed application's files.

The dependency inventory conservatively includes resolved build dependencies.
Root and nested license files and exact-revision supplements are collected with
provenance, together with Slint, Fluent, Go and go-pinyin notices. The inventory
records explicit license choices; both terms of AND expressions remain required.
Slint uses the Royalty-free 2.0 option for this desktop application; keep the
actual AboutSlint widget accessible from Settings / About. The MIT project
license does not replace dependency terms. This internal validation bundle is
NOT a public redistribution approval.

Use manual replacement or the separately validated per-user preview installer.
Public distribution, real administrative installation, Windows/driver coverage
and the final release choice remain subject to RS-14/G4.
'@.Replace('{{DEFAULT_RENDERER}}', $releasePolicy.DefaultRenderer) | Set-Content -LiteralPath (Join-Path $content 'README.md') -Encoding utf8NoBOM
@'
$ErrorActionPreference = 'Stop'
$info = [Diagnostics.ProcessStartInfo]::new((Join-Path $PSScriptRoot 'velo-slint.exe'))
$info.UseShellExecute = $false
$info.CreateNoWindow = $true
$info.WindowStyle = 'Hidden'
$info.ArgumentList.Add('--instance-id=preview')
$info.ArgumentList.Add(('--data-dir=' + (Join-Path $env:LOCALAPPDATA 'VeloSlintPreview')))
[Diagnostics.Process]::Start($info).Dispose()
'@ | Set-Content -LiteralPath (Join-Path $content 'RUN-PREVIEW.ps1') -Encoding utf8NoBOM
[ordered]@{
    RecordedAtUtc = [DateTime]::UtcNow.ToString('o'); Renderer = $Renderer
    ProductIdentity = $Product.IsPresent
    ExecutableSHA256 = $build.Receipt.ExecutableSHA256
    InternalValidationOnly = -not $Release; RedistributionCleared = $Release.IsPresent; DefaultRenderer = $releasePolicy.DefaultRenderer
    PublicReleaseApproved = $Release.IsPresent
    ReleaseTag = $(if ($Release) { $releasePolicy.ReleaseTag } else { $null })
    DefaultRendererScope = $releasePolicy.Scope
    ResolvedDependencies = $inventory.Count; MissingRootLicenseTexts = $missing
    MissingLicenseTexts = $missingTexts
    SupplementalLicenseEntries = @($supplements.Entries).Count
    LicenseOptionsRecorded = $true
    NestedLicenseTexts = @($inventory | ForEach-Object { $_.NestedLicenseTexts }).Count
    EmbeddedNotices = @($inventory | ForEach-Object { $_.EmbeddedNotices }).Count
    Version = $version
    NSISValidated = $false; UpgradeRollbackPackageValidated = $false; G4Passed = $Release.IsPresent
} | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $content 'package.json') -Encoding utf8NoBOM
if ($Product) {
    Remove-Item -LiteralPath (Join-Path $content 'RUN-PREVIEW.ps1')
    Copy-Item -LiteralPath (Join-Path $rustRoot 'windows/machine-install.ps1') -Destination $content
    @'
# Velo Rust machine-install candidate

Internal validation only. This candidate payload is not approved for public redistribution.
Use the separately built elevated system installer. Do not run this raw payload
against production data. Explicit test identities remain isolated.

The system installer replaces the registered Go v0.9.11 program in place and
retains byte-verified program/registry/shortcut snapshots in .velo-rollback.
Exit Velo first. To return to the retained Go program, run uninstall.exe
/ROLLBACK (or choose rollback when prompted); data remains in place.

On each user's first product launch, a complete byte-verified data snapshot is
required before any data-store initialization. Snapshots are stored alongside
Velo in VeloRustMigrationBackups, one directory per install transaction. Backup
failure blocks data initialization. They are never deleted by the installer.
Unknown program files and user data survive Rust uninstall.

The renderer remains an explicit build choice. Keep Settings / AboutSlint and
all collected dependency license texts. A successful installer test does not
extend physical driver, IME, performance or public redistribution acceptance.
'@ | Set-Content -LiteralPath (Join-Path $content 'README.md') -Encoding utf8NoBOM
}
if ($Release) {
    @'
# Velo {{VERSION}} — Rust + Slint

Windows 10/11 x64 application launcher. Install using the separate system
installer; this ZIP is an installation payload, not a portable application.
Software is the default renderer; FemtoVG is an explicit alternative.
Exit Velo from the tray before installing or switching renderers.

The installer can take over the verified official v0.9.11 system installation
in its existing directory. Other legacy builds must be uninstalled first,
preserving user data. See the v0.10.0 release notes for migration details.
Future updates use the matching Rust renderer's system installer.

Data stays in %LOCALAPPDATA%\Velo. Before first initialization, each user receives
a complete, byte-verified snapshot under sibling VeloRustMigrationBackups.
Backup failure stops initialization. Uninstall preserves data, unknown files and
.velo-rollback program archives. /ROLLBACK is available only when an original
Go installation was retained; it is a recovery option, not a maintained release.

Release verification scope: {{ACCEPTANCE_SCOPE}}
See the release manifest for the validation evidence and build identity.

Keep the project LICENSE, dependency-inventory.json and all licenses/ texts.
Slint uses Royalty-free 2.0; Settings / About provides the actual AboutSlint.
The MIT project license does not replace third-party terms.
'@.Replace('{{VERSION}}', $version).Replace('{{ACCEPTANCE_SCOPE}}', $releasePolicy.Scope) | Set-Content -LiteralPath (Join-Path $content 'README.md') -Encoding utf8NoBOM
}
$sums = @(Get-ChildItem -LiteralPath $content -File -Recurse | Sort-Object FullName | ForEach-Object {
    '{0}  {1}' -f (Get-FileHash -LiteralPath $_.FullName).Hash.ToLowerInvariant(), [IO.Path]::GetRelativePath($content, $_.FullName).Replace('\','/')
})
$sums | Set-Content -LiteralPath (Join-Path $content 'SHA256SUMS.txt') -Encoding utf8NoBOM
# Cargo archives use old timestamps for some license files. Normalize only the
# staged copies so ZIP's 1980 lower bound does not emit hundreds of warnings.
foreach ($file in Get-ChildItem -LiteralPath $content -Recurse -File) {
    if ($file.LastWriteTime.Year -lt 1980) { $file.LastWriteTime = [datetime]'1980-01-01' }
}
$archiveName = if ($Release) { "velo-rust-$version-$Renderer-payload.zip" } elseif ($Product) { "velo-rust-candidate-$Renderer-x64.zip" } else { "velo-slint-internal-$Renderer-x64.zip" }
$archive = Join-Path $OutputDirectory $archiveName
Compress-Archive -Path (Join-Path $content '*') -DestinationPath $archive
('{0}  {1}' -f (Get-FileHash -LiteralPath $archive).Hash.ToLowerInvariant(), [IO.Path]::GetFileName($archive)) |
    Set-Content -LiteralPath (Join-Path $OutputDirectory 'SHA256SUMS.txt') -Encoding utf8NoBOM
Write-Output "Staged $Renderer package; release=$($Release.IsPresent); $($missingTexts.Count) dependencies lack collected license texts."
