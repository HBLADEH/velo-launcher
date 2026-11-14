#Requires -Version 7.4
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$PreparedDirectory,
    [Parameter(Mandatory)][ValidateSet('software','femtovg','wails')][string]$Edition,
    [Parameter(Mandatory)][ValidatePattern('^[a-zA-Z0-9-]{1,35}$')][string]$RunName,
    [string]$FrameRecorderExecutable
)
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'lib/resource-sample.ps1')
# PreparedDirectory uses the RS-12 shared-seed/build manifest schema; the original
# measurement binaries and data are copied/read only, never overwritten.
$PreparedDirectory=(Resolve-Path -LiteralPath $PreparedDirectory).Path
$manifest=Read-VeloMetadataJson (Join-Path $PreparedDirectory 'manifest.json')
if(@(Get-CimInstance Win32_Process -Filter "Name LIKE 'velo-launcher%' OR Name LIKE 'velo-slint%' OR Name LIKE 'rs13_visual%' OR Name LIKE 'rs13_animation%'").Count){throw 'Exit the previous Velo edition first.'}
$build=$manifest.Builds.$Edition
if((Get-FileHash -LiteralPath $build.Executable).Hash -ne $build.SHA256){throw 'Visual session binary differs from its manifest.'}
if($FrameRecorderExecutable){
    $FrameRecorderExecutable=(Resolve-Path -LiteralPath $FrameRecorderExecutable).Path
    $recorderIdentity=Get-Content -Raw -LiteralPath (Join-Path (Split-Path -Parent $FrameRecorderExecutable) 'identity.json')|ConvertFrom-Json
    if((Get-FileHash -LiteralPath $FrameRecorderExecutable).Hash -ne $recorderIdentity.ExecutableSHA256){throw 'Frame recorder identity changed.'}
}
foreach($source in $manifest.Inputs.PSObject.Properties){
    if($source.Value -and (Get-FileHash -LiteralPath (Join-Path $manifest.SeedDirectory $source.Name)).Hash -ne $source.Value){throw 'Shared visual seed changed after preparation.'}
}
$run=Join-Path $PreparedDirectory ('runs/'+$RunName)
if($manifest.SeedDirectory.StartsWith((Join-Path $env:LOCALAPPDATA 'Velo')+'\',[StringComparison]::OrdinalIgnoreCase) -or
    $manifest.SeedDirectory.TrimEnd('\') -eq (Join-Path $env:LOCALAPPDATA 'Velo') -or
    $run.StartsWith((Join-Path $env:LOCALAPPDATA 'Velo')+'\',[StringComparison]::OrdinalIgnoreCase)){throw 'Visual sessions must stay outside formal Velo data.'}
if(Test-Path -LiteralPath $run){throw 'Use a fresh RunName.'}
New-Item -ItemType Directory -Path (Join-Path $run 'local') -Force|Out-Null
$data=Join-Path $run 'local/Velo'
Copy-Item -LiteralPath $manifest.SeedDirectory -Destination $data -Recurse

$log=Join-Path $run 'input-events.log'
$info=[Diagnostics.ProcessStartInfo]::new($build.Executable)
$info.UseShellExecute=$false
$info.CreateNoWindow=$true
$info.WindowStyle='Hidden'
$info.RedirectStandardError=$true
if($Edition -eq 'wails'){
    $probe=Join-Path $run 'probe'
    New-Item -ItemType Directory -Path $probe|Out-Null
    $log=Join-Path $probe 'accepted-input.log'
    $info.Environment['LOCALAPPDATA']=Join-Path $run 'local'
    $info.Environment['VELO_LATENCY_DIR']=$probe
    $info.Environment['VELO_LATENCY_ACTUAL_INPUT']='1'
    $info.Environment['VELO_LATENCY_SHA256']=$build.SHA256
}else{
    $info.RedirectStandardInput=$true
    $info.RedirectStandardOutput=$true
    foreach($argument in @(('--instance-id=rs13-visual-'+$RunName),('--data-dir='+$data),'--measurement-business',('--measurement-log='+$log))){$info.ArgumentList.Add($argument)}
    $info.ArgumentList.Add('--measurement-stdio')
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
$info.ArgumentList.Add('--diagnostics')
$process=[Diagnostics.Process]::Start($info)
$stderrTask=$process.StandardError.ReadToEndAsync()
try {
[ordered]@{Edition=$Edition;ProcessId=$process.Id;StartedTicks=$process.StartTime.ToUniversalTime().Ticks;Executable=$build.Executable;SHA256=$build.SHA256;DataDirectory=$data;MeasurementLog=$log;Inputs=$inputs;InputId=$inputId;Environment=$environment;EnvironmentId=$environmentId;Diagnostics=$true;BlurHideDisabled=($Edition -eq 'wails');Method='RS-13 native visual session with isolated shared seed. Wails diagnostics suppress blur-hide; Slint diagnostics add logging and retain normal blur-hide. Computer Use input is synthetic, not physical IME or latency evidence.'}|ConvertTo-Json -Depth 8|Set-Content -LiteralPath (Join-Path $run 'session.json') -Encoding utf8NoBOM
Write-Output "RS-13 visual session started: $Edition / $RunName / PID $($process.Id)."
$frameProcess=$null
$resourceChunks=[Collections.Generic.List[object]]::new()
$resourceLimitReached=$false
if($FrameRecorderExecutable){
    # The process exists before Slint has created and shown its HWND. Attaching
    # immediately can race native startup, so wait for the normal lifecycle log.
    $readyWatch=[Diagnostics.Stopwatch]::StartNew()
    do {
        if($process.HasExited){throw 'Visual target exited before native readiness.'}
        $readyText=if(Test-Path -LiteralPath $log){(Read-VeloEventLines $log) -join "`n"}else{''}
        $nativeReady=if($Edition -eq 'wails'){
            $applicationLog=Join-Path $data 'logs/velo.log'
            (Test-Path -LiteralPath $applicationLog) -and ((Get-Content -Raw -LiteralPath $applicationLog) -match 'frontend interactive')
        }else{$readyText -match 'event=lifecycle_shown '}
        if(-not $nativeReady){Start-Sleep -Milliseconds 100}
        if($readyWatch.Elapsed.TotalSeconds -gt 25){throw 'Visual target did not reach native readiness.'}
    }until($nativeReady)
    $frameDirectory=Join-Path $run 'native-frames'
    $frameStart=[Diagnostics.ProcessStartInfo]::new($FrameRecorderExecutable)
    $frameStart.UseShellExecute=$false;$frameStart.CreateNoWindow=$true;$frameStart.WindowStyle='Hidden'
    $frameStart.RedirectStandardOutput=$true;$frameStart.RedirectStandardError=$true
    foreach($argument in @($(if($Edition -eq 'wails'){'wails'}else{'slint'}),[string]$process.Id,'900','128',$frameDirectory)){$frameStart.ArgumentList.Add($argument)}
    $frameProcess=[Diagnostics.Process]::Start($frameStart)
    $frameErrors=$frameProcess.StandardError.ReadToEndAsync()
    $frameReady=$frameProcess.StandardOutput.ReadLineAsync()
    if(-not $frameReady.Wait(15000) -or $frameReady.Result -notlike 'RS13_FRAME_RECORDER_READY *'){throw 'Native visual recorder did not arm.'}
    Add-Type -Path (Join-Path $PSScriptRoot 'lib/rs12-resource-observer.cs')
    $resourceTask=[VeloRS12Resources.Observer]::Start($process.Id,$process.StartTime.ToUniversalTime().Ticks,120,20)
    Write-Output 'Native recording armed; write a fresh clip name to native-frames/clip.txt before each interaction.'
}
$commandPath=Join-Path $run 'command.txt'
$lastRequest=''
while(-not $process.WaitForExit(250)){
    if($frameProcess -and $resourceTask -and $resourceTask.IsCompleted){
        $resourceChunks.Add($resourceTask.GetAwaiter().GetResult())
        if($resourceChunks.Count -ge 16){
            $resourceLimitReached=$true;$resourceTask=$null
            Write-Output 'Bounded CPU observation finished; the interactive test remains open.'
        }else{$resourceTask=[VeloRS12Resources.Observer]::Start($process.Id,$process.StartTime.ToUniversalTime().Ticks,120,20)}
    }
    if($Edition -eq 'wails' -or -not (Test-Path -LiteralPath $commandPath)){continue}
    $request=(Get-Content -Raw -LiteralPath $commandPath).Trim()
    if(-not $request -or $request -eq $lastRequest){continue}
    if($request -notmatch '^[a-zA-Z0-9-]{1,40} (show|home|settings|status|query-editor|query-chinese|quit)$'){throw 'Invalid fixed visual-session command.'}
    $command=$Matches[1]
    $process.StandardInput.WriteLine($command);$process.StandardInput.Flush()
    $ack=$process.StandardOutput.ReadLineAsync()
    if(-not $ack.Wait(10000) -or $ack.Result -notlike "event=measurement_state command=$command *"){throw 'Visual-session component command failed.'}
    [ordered]@{RecordedAtUtc=[datetime]::UtcNow.ToString('o');Request=$request;Response=$ack.Result;Synthetic=$true}|ConvertTo-Json -Compress|Add-Content -LiteralPath (Join-Path $run 'commands.jsonl') -Encoding utf8NoBOM
    $lastRequest=$request
}
[ordered]@{Completed=$process.ExitCode -eq 0;ExitCode=$process.ExitCode;ProcessId=$process.Id;SHA256=$build.SHA256;ForcedCleanup=$(if($process.ExitCode -eq 0){$false}else{$null});CleanupScope='This wrapper never terminates a process. External cleanup records take precedence on nonzero exit.'}|ConvertTo-Json|Set-Content -LiteralPath (Join-Path $run 'completion.json') -Encoding utf8NoBOM
if($frameProcess){
    if(-not $frameProcess.WaitForExit(15000)){throw 'Native recorder did not finish after target exit.'}
    $frameErrors.GetAwaiter().GetResult()|Set-Content -LiteralPath (Join-Path $run 'frame-errors.log') -Encoding utf8NoBOM
    if($resourceTask){$resourceChunks.Add($resourceTask.GetAwaiter().GetResult())}
    $resourceChunks.ToArray()|ConvertTo-Json -Depth 10|Set-Content -LiteralPath (Join-Path $run 'native-resources.json') -Encoding utf8NoBOM
    [ordered]@{ExitCode=$frameProcess.ExitCode;RecorderExecutableSHA256=$recorderIdentity.ExecutableSHA256;TargetSHA256=$build.SHA256;ResourceLimitReached=$resourceLimitReached;Scope='Named selected-window/desktop inset clips and bounded full-process-tree CPU observations; not stable idle or first-ready timing. Read recorder identity for the actual capture backend.'}|ConvertTo-Json|Set-Content -LiteralPath (Join-Path $run 'native-recording.json') -Encoding utf8NoBOM
    $frameProcess.Dispose()
}
} finally {
    if(-not $process.HasExited){
        if($Edition -ne 'wails'){
            $process.StandardInput.WriteLine('quit');$process.StandardInput.Flush()
        }else{
            $controlPath=Join-Path $run 'probe/control.json'
            if(Test-Path -LiteralPath $controlPath){
                $control=Get-Content -Raw -LiteralPath $controlPath|ConvertFrom-Json
                try {$null=Invoke-RestMethod -NoProxy -Uri ($control.URL+'/quit') -Method Post -Headers @{Authorization=('Bearer '+$control.Token)}}
                catch {Write-Warning 'Normal cleanup request not accepted; original failure and process identity are retained.'}
            }
        }
        $null=$process.WaitForExit(10000)
        [ordered]@{Exited=$process.HasExited;NormalQuitRequested=$true;Reason='Session cleanup after an incomplete observation; retain original failure.'}|ConvertTo-Json|Set-Content -LiteralPath (Join-Path $run 'incomplete-cleanup.json') -Encoding utf8NoBOM
    }
    if($process.HasExited){$stderrTask.GetAwaiter().GetResult()|Set-Content -LiteralPath (Join-Path $run 'stderr.log') -Encoding utf8NoBOM}
    $process.Dispose()
}
