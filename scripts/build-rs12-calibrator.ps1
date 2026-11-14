#Requires -Version 7.4
[CmdletBinding()]
param([Parameter(Mandatory)][string]$OutputDirectory)
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'lib/rust-environment.ps1')
$environment=Initialize-VeloRustEnvironment
$OutputDirectory=$ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($OutputDirectory)
if(Test-Path -LiteralPath $OutputDirectory){throw 'Preserve previous native calibrator builds.'}
New-Item -ItemType Directory -Path $OutputDirectory|Out-Null
$source=Join-Path $PSScriptRoot 'lib/rs12-input-calibrator.cpp'
$exe=Join-Path $OutputDirectory 'rs12-input-calibrator.exe'
& cl.exe /nologo /std:c++20 /O2 /EHsc /W4 /WX /MD /utf-8 $source "/Fe:$exe" "/Fo:$(Join-Path $OutputDirectory 'input-calibrator.obj')" /link user32.lib
if($LASTEXITCODE -ne 0){throw 'Input calibrator build failed.'}
Copy-Item -LiteralPath $source -Destination $OutputDirectory
[ordered]@{BuiltAtUtc=[datetime]::UtcNow.ToString('o');ExecutableSHA256=(Get-FileHash -LiteralPath $exe).Hash;SourceSHA256=(Get-FileHash -LiteralPath $source).Hash;MsvcToolsVersion=$environment.MsvcToolsVersion;WindowsSdkVersion=$environment.WindowsSdkVersion;Scope='Isolated native integration-test input calibration. All generated input explicitly machine-labelled. Exact PID/creation/exe/HWND and foreground/focus required; fixed Unicode a only; no selection is launched.'}|ConvertTo-Json|Set-Content -LiteralPath (Join-Path $OutputDirectory 'identity.json') -Encoding utf8NoBOM
"Native input calibrator built: $exe"
