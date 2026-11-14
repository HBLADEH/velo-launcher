#Requires -Version 7.4
[CmdletBinding()]
param([Parameter(Mandatory)][string]$OutputDirectory)
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'lib/rust-environment.ps1')
$environment=Initialize-VeloRustEnvironment
$OutputDirectory=$ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($OutputDirectory)
if(Test-Path -LiteralPath $OutputDirectory){throw 'Use a fresh window recorder build directory.'}
New-Item -ItemType Directory -Path $OutputDirectory|Out-Null
$source=Join-Path $PSScriptRoot 'lib/rs13-window-recorder.cpp';$exe=Join-Path $OutputDirectory 'rs13-window-recorder.exe'
& cl.exe /nologo /std:c++20 /O2 /EHsc /W4 /WX /MD /utf-8 $source "/Fe:$exe" "/Fo:$(Join-Path $OutputDirectory 'recorder.obj')" /link d3d11.lib dxgi.lib user32.lib gdi32.lib dwmapi.lib windowsapp.lib
if($LASTEXITCODE -ne 0){throw 'Window graphics recorder build failed.'}
Copy-Item -LiteralPath $source -Destination (Join-Path $OutputDirectory 'rs13-window-recorder.cpp')
[ordered]@{BuiltAtUtc=[datetime]::UtcNow.ToString('o');ExecutableSHA256=(Get-FileHash $exe).Hash;SourceSHA256=(Get-FileHash $source).Hash;BuilderSHA256=(Get-FileHash $PSCommandPath).Hash;Backend='WindowsGraphicsCapture';Scope='Selected product HWND client inset. No activation, input injection, cursor or other app capture. Surface frame timestamps differ from final desktop and photon presentation. 128 frames/128MiB per named clip.'}|ConvertTo-Json|Set-Content (Join-Path $OutputDirectory 'identity.json') -Encoding utf8NoBOM
'Selected-window graphics recorder built.'
