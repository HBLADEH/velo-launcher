#Requires -Version 7.4
[CmdletBinding()]
param([Parameter(Mandatory)][string]$OutputDirectory)
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'lib/rust-environment.ps1')
$environment=Initialize-VeloRustEnvironment
$OutputDirectory=$ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($OutputDirectory)
if(Test-Path -LiteralPath $OutputDirectory){throw 'Use a fresh observer build directory; preserve earlier identities.'}
New-Item -ItemType Directory -Path $OutputDirectory|Out-Null
$source=Join-Path $PSScriptRoot 'lib/rs12-frame-observer.cpp'
$exe=Join-Path $OutputDirectory 'rs12-frame-observer.exe'
& cl.exe /nologo /std:c++20 /O2 /EHsc /W4 /WX /MD /utf-8 $source "/Fe:$exe" "/Fo:$(Join-Path $OutputDirectory 'observer.obj')" /link d3d11.lib dxgi.lib user32.lib gdi32.lib
if($LASTEXITCODE -ne 0){throw 'Native frame observer build failed.'}
Copy-Item -LiteralPath $source -Destination (Join-Path $OutputDirectory 'rs12-frame-observer.cpp')
[ordered]@{BuiltAtUtc=[datetime]::UtcNow.ToString('o');ExecutableSHA256=(Get-FileHash -LiteralPath $exe).Hash;SourceSHA256=(Get-FileHash -LiteralPath $source).Hash;MsvcToolsVersion=$environment.MsvcToolsVersion;WindowsSdkVersion=$environment.WindowsSdkVersion;CompilerFileVersion=$environment.CompilerFileVersion;Scope='External read-only QPC/Desktop Duplication observer; no product dependency or injected input. Captured desktop crops are not photon presentation timestamps.'}|ConvertTo-Json|Set-Content -LiteralPath (Join-Path $OutputDirectory 'identity.json') -Encoding utf8NoBOM
Write-Output "Native RS-12 frame observer built: $exe"
