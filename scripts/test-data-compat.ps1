param([ValidateSet('FemtoVG', 'Software')][string]$Renderer = 'Software')
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'lib/rust-environment.ps1')
$rs08Environment = Initialize-VeloRustEnvironment
$repository = Split-Path -Parent $PSScriptRoot
$fixture = Join-Path $repository 'tests/fixtures/compat/rs08.json'
$temporaryBase = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\')
$directoryName = 'velo-rs08-' + [guid]::NewGuid().ToString('N')
$taskDirectory = [IO.Path]::GetFullPath((Join-Path $temporaryBase $directoryName))
New-Item -ItemType Directory -Path $taskDirectory | Out-Null

function Invoke-RS08Check([string]$Executable, [string[]]$Arguments) {
    & $Executable @Arguments
    if ($LASTEXITCODE -ne 0) { throw "RS-08 command failed: $($Arguments[0]) (exit $LASTEXITCODE)" }
}

Push-Location $repository
try {
    $goRunner = Join-Path $taskDirectory 'go-data-contract.exe'
    & go build -o $goRunner ./cmd/rs08-compat
    if ($LASTEXITCODE -ne 0) { throw 'Go data oracle build failed.' }
    Push-Location $rs08Environment.RustRoot
    try {
        & cargo build --locked --no-default-features --features "renderer-$($Renderer.ToLowerInvariant())" --example rs08_data_contract
        if ($LASTEXITCODE -ne 0) { throw 'Rust data oracle build failed.' }
    } finally { Pop-Location }
    $rustRunner = Join-Path $rs08Environment.RustRoot 'target/x86_64-pc-windows-msvc/debug/examples/rs08_data_contract.exe'
    Invoke-RS08Check $goRunner @('check', $fixture)
    Invoke-RS08Check $rustRunner @('check', $fixture)

    $goDirectory = Join-Path $taskDirectory 'go-generated'
    Invoke-RS08Check $goRunner @('emit', $fixture, $goDirectory)
    Invoke-RS08Check $rustRunner @('verify', $fixture, $goDirectory)
    Invoke-RS08Check $rustRunner @('rewrite', $fixture, $goDirectory)
    Invoke-RS08Check $goRunner @('verify-edited', $fixture, $goDirectory)
    Invoke-RS08Check $rustRunner @('verify-edited', $fixture, $goDirectory)

    $rustDirectory = Join-Path $taskDirectory 'rust-generated'
    Invoke-RS08Check $rustRunner @('emit', $fixture, $rustDirectory)
    Invoke-RS08Check $goRunner @('verify', $fixture, $rustDirectory)
    Invoke-RS08Check $goRunner @('rewrite', $fixture, $rustDirectory)
    Invoke-RS08Check $rustRunner @('verify-edited', $fixture, $rustDirectory)

    # A real NTFS directory denial is distinct from a read-only file attribute.
    # Only our fresh synthetic directory is changed, and its original ACL is restored.
    $deniedDirectory = Join-Path $taskDirectory 'write-denied'
    Invoke-RS08Check $goRunner @('emit', $fixture, $deniedDirectory)
    $originalAcl = Get-Acl -LiteralPath $deniedDirectory
    $userSid = [Security.Principal.WindowsIdentity]::GetCurrent().User.Value
    $originalHashes = @(Get-ChildItem -LiteralPath $deniedDirectory -File | Sort-Object Name | Get-FileHash -Algorithm SHA256 | Select-Object -ExpandProperty Hash)
    try {
        & icacls.exe $deniedDirectory /deny "*${userSid}:(WD,AD)" | Out-Null
        if ($LASTEXITCODE -ne 0) { throw 'Could not establish the write-denied directory test.' }
        & $rustRunner rewrite $fixture $deniedDirectory
        if ($LASTEXITCODE -eq 0) { throw 'Rust unexpectedly committed to a write-denied directory.' }
        Invoke-RS08Check $goRunner @('verify', $fixture, $deniedDirectory)
        $afterHashes = @(Get-ChildItem -LiteralPath $deniedDirectory -File | Sort-Object Name | Get-FileHash -Algorithm SHA256 | Select-Object -ExpandProperty Hash)
        if (Compare-Object $originalHashes $afterHashes) { throw 'A denied write changed the original data files.' }
    } finally { Set-Acl -LiteralPath $deniedDirectory -AclObject $originalAcl }
    Write-Output "RS-08 ${Renderer}: Go -> Rust -> Go and Rust -> Go -> Rust passed; NTFS write denial preserved all four files."
} finally {
    Pop-Location
    # Resolve and bound the recursive cleanup before removing only our test root.
    $cleanupPath = [IO.Path]::GetFullPath($taskDirectory)
    if (-not $cleanupPath.StartsWith($temporaryBase + '\', [StringComparison]::OrdinalIgnoreCase) -or
        [IO.Path]::GetFileName($cleanupPath) -ne $directoryName) {
        throw 'Refusing cleanup outside the RS-08 temporary test directory.'
    }
    Remove-Item -LiteralPath $cleanupPath -Recurse -Force
}
