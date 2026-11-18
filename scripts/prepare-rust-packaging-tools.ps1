#Requires -Version 7.4
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$OutputDirectory,
    [string]$ArchivePath = $env:VELO_NSIS_ARCHIVE
)
$ErrorActionPreference='Stop'
$repository=Split-Path -Parent $PSScriptRoot
$target=(Resolve-Path -LiteralPath (Join-Path $repository 'target')).Path
$OutputDirectory=$ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($OutputDirectory)
if(-not $OutputDirectory.StartsWith($target+'\',[StringComparison]::OrdinalIgnoreCase) -or (Test-Path -LiteralPath $OutputDirectory)){throw 'Use a fresh packaging-tools directory inside target.'}
$identity=Get-Content -LiteralPath (Join-Path $repository 'windows/nsis-toolchain.json') -Raw|ConvertFrom-Json
if($ArchivePath){
    $ArchivePath=(Resolve-Path -LiteralPath $ArchivePath).Path
    if((Get-FileHash -LiteralPath $ArchivePath).Hash -ne $identity.archiveSHA256){throw 'Local NSIS archive checksum differs; no compiler was executed.'}
}
New-Item -ItemType Directory -Path $OutputDirectory|Out-Null
$archive=Join-Path $OutputDirectory 'nsis.zip'
if($ArchivePath){
    Copy-Item -LiteralPath $ArchivePath -Destination $archive
}else{
    $downloadArguments = @(
        '--fail', '--silent', '--show-error', '--location',
        '--proto', '=https', '--proto-redir', '=https',
        '--connect-timeout', '15', '--max-time', '60', '--retry', '2',
        '--output', $archive, '--', $identity.archiveUrl
    )
    & curl.exe @downloadArguments
    if ($LASTEXITCODE -ne 0) { throw 'Pinned NSIS HTTPS download failed. Supply the pinned ZIP with -ArchivePath.' }
}
if((Get-FileHash -LiteralPath $archive).Hash -ne $identity.archiveSHA256){throw 'Downloaded NSIS archive checksum differs; no compiler was executed.'}
Expand-Archive -LiteralPath $archive -DestinationPath (Join-Path $OutputDirectory 'tools')
$compiler=Join-Path $OutputDirectory "tools/nsis-$($identity.version)/makensis.exe"
if((Get-FileHash -LiteralPath $compiler).Hash -ne $identity.compilerSHA256){throw 'Extracted NSIS compiler checksum differs; no compiler was executed.'}
if((& $compiler /VERSION) -ne "v$($identity.version)"){throw 'NSIS compiler version differs.'}
[ordered]@{Version=$identity.version;Compiler=$compiler;CompilerSHA256=$identity.compilerSHA256;ArchiveSHA256=$identity.archiveSHA256;ArchiveSource=$(if($ArchivePath){'VerifiedLocalArchive'}else{'PinnedHTTPSDownload'});GlobalInstallation=$false}|ConvertTo-Json|Set-Content -LiteralPath (Join-Path $OutputDirectory 'tools.json') -Encoding utf8NoBOM
Write-Output $compiler
