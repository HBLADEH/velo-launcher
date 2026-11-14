#Requires -Version 7.4
[CmdletBinding()]
param(
    [Parameter(Mandatory)][ValidateSet('software','femtovg')][string]$Renderer,
    [Parameter(Mandatory)][string]$ReferenceManifest,
    [Parameter(Mandatory)][string]$OutputDirectory,
    [Parameter(Mandatory)][string]$Makensis
)
$ErrorActionPreference='Stop'
$repository=Split-Path -Parent $PSScriptRoot
$target=(Resolve-Path -LiteralPath (Join-Path $repository 'rust/target')).Path
$OutputDirectory=$ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($OutputDirectory)
if(-not $OutputDirectory.StartsWith($target+'\',[StringComparison]::OrdinalIgnoreCase) -or (Test-Path -LiteralPath $OutputDirectory)){throw 'Use a fresh rollback directory inside rust/target.'}
if(@(Get-CimInstance Win32_Process -Filter "Name LIKE 'velo-launcher%' OR Name LIKE 'velo-slint%'").Count){throw 'Exit other Velo programs before the isolated package test.'}
$reference=Get-Content -LiteralPath $ReferenceManifest -Raw|ConvertFrom-Json
$goSource=$reference.Builds.wails.Executable
if((Get-FileHash -LiteralPath $goSource).Hash -ne $reference.Builds.wails.SHA256){throw 'Go reference executable identity differs.'}
foreach($entry in $reference.Inputs.PSObject.Properties){if($entry.Value -and (Get-FileHash -LiteralPath (Join-Path $reference.SeedDirectory $entry.Name)).Hash -ne $entry.Value){throw 'Shared synthetic seed identity differs.'}}
New-Item -ItemType Directory -Path $OutputDirectory|Out-Null
$local=Join-Path $OutputDirectory 'local'
New-Item -ItemType Directory -Path $local|Out-Null
$data=Join-Path $local 'Velo'
Copy-Item -LiteralPath $reference.SeedDirectory -Destination $data -Recurse
$backup=Join-Path $OutputDirectory 'data-backup'
Copy-Item -LiteralPath $data -Destination $backup -Recurse
$userNames=@('config.json','history.json','application-library.json')
$before=@{}
foreach($name in $userNames){$before[$name]=(Get-FileHash -LiteralPath (Join-Path $data $name)).Hash}
function Assert-UserData {
    foreach($name in $userNames){
        if((Get-FileHash -LiteralPath (Join-Path $data $name)).Hash -ne $before[$name] -or
           (Get-FileHash -LiteralPath (Join-Path $backup $name)).Hash -ne $before[$name]){throw 'Package transition changed synthetic config/history/library or its original backup.'}
    }
}
$goPackage=Join-Path $OutputDirectory 'go-package'
New-Item -ItemType Directory -Path $goPackage|Out-Null
Copy-Item -LiteralPath $goSource -Destination (Join-Path $goPackage 'velo-launcher.exe')
$goZip=Join-Path $OutputDirectory 'go-reference.zip'
Compress-Archive -Path (Join-Path $goPackage '*') -DestinationPath $goZip
$install=Join-Path $OutputDirectory '安装目录'
Expand-Archive -LiteralPath $goZip -DestinationPath $install
$go=Join-Path $install 'velo-launcher.exe'
$preview=Join-Path $OutputDirectory 'rust-preview'
& (Join-Path $PSScriptRoot 'package-rust-preview.ps1') -Renderer $Renderer -OutputDirectory $preview
$installerBuild=Join-Path $OutputDirectory 'rust-installer'
& (Join-Path $PSScriptRoot 'package-rust-installer.ps1') -PreviewContent (Join-Path $preview 'content') -OutputDirectory $installerBuild -Makensis $Makensis -ValidationInstallDirectory $install
$installer=(Get-ChildItem -LiteralPath $installerBuild -Filter '*installer.exe').FullName
$phases=@()

function Invoke-GoPhase([string]$name){
    $probe=Join-Path $OutputDirectory $name
    New-Item -ItemType Directory -Path $probe|Out-Null
    $info=[Diagnostics.ProcessStartInfo]::new($go)
    $info.UseShellExecute=$false;$info.CreateNoWindow=$true;$info.WindowStyle='Hidden';$info.RedirectStandardError=$true
    $info.Environment['LOCALAPPDATA']=$local
    $info.Environment['VELO_LATENCY_DIR']=$probe
    $info.Environment['VELO_LATENCY_ACTUAL_INPUT']='1'
    $info.Environment['VELO_LATENCY_SHA256']=$reference.Builds.wails.SHA256
    $info.ArgumentList.Add('--diagnostics')
    $process=[Diagnostics.Process]::Start($info)
    $errors=$process.StandardError.ReadToEndAsync()
    $control=$null
    try{
        $deadline=[datetime]::UtcNow.AddSeconds(25)
        do{
            if($process.HasExited){throw 'Go reference exited before its probe endpoint.'}
            $controlPath=Join-Path $probe 'control.json'
            if(Test-Path -LiteralPath $controlPath){$control=Get-Content -LiteralPath $controlPath -Raw|ConvertFrom-Json;break}
            Start-Sleep -Milliseconds 100
        }while([datetime]::UtcNow -lt $deadline)
        if(-not $control){throw 'Go probe endpoint did not appear.'}
        $headers=@{Authorization=('Bearer '+$control.Token)}
        $state=$null
        do{
            $state=Invoke-RestMethod -NoProxy -Uri ($control.URL+'/state') -Headers $headers
            if($state.Attached -and $state.Status -and $state.Status.count -ge $reference.ExpectedCatalogCount){break}
            Start-Sleep -Milliseconds 100
        }while([datetime]::UtcNow -lt $deadline)
        if(-not $state.Attached -or $state.Status.count -lt $reference.ExpectedCatalogCount){throw 'Go reference did not read the shared catalog.'}
        $null=Invoke-RestMethod -NoProxy -Uri ($control.URL+'/quit') -Headers $headers -Method Post
        if(-not $process.WaitForExit(10000)){throw 'Go reference did not exit normally.'}
        $process.Refresh()
        if($process.ExitCode -ne 0){throw 'Go reference exit failed.'}
        Assert-UserData
        return [ordered]@{Phase=$name;ExecutableSHA256=(Get-FileHash -LiteralPath $go).Hash;ExitCode=$process.ExitCode;CatalogCount=$state.Status.count;UserDataAndBackupUnchanged=$true}
    }finally{
        if(-not $process.HasExited -and $control){$null=Invoke-RestMethod -NoProxy -Uri ($control.URL+'/quit') -Headers @{Authorization=('Bearer '+$control.Token)} -Method Post; $null=$process.WaitForExit(10000)}
        if(-not $process.HasExited){Stop-Process -Id $process.Id; throw 'Incomplete package phase required process cleanup; do not mark passed.'}
        $errors.GetAwaiter().GetResult()|Set-Content -LiteralPath (Join-Path $probe 'stderr.log') -Encoding utf8NoBOM
        $process.Dispose()
    }
}

$phases+=Invoke-GoPhase 'go-before'
$setup=Start-Process -FilePath $installer -ArgumentList "/S /D=$install" -WindowStyle Hidden -PassThru
if(-not $setup.WaitForExit(30000) -or $setup.ExitCode -ne 0){throw 'Rust installer failed during Go-to-Rust transition.'}
$setup.Dispose()
if((Get-FileHash -LiteralPath $go).Hash -ne $reference.Builds.wails.SHA256){throw 'Rust installer replaced the preserved Go program.'}
Assert-UserData
$rust=Join-Path $install 'velo-slint.exe'
$log=Join-Path $OutputDirectory 'rust-events.log'
$info=[Diagnostics.ProcessStartInfo]::new($rust)
$info.UseShellExecute=$false;$info.CreateNoWindow=$true;$info.WindowStyle='Hidden';$info.RedirectStandardError=$true;$info.RedirectStandardOutput=$true;$info.RedirectStandardInput=$true
foreach($argument in @('--background','--diagnostics','--measurement-business','--measurement-stdio','--instance-id=rs14-package-rollback',('--data-dir='+$data),('--measurement-log='+$log))){$info.ArgumentList.Add($argument)}
$process=[Diagnostics.Process]::Start($info)
$errors=$process.StandardError.ReadToEndAsync();$stdout=$process.StandardOutput.ReadToEndAsync()
try{
    $deadline=[datetime]::UtcNow.AddSeconds(25)
    do{
        if($process.HasExited){throw 'Installed Rust program exited before reading data.'}
        $text=if(Test-Path -LiteralPath $log){Get-Content -LiteralPath $log -Raw}else{''}
        if($text -match "event=catalog_published[^`r`n]*count=$($reference.ExpectedCatalogCount)( |`r|`n)"){break}
        Start-Sleep -Milliseconds 100
    }while([datetime]::UtcNow -lt $deadline)
    if($text -notmatch 'event=catalog_published'){throw 'Installed Rust program did not read the shared catalog.'}
    $process.StandardInput.WriteLine('quit');$process.StandardInput.Flush()
    if(-not $process.WaitForExit(10000)){throw 'Installed Rust program did not exit normally.'}
    $process.Refresh()
    if($process.ExitCode -ne 0){throw 'Installed Rust program exit failed.'}
    Assert-UserData
    $phases+=@{Phase='rust-installed';ExecutableSHA256=(Get-FileHash -LiteralPath $rust).Hash;ExitCode=0;SharedCatalogRead=$true;UserDataAndBackupUnchanged=$true}
}finally{
    if(-not $process.HasExited){$process.StandardInput.WriteLine('quit');$process.StandardInput.Flush();$null=$process.WaitForExit(10000)}
    if(-not $process.HasExited){Stop-Process -Id $process.Id;throw 'Incomplete Rust phase required cleanup.'}
    $errors.GetAwaiter().GetResult()|Set-Content -LiteralPath (Join-Path $OutputDirectory 'rust-stderr.log') -Encoding utf8NoBOM
    $stdout.GetAwaiter().GetResult()|Set-Content -LiteralPath (Join-Path $OutputDirectory 'rust-stdout.log') -Encoding utf8NoBOM
    $process.Dispose()
}
$uninstall=Start-Process -FilePath (Join-Path $install 'uninstall.exe') -ArgumentList '/S' -WindowStyle Hidden -PassThru
$null=$uninstall.WaitForExit(30000);$uninstall.Dispose()
$deadline=[datetime]::UtcNow.AddSeconds(20)
while((Test-Path -LiteralPath $rust) -and [datetime]::UtcNow -lt $deadline){Start-Sleep -Milliseconds 200}
if((Test-Path -LiteralPath $rust) -or (Get-FileHash -LiteralPath $go).Hash -ne $reference.Builds.wails.SHA256){throw 'Rust uninstall did not preserve the rollback Go executable.'}
Assert-UserData
$phases+=Invoke-GoPhase 'go-after'
[ordered]@{Passed=$true;Renderer=$Renderer;RecordedAtUtc=[datetime]::UtcNow.ToString('o');Phases=$phases;
    GoPackageSHA256=(Get-FileHash -LiteralPath $goZip).Hash;RustInstallerSHA256=(Get-FileHash -LiteralPath $installer).Hash;UserDataSHA256=$before;
    FormalDataUsed=$false;AdministrativeInstallValidated=$false;Scope='Actual isolated Go portable package -> NSIS-installed Rust -> preserved Go rollback using one shared synthetic data copy and exact original backup. No launches or user edits; read compatibility complements existing Go/Rust write-roundtrip contracts.'
}|ConvertTo-Json -Depth 7|Set-Content -LiteralPath (Join-Path $OutputDirectory 'results.json') -Encoding utf8NoBOM
'Isolated Go -> actual NSIS Rust -> Go package/data rollback passed.'
