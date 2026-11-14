#Requires -Version 7.4
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$ReferenceManifest,
    [Parameter(Mandatory)][string]$OutputDirectory
)
$ErrorActionPreference = 'Stop'
$repository = Split-Path -Parent $PSScriptRoot
$reference = Get-Content -Raw -LiteralPath $ReferenceManifest | ConvertFrom-Json
$OutputDirectory = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($OutputDirectory)
if (Test-Path -LiteralPath $OutputDirectory) { throw 'Preserve earlier visual evidence; use a fresh directory.' }
$builds = [ordered]@{}
foreach ($renderer in 'software','femtovg') {
    $directory = Join-Path $repository "rust/target/artifacts/$renderer"
    $receipt = Get-Content -Raw -LiteralPath (Join-Path $directory 'verification.json') | ConvertFrom-Json
    $identity = Get-Content -Raw -LiteralPath (Join-Path $directory 'build-identity.json') | ConvertFrom-Json
    $executable = Join-Path $directory 'velo-slint.exe'
    if (-not $receipt.ChecksPassed -or -not $receipt.SmokeTestPassed -or
        (Get-FileHash -LiteralPath $executable).Hash -ne $receipt.ExecutableSHA256 -or
        (Get-FileHash -LiteralPath (Join-Path $directory 'build-identity.json')).Hash -ne $receipt.BuildIdentitySHA256) {
        throw "Missing complete verification for $renderer."
    }
    foreach ($source in $identity.Sources) {
        if ((Get-FileHash -LiteralPath (Join-Path $repository ('rust/' + $source.Path))).Hash -ne $source.SHA256) {
            throw "Verified build source changed: $($source.Path)."
        }
    }
    $builds[$renderer] = [ordered]@{Executable=$executable;SHA256=$receipt.ExecutableSHA256;BuildIdentitySHA256=$receipt.BuildIdentitySHA256;VerificationSHA256=(Get-FileHash -LiteralPath (Join-Path $directory 'verification.json')).Hash;Scope='Current verified RS-13 UI; separate from frozen RS-12 performance builds.'}
}
$builds.wails = $reference.Builds.wails
if ((Get-FileHash -LiteralPath $builds.wails.Executable).Hash -ne $builds.wails.SHA256) { throw 'Frozen Wails reference changed.' }
foreach ($inputFile in $reference.Inputs.PSObject.Properties) {
    if ($inputFile.Value -and (Get-FileHash -LiteralPath (Join-Path $reference.SeedDirectory $inputFile.Name)).Hash -ne $inputFile.Value) {
        throw 'Reference synthetic seed changed.'
    }
}
New-Item -ItemType Directory -Path $OutputDirectory | Out-Null
foreach ($theme in 'light','dark') {
    $directory = Join-Path $OutputDirectory $theme
    New-Item -ItemType Directory -Path $directory | Out-Null
    $seed = Join-Path $directory 'seed'
    Copy-Item -LiteralPath $reference.SeedDirectory -Destination $seed -Recurse
    # Only the disposable shared seed is changed. Each renderer receives an
    # independent copy with exactly the same initial bytes for this theme.
    $config = Get-Content -Raw -LiteralPath (Join-Path $seed 'config.json') | ConvertFrom-Json
    $config.theme = $theme; $config.auto_check_updates = $false; $config.launch_at_startup = $false
    $config | ConvertTo-Json -Depth 20 | Set-Content -LiteralPath (Join-Path $seed 'config.json') -Encoding utf8NoBOM
    $inputs = [ordered]@{}
    foreach ($name in 'config.json','index.json','history.json','application-library.json') {
        $inputs[$name] = (Get-FileHash -LiteralPath (Join-Path $seed $name)).Hash
    }
    [ordered]@{
        PreparedAtUtc=[datetime]::UtcNow.ToString('o');Theme=$theme;SeedDirectory=$seed;Builds=$builds;Inputs=$inputs
        ExpectedCatalogCount=$reference.ExpectedCatalogCount;ReferenceManifestSHA256=(Get-FileHash -LiteralPath $ReferenceManifest).Hash
        Scope='Same synthetic seed for native sequential visual comparison. Not physical input or a new RS-12 performance sample.'
    } | ConvertTo-Json -Depth 12 | Set-Content -LiteralPath (Join-Path $directory 'manifest.json') -Encoding utf8NoBOM
}
'Prepared independent light/dark native visual sessions; no application started.'
