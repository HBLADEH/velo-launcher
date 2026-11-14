#Requires -Version 7.4
# Stages the disposable-guest machine acceptance input. The fault fixture is a
# test-only rebuild of the exact Software payload with one throw after program
# replacement; it is never published and never used as a product installer.
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$SoftwareDirectory,
    [Parameter(Mandatory)][string]$FemtovgDirectory,
    [Parameter(Mandatory)][string]$GoInstaller,
    [Parameter(Mandatory)][string]$GoExecutableSHA256,
    [Parameter(Mandatory)][string]$SeedDirectory,
    [Parameter(Mandatory)][string]$OutputDirectory,
    [Parameter(Mandatory)][string]$Makensis
)
$ErrorActionPreference = 'Stop'
$repository = Split-Path -Parent $PSScriptRoot
$target = (Resolve-Path -LiteralPath (Join-Path $repository 'rust/target')).Path
$OutputDirectory = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($OutputDirectory)
if (-not $OutputDirectory.StartsWith($target + '\', [StringComparison]::OrdinalIgnoreCase) -or (Test-Path -LiteralPath $OutputDirectory)) { throw 'Use a fresh machine acceptance input inside rust/target.' }
$Makensis = (Resolve-Path -LiteralPath $Makensis).Path
$toolchain = Get-Content -LiteralPath (Join-Path $repository 'rust/windows/nsis-toolchain.json') -Raw | ConvertFrom-Json
if ((Get-FileHash -LiteralPath $Makensis).Hash -ne $toolchain.compilerSHA256 -or (& $Makensis /VERSION) -ne "v$($toolchain.version)") { throw 'Pinned NSIS identity required.' }
New-Item -ItemType Directory -Path $OutputDirectory | Out-Null

$renderers = @{}
foreach ($entry in @(@{ Name = 'software'; Directory = $SoftwareDirectory }, @{ Name = 'femtovg'; Directory = $FemtovgDirectory })) {
    $product = (Resolve-Path -LiteralPath $entry.Directory).Path
    $receipt = Get-Content -LiteralPath (Join-Path $product 'installer.json') -Raw | ConvertFrom-Json
    $filename = if ($receipt.PSObject.Properties['InstallerFile']) { $receipt.InstallerFile } else { "velo-rust-candidate-$($entry.Name)-x64-installer.exe" }
    $installer = Join-Path $product $filename
    if (-not $receipt.ProductIdentity -or -not $receipt.SystemScope -or $receipt.Renderer -ne $entry.Name -or
        (Get-FileHash -LiteralPath $installer).Hash -ne $receipt.InstallerSHA256) { throw "Product installer identity differs: $($entry.Name)" }
    Copy-Item -LiteralPath $installer -Destination (Join-Path $OutputDirectory "$($entry.Name)-installer.exe")
    $renderers[$entry.Name] = $receipt
}
Copy-Item -LiteralPath (Resolve-Path -LiteralPath $GoInstaller).Path -Destination (Join-Path $OutputDirectory 'GoInstaller.exe')

# Rebuild the Software payload with a single post-replacement failure injected.
$content = Join-Path (Resolve-Path -LiteralPath $SoftwareDirectory).Path 'package/content'
$fault = Join-Path $OutputDirectory 'fault-content'
Copy-Item -LiteralPath $content -Destination $fault -Recurse
$script = Join-Path $fault 'machine-install.ps1'
$text = Get-Content -LiteralPath $script -Raw
$anchor = "        Write-DurableJson (Join-Path `$resources 'installation.json')"
$line = @($text -split "`r?`n" | Where-Object { $_.StartsWith($anchor) })
if ($line.Count -ne 1) { throw 'Expected exactly one installation receipt write to inject after.' }
$updated = $text.Replace($line[0], $line[0] + "`r`n        throw 'Injected post-replacement fault for machine recovery acceptance.'")
[IO.File]::WriteAllText($script, $updated, [Text.UTF8Encoding]::new($false))
$sums = @(Get-ChildItem -LiteralPath $fault -File -Recurse | Where-Object Name -ne 'SHA256SUMS.txt' | Sort-Object FullName | ForEach-Object {
    '{0}  {1}' -f (Get-FileHash -LiteralPath $_.FullName).Hash.ToLowerInvariant(), [IO.Path]::GetRelativePath($fault, $_.FullName).Replace('\', '/')
})
$sums | Set-Content -LiteralPath (Join-Path $fault 'SHA256SUMS.txt') -Encoding utf8NoBOM
$faultInstaller = Join-Path $OutputDirectory 'fault-installer.exe'
& $Makensis /WX /V3 /INPUTCHARSET UTF8 "/DCONTENT=$fault" "/DOUTPUT=$faultInstaller" "/DVERSION=$($renderers['software'].Version)" '/DRENDERER=software' (Join-Path $repository 'rust/windows/product-installer.nsi') *> (Join-Path $OutputDirectory 'fault-build.log')
if ($LASTEXITCODE) { throw 'Fault fixture NSIS compilation failed; inspect fault-build.log.' }

# The elevated stage script and the guest-only final cleanup travel with the
# payload so the guest never needs the repository or a network share to run.
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'test-rust-machine-install.ps1') -Destination $OutputDirectory
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'test-rust-machine-cleanup.ps1') -Destination $OutputDirectory
$seed = (Resolve-Path -LiteralPath $SeedDirectory).Path
Copy-Item -LiteralPath $seed -Destination (Join-Path $OutputDirectory 'seed') -Recurse
$seedFiles = @(Get-ChildItem -LiteralPath (Join-Path $OutputDirectory 'seed') -File -Recurse | Sort-Object FullName | ForEach-Object {
    [ordered]@{ Path = 'seed/' + [IO.Path]::GetRelativePath((Join-Path $OutputDirectory 'seed'), $_.FullName).Replace('\', '/'); SHA256 = (Get-FileHash -LiteralPath $_.FullName).Hash }
})
$files = @(Get-ChildItem -LiteralPath $OutputDirectory -File | Where-Object Name -notin 'machine-test-input.json', 'fault-build.log' | Sort-Object Name | ForEach-Object {
    [ordered]@{ Name = $_.Name; SHA256 = (Get-FileHash -LiteralPath $_.FullName).Hash }
})
[ordered]@{
    AtUtc = [datetime]::UtcNow.ToString('o')
    Purpose = 'Disposable RS14 guest machine acceptance only'
    GoExeSHA256 = $GoExecutableSHA256.ToUpperInvariant()
    ExecutableHashes = [ordered]@{ software = $renderers['software'].ExecutableSHA256; femtovg = $renderers['femtovg'].ExecutableSHA256 }
    InstallerHashes = [ordered]@{ software = $renderers['software'].InstallerSHA256; femtovg = $renderers['femtovg'].InstallerSHA256 }
    FaultFixture = [ordered]@{ BaseRenderer = 'software'; InjectedAfter = 'installation-receipt-write'; Published = $false }
    Files = $files
    SeedFiles = $seedFiles
} | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $OutputDirectory 'machine-test-input.json') -Encoding utf8NoBOM
Write-Output "Staged machine acceptance input with $($files.Count) verified files and $($seedFiles.Count) seed files; fault fixture is test-only."
