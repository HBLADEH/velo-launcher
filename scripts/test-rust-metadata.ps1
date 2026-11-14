#Requires -Version 7.4
[CmdletBinding()]
param(
    [Parameter(Mandatory)][ValidateSet('software','femtovg')][string]$Renderer,
    [Parameter(Mandatory)][string]$Executable,
    [Parameter(Mandatory)][string]$OutputDirectory
)
$ErrorActionPreference='Stop'
$repository=Split-Path -Parent $PSScriptRoot
$Executable=(Resolve-Path -LiteralPath $Executable).Path
if(Test-Path -LiteralPath $OutputDirectory){throw 'Use a fresh PE metadata evidence directory.'}
$cargo=Get-Content -LiteralPath (Join-Path $repository 'rust/Cargo.toml') -Raw
$version=[regex]::Match($cargo,'(?m)^version = "(\d+\.\d+\.\d+)"$').Groups[1].Value
$info=[Diagnostics.FileVersionInfo]::GetVersionInfo($Executable)
if(-not $version -or $info.ProductVersion -ne $version -or $info.FileVersion -ne $version -or $info.OriginalFilename -ne 'velo-slint.exe' -or $info.FileDescription -notmatch [regex]::Escape($Renderer)){
    throw 'PE version/renderer metadata differs from Cargo or product identity.'
}
New-Item -ItemType Directory -Path $OutputDirectory|Out-Null
$manifest=Join-Path $OutputDirectory 'app.manifest'
& mt.exe -nologo "-inputresource:$Executable;#1" "-out:$manifest" *> (Join-Path $OutputDirectory 'manifest-extract.log')
if($LASTEXITCODE){throw 'Cannot extract embedded application manifest.'}
[xml]$xml=Get-Content -LiteralPath $manifest -Raw
$manager=[Xml.XmlNamespaceManager]::new($xml.NameTable)
$manager.AddNamespace('a','urn:schemas-microsoft-com:asm.v1')
$manager.AddNamespace('t','urn:schemas-microsoft-com:asm.v3')
$manager.AddNamespace('d','http://schemas.microsoft.com/SMI/2016/WindowsSettings')
$level=$xml.SelectSingleNode('//t:requestedExecutionLevel',$manager)
if($xml.SelectSingleNode('/a:assembly/a:assemblyIdentity',$manager).version -ne "$version.0" -or $level.level -ne 'asInvoker' -or $level.uiAccess -ne 'false' -or
    $xml.SelectSingleNode('//d:dpiAwareness',$manager).InnerText -ne 'PerMonitorV2' -or $xml.SelectSingleNode('//d:longPathAware',$manager).InnerText -ne 'true'){
    throw 'Embedded manifest version, privilege or DPI settings differ.'
}
$headers=& dumpbin.exe /nologo /headers $Executable
if($LASTEXITCODE -or ($headers -join "`n") -notmatch 'subsystem \(Windows GUI\)'){throw 'Release binary is not a Windows GUI subsystem executable.'}
$dependencies=& dumpbin.exe /nologo /dependents $Executable
if($LASTEXITCODE){throw 'Cannot inspect PE dependencies.'}
$dependencies|Set-Content -LiteralPath (Join-Path $OutputDirectory 'dependencies.txt') -Encoding utf8NoBOM
if(($dependencies -join "`n") -match '(?i)WebView2Loader|wails|VCRUNTIME\d|MSVCP\d'){throw 'Unexpected WebView or dynamic VC runtime dependency.'}
$processInfo=[Diagnostics.ProcessStartInfo]::new($Executable)
$processInfo.UseShellExecute=$false;$processInfo.CreateNoWindow=$true;$processInfo.WindowStyle='Hidden';$processInfo.RedirectStandardOutput=$true
$processInfo.ArgumentList.Add('--version')
$process=[Diagnostics.Process]::Start($processInfo)
$text=$process.StandardOutput.ReadToEnd()
$process.WaitForExit()
if($process.ExitCode -ne 0 -or $text -notmatch "^velo-slint $([regex]::Escape($version)) slint=1\.18\.1 backend=winit renderer=$Renderer target=x86_64-pc-windows-msvc"){throw 'Command-line version differs from PE/Cargo.'}
$process.Dispose()
[ordered]@{Passed=$true;Version=$version;Renderer=$Renderer;ExecutableSHA256=(Get-FileHash -LiteralPath $Executable).Hash;
    ProductVersion=$info.ProductVersion;FileVersion=$info.FileVersion;ManifestSHA256=(Get-FileHash -LiteralPath $manifest).Hash;
    ManifestAsInvoker=$true;PerMonitorV2=$true;LongPathAware=$true;NoWebViewOrDynamicVCRuntimeImports=$true;
    Scope='Actual PE resources, CLI identity and direct import table. Windows 10/other drivers still require execution on those systems.'
}|ConvertTo-Json|Set-Content -LiteralPath (Join-Path $OutputDirectory 'result.json') -Encoding utf8NoBOM
'PE version, renderer, DPI/privilege manifest, subsystem and direct imports verified.'
