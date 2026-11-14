#Requires -Version 7.4
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$PreparedDirectory,
    [Parameter(Mandatory)][ValidateSet('software','femtovg','wails')][string]$Edition,
    [Parameter(Mandatory)][ValidatePattern('^[a-zA-Z0-9-]{1,35}$')][string]$RunName,
    [switch]$DefaultNetwork
)
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'lib/resource-sample.ps1')
$PreparedDirectory=(Resolve-Path -LiteralPath $PreparedDirectory).Path
$manifest=Read-VeloMetadataJson (Join-Path $PreparedDirectory 'manifest.json')
if(@(Get-CimInstance Win32_Process -Filter "Name LIKE 'velo-launcher%' OR Name LIKE 'velo-slint%'").Count){throw 'Exit the previous Velo edition first.'}
$build=$manifest.Builds.$Edition
if((Get-FileHash -LiteralPath $build.Executable).Hash -ne $build.SHA256){throw 'Input measurement binary changed.'}
$run=Join-Path $PreparedDirectory ('runs/'+$RunName)
if(Test-Path -LiteralPath $run){throw 'Use a fresh RunName.'}
New-Item -ItemType Directory -Path (Join-Path $run 'local') -Force|Out-Null
$data=Join-Path $run 'local/Velo'
Copy-Item -LiteralPath $manifest.SeedDirectory -Destination $data -Recurse
if($DefaultNetwork){
    $config=Read-VeloMetadataJson (Join-Path $data 'config.json')
    $config.auto_check_updates=$true
    $config.refresh_minutes=30
    $config|ConvertTo-Json -Depth 20|Set-Content -LiteralPath (Join-Path $data 'config.json') -Encoding utf8NoBOM
}
$log=Join-Path $run 'input-events.log'
$info=[Diagnostics.ProcessStartInfo]::new($build.Executable)
$info.UseShellExecute=$false
$info.CreateNoWindow=$true
$info.WindowStyle='Hidden'
if($Edition -eq 'wails'){
    $probe=Join-Path $run 'probe'
    New-Item -ItemType Directory -Path $probe|Out-Null
    $log=Join-Path $probe 'accepted-input.log'
    $info.Environment['LOCALAPPDATA']=Join-Path $run 'local'
    $info.Environment['VELO_LATENCY_DIR']=$probe
    $info.Environment['VELO_LATENCY_ACTUAL_INPUT']='1'
    $info.Environment['VELO_LATENCY_SHA256']=$build.SHA256
}else{
    foreach($argument in @(('--instance-id=rs12-input-'+$RunName),('--data-dir='+$data),'--measurement-business',('--measurement-log='+$log))){$info.ArgumentList.Add($argument)}
}
$inputs=[ordered]@{}
foreach($name in 'config.json','index.json','history.json','application-library.json','quick-launch.json'){
    $path=Join-Path $data $name
    $inputs[$name]=if(Test-Path -LiteralPath $path){(Get-FileHash -LiteralPath $path).Hash}else{$null}
}
$os=Get-CimInstance Win32_OperatingSystem
$environment=[ordered]@{Windows=$os.Version;Build=$os.BuildNumber;BootUtc=$os.LastBootUpTime.ToUniversalTime().ToString('o');CPU=@((Get-CimInstance Win32_Processor).Name)}
$inputId=[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes(($inputs|ConvertTo-Json -Compress))))
$environmentId=[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes(($environment|ConvertTo-Json -Compress))))
$process=[Diagnostics.Process]::Start($info)
[ordered]@{Edition=$Edition;ProcessId=$process.Id;StartedTicks=$process.StartTime.ToUniversalTime().Ticks;Executable=$build.Executable;SHA256=$build.SHA256;DataDirectory=$data;MeasurementLog=$log;Inputs=$inputs;InputId=$inputId;Environment=$environment;EnvironmentId=$environmentId;DefaultNetwork=$DefaultNetwork.IsPresent;Method='Opt-in actual-input observer; no synthesized input. Manual startup/typing delay remains in the reported bound.'}|ConvertTo-Json -Depth 8|Set-Content -LiteralPath (Join-Path $run 'session.json') -Encoding utf8NoBOM
Write-Output "Input measurement session started: $Edition / $RunName / PID $($process.Id)."
while(-not $process.WaitForExit(1000)){}
[ordered]@{Completed=$process.ExitCode -eq 0;ExitCode=$process.ExitCode;ProcessId=$process.Id;SHA256=$build.SHA256;ForcedCleanup=$false}|ConvertTo-Json|Set-Content -LiteralPath (Join-Path $run 'completion.json') -Encoding utf8NoBOM
$process.Dispose()
