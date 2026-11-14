#Requires -Version 7.4
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$PreparedDirectory,
    [Parameter(Mandatory)][ValidateSet('software','femtovg','wails')][string]$Edition,
    [Parameter(Mandatory)][ValidatePattern('^[a-zA-Z0-9-]{1,25}$')][string]$RunName,
    [Parameter(Mandatory)][string]$CalibratorExecutable,
    [Parameter(Mandatory)][string]$FrameObserverExecutable,
    [ValidateRange(3,8)][int]$Queries=5,
    [string]$WebViewCacheSource
)
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'lib/resource-sample.ps1')
$PreparedDirectory=(Resolve-Path -LiteralPath $PreparedDirectory).Path
$CalibratorExecutable=(Resolve-Path -LiteralPath $CalibratorExecutable).Path
$FrameObserverExecutable=(Resolve-Path -LiteralPath $FrameObserverExecutable).Path
$manifest=Read-VeloMetadataJson (Join-Path $PreparedDirectory 'manifest.json')
$build=$manifest.Builds.$Edition
if((Get-FileHash -LiteralPath $build.Executable).Hash -ne $build.SHA256){throw 'Frozen animation executable changed.'}
foreach($helper in @($CalibratorExecutable,$FrameObserverExecutable)){
    $identity=Read-VeloMetadataJson (Join-Path (Split-Path -Parent $helper) 'identity.json')
    if((Get-FileHash -LiteralPath $helper).Hash -ne $identity.ExecutableSHA256){throw 'Animation helper identity changed.'}
}
if(@(Get-CimInstance Win32_Process -Filter "Name LIKE 'velo-launcher%' OR Name LIKE 'velo-slint%'").Count){throw 'One isolated Velo instance at a time.'}
if($Edition -eq 'wails' -and -not $WebViewCacheSource){throw 'Use an isolated normally exited WebView profile.'}
$directory=Join-Path $PreparedDirectory ('runs/'+$RunName)
if(Test-Path -LiteralPath $directory){throw 'Preserve earlier animation samples.'}
New-Item -ItemType Directory -Path (Join-Path $directory 'local') -Force|Out-Null
$data=Join-Path $directory 'local/Velo';Copy-Item -LiteralPath $manifest.SeedDirectory -Destination $data -Recurse
if($Edition -eq 'wails'){
    $WebViewCacheSource=(Resolve-Path -LiteralPath $WebViewCacheSource).Path
    if($WebViewCacheSource -notlike '*\.reasonix\*'){throw 'Only isolated WebView caches may be used.'}
    Copy-Item -LiteralPath $WebViewCacheSource -Destination (Join-Path $data 'webview') -Recurse
}
$config=Read-VeloMetadataJson (Join-Path $data 'config.json')
if($config.launch_at_startup -or $config.auto_check_updates){throw 'Controlled seed must disable startup and update network activity.'}
$probe=Join-Path $directory 'probe';New-Item -ItemType Directory -Path $probe|Out-Null
$log=if($Edition -eq 'wails'){Join-Path $probe 'accepted-input.log'}else{Join-Path $directory 'input-events.log'}
$start=[Diagnostics.ProcessStartInfo]::new($build.Executable)
$start.UseShellExecute=$false;$start.CreateNoWindow=$true;$start.WindowStyle='Hidden';$start.RedirectStandardError=$true
if($Edition -eq 'wails'){
    $start.Environment['LOCALAPPDATA']=Join-Path $directory 'local';$start.Environment['VELO_LATENCY_DIR']=$probe
    $start.Environment['VELO_LATENCY_ACTUAL_INPUT']='1';$start.Environment['VELO_LATENCY_SHA256']=$build.SHA256
}else{
    $start.RedirectStandardInput=$true;$start.RedirectStandardOutput=$true
    foreach($argument in @(('--instance-id=rs12-anim-'+$RunName),('--data-dir='+$data),'--measurement-business','--measurement-stdio',('--measurement-log='+$log))){$start.ArgumentList.Add($argument)}
}
$inputs=[ordered]@{};foreach($name in 'config.json','index.json','history.json','application-library.json'){$inputs[$name]=(Get-FileHash -LiteralPath (Join-Path $data $name)).Hash}
$process=[Diagnostics.Process]::Start($start);$stderr=$process.StandardError.ReadToEndAsync()
$identityFile=Join-Path $directory 'target.identity'
[IO.File]::WriteAllText($identityFile,([string]$process.Id+' '+$process.StartTime.ToUniversalTime().ToFileTimeUtc()),[Text.UTF8Encoding]::new($false))
$kind=if($Edition -eq 'wails'){'wails'}else{'slint'}
$control=$null;$frameProcess=$null;$resourceTask=$null;$complete=$false;$phases=[Collections.Generic.List[object]]::new()
function Command([string]$Text){
    if($process.HasExited){throw 'Animation target exited early.'}
    $process.StandardInput.WriteLine($Text);$process.StandardInput.Flush();$ack=$process.StandardOutput.ReadLineAsync()
    if(-not $ack.Wait(10000) -or $ack.Result -notlike "event=measurement_state command=$Text *"){throw 'Animation stdio command failed.'}
    $ack.Result
}
function Clear-Search([int]$Number){
    if($Edition -eq 'wails'){
        & $CalibratorExecutable clear $kind $identityFile $log $build.Executable (Join-Path $directory ('clear-'+$Number+'.jsonl')) (Join-Path $directory ('clear-'+$Number+'.ready'))
        if($LASTEXITCODE -ne 0){throw 'Animation clear refused its native focus guard.'}
    }else{$null=Command home}
}
try{
    $watch=[Diagnostics.Stopwatch]::StartNew()
    if($Edition -eq 'wails'){
        $controlPath=Join-Path $probe 'control.json'
        while(-not(Test-Path -LiteralPath $controlPath)){if($process.HasExited -or $watch.Elapsed.TotalSeconds -gt 25){throw 'Animation Wails control not ready.'};Start-Sleep -Milliseconds 50}
        $control=Read-VeloMetadataJson $controlPath
        do{$state=Invoke-RestMethod -Uri ($control.URL+'/state') -Headers @{Authorization='Bearer '+$control.Token};if($watch.Elapsed.TotalSeconds -gt 90){throw 'Animation Wails scan did not finish.'};Start-Sleep -Milliseconds 100}while($state.Status.scanning -or -not $state.NativeActive)
    }else{
        do{Start-Sleep -Milliseconds 100;$observation=if(Test-Path -LiteralPath $log){Read-VeloBusinessObservation $log $process.Id}else{$null};if($watch.Elapsed.TotalSeconds -gt 90){throw 'Animation Slint scan did not finish.'}}until($observation -and $observation.Complete -and -not $observation.Scanning)
        $null=Command show;$null=Command home
    }
    [ordered]@{RecordedAtUtc=[datetime]::UtcNow.ToString('o');Edition=$Edition;ProcessId=$process.Id;StartedTicks=$process.StartTime.ToUniversalTime().Ticks;Executable=$build.Executable;SHA256=$build.SHA256;DataDirectory=$data;MeasurementLog=$log;Inputs=$inputs;ExpectedCatalogCount=$manifest.ExpectedCatalogCount;InputSource='Native generated Unicode query; not physical keyboard';CalibratorSHA256=(Get-FileHash -LiteralPath $CalibratorExecutable).Hash;FrameObserverSHA256=(Get-FileHash -LiteralPath $FrameObserverExecutable).Hash}|ConvertTo-Json -Depth 8|Set-Content -LiteralPath (Join-Path $directory 'session.json') -Encoding utf8NoBOM
    $frameDirectory=Join-Path $directory 'frames'
    $frameStart=[Diagnostics.ProcessStartInfo]::new($FrameObserverExecutable)
    $frameStart.UseShellExecute=$false;$frameStart.CreateNoWindow=$true;$frameStart.WindowStyle='Hidden';$frameStart.RedirectStandardOutput=$true;$frameStart.RedirectStandardError=$true
    foreach($argument in @($kind,[string]$process.Id,'120','0',$frameDirectory)){$frameStart.ArgumentList.Add($argument)}
    $frameProcess=[Diagnostics.Process]::Start($frameStart);$frameError=$frameProcess.StandardError.ReadToEndAsync()
    $ready=$frameProcess.StandardOutput.ReadLineAsync()
    if(-not $ready.Wait(15000) -or $ready.Result -notlike 'RS12_FRAME_OBSERVER_READY *'){throw 'Animation frame observer did not arm.'}
    Add-Type -Path (Join-Path $PSScriptRoot 'lib/rs12-resource-observer.cs')
    $resourceTask=[VeloRS12Resources.Observer]::Start($process.Id,$process.StartTime.ToUniversalTime().Ticks,120,20)
    Start-Sleep -Milliseconds 1000
    foreach($number in 1..$Queries){
        Clear-Search $number;Start-Sleep -Milliseconds 700
        $phaseStart=[Diagnostics.Stopwatch]::GetTimestamp()
        & $CalibratorExecutable query $kind $identityFile $log $build.Executable (Join-Path $directory ('query-'+$number+'.jsonl')) (Join-Path $directory ('query-'+$number+'.ready'))
        $exit=$LASTEXITCODE
        Start-Sleep -Milliseconds 700
        $phases.Add([ordered]@{Number=$number;HostStartQpc=$phaseStart;HostEndQpc=[Diagnostics.Stopwatch]::GetTimestamp();NativeExitCode=$exit;MachineInput=$true})
        Write-Output "Animation CPU/frame query: $Edition $number/$Queries exit=$exit"
        if($exit -ne 0){throw 'Animation generated query failed; raw evidence retained.'}
    }
    if($Edition -eq 'wails'){$null=Invoke-RestMethod -Uri ($control.URL+'/quit') -Method Post -Headers @{Authorization='Bearer '+$control.Token}}else{$null=Command quit}
    if(-not $process.WaitForExit(20000) -or $process.ExitCode -ne 0){throw 'Animation normal exit not confirmed.'}
    $complete=$true
}finally{
    if(-not $process.HasExited){try{if($Edition -eq 'wails' -and $control){$null=Invoke-RestMethod -Uri ($control.URL+'/quit') -Method Post -Headers @{Authorization='Bearer '+$control.Token}}elseif($Edition -ne 'wails'){$process.StandardInput.WriteLine('quit');$process.StandardInput.Flush()}}catch{};$null=$process.WaitForExit(20000)}
    if($resourceTask){$capture=$resourceTask.GetAwaiter().GetResult();$capture|ConvertTo-Json -Depth 10|Set-Content -LiteralPath (Join-Path $directory 'cpu-resources.json') -Encoding utf8NoBOM}
    if($frameProcess){$null=$frameProcess.WaitForExit(15000);if($frameProcess.HasExited){$frameError.GetAwaiter().GetResult()|Set-Content -LiteralPath (Join-Path $directory 'frame-errors.log') -Encoding utf8NoBOM;$frameProcess.Dispose()}}
    $phases|ConvertTo-Json -Depth 6|Set-Content -LiteralPath (Join-Path $directory 'phases.json') -Encoding utf8NoBOM
    if($process.HasExited){$stderr.GetAwaiter().GetResult()|Set-Content -LiteralPath (Join-Path $directory 'stderr.log') -Encoding utf8NoBOM}
    [ordered]@{RecordedAtUtc=[datetime]::UtcNow.ToString('o');Completed=$complete;Exited=$process.HasExited;ExitCode=$(if($process.HasExited){$process.ExitCode}else{$null});ForcedCleanup=$false;MachineInput=$true;CalibratorSHA256=(Get-FileHash -LiteralPath $CalibratorExecutable).Hash;ResourceObserverSHA256=(Get-FileHash -LiteralPath (Join-Path $PSScriptRoot 'lib/rs12-resource-observer.cs')).Hash}|ConvertTo-Json|Set-Content -LiteralPath (Join-Path $directory 'completion.json') -Encoding utf8NoBOM
    if($process.HasExited){$process.Dispose()}
}
