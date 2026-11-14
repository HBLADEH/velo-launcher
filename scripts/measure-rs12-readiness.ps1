#Requires -Version 7.4
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$PreparedDirectory,
    [Parameter(Mandatory)][ValidateSet('software','femtovg','wails')][string]$Edition,
    [Parameter(Mandatory)][ValidatePattern('^[a-zA-Z0-9-]{1,25}$')][string]$RunName,
    [Parameter(Mandatory)][string]$CalibratorExecutable,
    [ValidateRange(1,3)][int]$StartupSamples=3,
    [ValidateRange(0,20)][int]$HotkeySamples=20,
    [string]$WebViewCacheSource
)
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'lib/resource-sample.ps1')
$PreparedDirectory=(Resolve-Path -LiteralPath $PreparedDirectory).Path
$CalibratorExecutable=(Resolve-Path -LiteralPath $CalibratorExecutable).Path
$calibratorIdentity=Read-VeloMetadataJson (Join-Path (Split-Path -Parent $CalibratorExecutable) 'identity.json')
if((Get-FileHash -LiteralPath $CalibratorExecutable).Hash -ne $calibratorIdentity.ExecutableSHA256){throw 'Native calibrator identity changed.'}
$manifest=Read-VeloMetadataJson (Join-Path $PreparedDirectory 'manifest.json')
$build=$manifest.Builds.$Edition
if((Get-FileHash -LiteralPath $build.Executable).Hash -ne $build.SHA256){throw 'Frozen readiness executable changed.'}
if(@(Get-CimInstance Win32_Process -Filter "Name LIKE 'velo-launcher%' OR Name LIKE 'velo-slint%'").Count){throw 'Exit the previous Velo instance before calibration.'}
if($Edition -eq 'wails' -and -not $WebViewCacheSource){throw 'Warm Wails calibration requires an isolated normally exited WebView cache source.'}
if($WebViewCacheSource){$WebViewCacheSource=(Resolve-Path -LiteralPath $WebViewCacheSource).Path;if($WebViewCacheSource -notlike '*\.reasonix\*'){throw 'Only an isolated test WebView profile may be copied.'}}
$group=Join-Path $PreparedDirectory ('runs/'+$RunName)
if(Test-Path -LiteralPath $group){throw 'Preserve earlier readiness calibration; use a new RunName.'}
New-Item -ItemType Directory -Path $group|Out-Null
$kind=if($Edition -eq 'wails'){'wails'}else{'slint'}
function New-CalibratorInfo([string]$Mode,[string]$Trace,[string]$Ready,[string]$IdentityFile,[string]$Log){
    $info=[Diagnostics.ProcessStartInfo]::new($CalibratorExecutable)
    $info.UseShellExecute=$false;$info.CreateNoWindow=$true;$info.WindowStyle='Hidden'
    foreach($argument in @($Mode,$kind,$IdentityFile,$Log,$build.Executable,$Trace,$Ready)){$info.ArgumentList.Add($argument)}
    $info
}
function Probe-Call($Control,[string]$Route){
    Invoke-RestMethod -Uri ($Control.URL+$Route) -Method Post -Headers @{Authorization='Bearer '+$Control.Token}
}
foreach($round in 1..$StartupSamples){
    $directory=Join-Path $group ('warm-'+$round)
    $local=Join-Path $directory 'local';New-Item -ItemType Directory -Path $local -Force|Out-Null
    $data=Join-Path $local 'Velo';Copy-Item -LiteralPath $manifest.SeedDirectory -Destination $data -Recurse
    if($Edition -eq 'wails'){Copy-Item -LiteralPath $WebViewCacheSource -Destination (Join-Path $data 'webview') -Recurse}
    $config=Read-VeloMetadataJson (Join-Path $data 'config.json')
    if($config.launch_at_startup -or $config.hotkey -ne 'Alt+Space' -or $config.auto_check_updates){throw 'Calibration seed must retain isolated no-startup/no-update Alt+Space preset.'}
    $probeDirectory=Join-Path $directory 'probe';New-Item -ItemType Directory -Path $probeDirectory|Out-Null
    $log=if($Edition -eq 'wails'){Join-Path $probeDirectory 'accepted-input.log'}else{Join-Path $directory 'input-events.log'}
    $identityFile=Join-Path $directory 'target.identity'
    $trace=Join-Path $directory 'startup-calibration.jsonl';$ready=Join-Path $directory 'startup.ready'
    $calibrator=[Diagnostics.Process]::Start((New-CalibratorInfo 'startup' $trace $ready $identityFile $log))
    $process=$null;$completed=$false;$control=$null;$stderr=$null
    try{
        $wait=[Diagnostics.Stopwatch]::StartNew()
        while(-not(Test-Path -LiteralPath $ready)){if($calibrator.HasExited -or $wait.Elapsed.TotalSeconds -ge 15){throw 'Startup calibration did not arm before launch.'};Start-Sleep -Milliseconds 2}
        $start=[Diagnostics.ProcessStartInfo]::new($build.Executable)
        $start.UseShellExecute=$false;$start.CreateNoWindow=$true;$start.WindowStyle='Hidden';$start.RedirectStandardError=$true
        if($Edition -eq 'wails'){
            $start.Environment['LOCALAPPDATA']=$local;$start.Environment['VELO_LATENCY_DIR']=$probeDirectory
            $start.Environment['VELO_LATENCY_ACTUAL_INPUT']='1';$start.Environment['VELO_LATENCY_SHA256']=$build.SHA256
        }else{
            $start.RedirectStandardInput=$true;$start.RedirectStandardOutput=$true
            foreach($argument in @(('--instance-id=rs12-cal-'+$RunName+'-'+$round),('--data-dir='+$data),'--measurement-business','--measurement-stdio',('--measurement-log='+$log))){$start.ArgumentList.Add($argument)}
        }
        $inputs=[ordered]@{};foreach($name in 'config.json','index.json','history.json','application-library.json'){$inputs[$name]=(Get-FileHash -LiteralPath (Join-Path $data $name)).Hash}
        $process=[Diagnostics.Process]::Start($start);$stderr=$process.StandardError.ReadToEndAsync()
        $identityTemporary=$identityFile+'.tmp'
        [IO.File]::WriteAllText($identityTemporary,([string]$process.Id+' '+$process.StartTime.ToUniversalTime().ToFileTimeUtc()),[Text.UTF8Encoding]::new($false))
        [IO.File]::Move($identityTemporary,$identityFile)
        [ordered]@{RecordedAtUtc=[datetime]::UtcNow.ToString('o');Edition=$Edition;ProcessId=$process.Id;StartedTicks=$process.StartTime.ToUniversalTime().Ticks;SHA256=$build.SHA256;DataDirectory=$data;MeasurementLog=$log;Inputs=$inputs;InputSource='Native machine Unicode calibration; separate from physical-key evidence';WarmStartup=$true;WebViewCacheCopied=($Edition -eq 'wails');CalibratorSHA256=$calibratorIdentity.ExecutableSHA256;CalibratorSourceSHA256=$calibratorIdentity.SourceSHA256}|ConvertTo-Json -Depth 8|Set-Content -LiteralPath (Join-Path $directory 'session.json') -Encoding utf8NoBOM
        while(-not $calibrator.WaitForExit(250)){if($wait.Elapsed.TotalSeconds -gt 40){throw 'Readiness calibration exceeded its bound.'}}
        $startupPassed=$calibrator.ExitCode -eq 0
        Write-Output "Readiness startup capture: $Edition round $round exit=$($calibrator.ExitCode)"
        function Command([string]$Text){
            if($process.HasExited){throw 'Test process ended before its command.'}
            $process.StandardInput.WriteLine($Text);$process.StandardInput.Flush()
            $ack=$process.StandardOutput.ReadLineAsync()
            if(-not $ack.Wait(10000) -or $ack.Result -notlike "event=measurement_state command=$Text *"){throw 'Isolated stdio command did not acknowledge.'}
            $ack.Result
        }
        if($Edition -eq 'wails'){
            $controlPath=Join-Path $probeDirectory 'control.json';$controlWait=[Diagnostics.Stopwatch]::StartNew()
            while(-not(Test-Path -LiteralPath $controlPath)){if($process.HasExited -or $controlWait.Elapsed.TotalSeconds -gt 20){throw 'Isolated Wails control unavailable.'};Start-Sleep -Milliseconds 20}
            $control=Read-VeloMetadataJson $controlPath
            $state=Invoke-RestMethod -Uri ($control.URL+'/state') -Headers @{Authorization='Bearer '+$control.Token}
            $scanWait=[Diagnostics.Stopwatch]::StartNew()
            while($state.Status.scanning){if($scanWait.Elapsed.TotalSeconds -gt 90){throw 'Wait for scan completion before hotkey calibration.'};Start-Sleep -Milliseconds 100;$state=Invoke-RestMethod -Uri ($control.URL+'/state') -Headers @{Authorization='Bearer '+$control.Token}}
        }else{
            $scanWait=[Diagnostics.Stopwatch]::StartNew()
            do{$observation=Read-VeloBusinessObservation $log $process.Id;if($scanWait.Elapsed.TotalSeconds -gt 90){throw 'Scan did not settle before calibration.'};Start-Sleep -Milliseconds 100}until($observation.Complete -and -not $observation.Scanning)
        }
        if($round -eq $StartupSamples -and $startupPassed){
            foreach($sample in 1..$HotkeySamples){
                if($HotkeySamples -eq 0){break}
                if($Edition -eq 'wails'){
                    $null=& $CalibratorExecutable clear $kind $identityFile $log $build.Executable (Join-Path $directory ('clear-'+$sample+'.jsonl')) (Join-Path $directory ('clear-'+$sample+'.ready'))
                    if($LASTEXITCODE -ne 0){throw 'Test search clear refused its foreground guard.'}
                    $null=Probe-Call $control '/expire';$null=Probe-Call $control ('/prepare?id=machine-'+$sample)
                }else{$null=Command home;$null=Command hide}
                Start-Sleep -Milliseconds 50
                & $CalibratorExecutable hotkey $kind $identityFile $log $build.Executable (Join-Path $directory ('hotkey-'+$sample+'.jsonl')) (Join-Path $directory ('hotkey-'+$sample+'.ready'))
                $sampleExit=$LASTEXITCODE
                [ordered]@{Number=$sample;NativeExitCode=$sampleExit;MachineInput=$true}|ConvertTo-Json|Set-Content -LiteralPath (Join-Path $directory ('hotkey-'+$sample+'-exit.json')) -Encoding utf8NoBOM
                Write-Output "Readiness hotkey capture: $Edition $sample/$HotkeySamples exit=$sampleExit"
                if($sampleExit -ne 0){throw 'Hotkey calibration failed; preserve trace and inspect focus.'}
            }
        }
        if($Edition -eq 'wails'){$null=Probe-Call $control '/quit'}else{$null=Command quit}
        if(-not $process.WaitForExit(20000) -or $process.ExitCode -ne 0){throw 'Normal isolated calibration shutdown not confirmed.'}
        $completed=$true
    }finally{
        if($process -and -not $process.HasExited){
            try{if($Edition -eq 'wails' -and $control){$null=Probe-Call $control '/quit'}elseif($Edition -ne 'wails'){$process.StandardInput.WriteLine('quit');$process.StandardInput.Flush()}}catch{}
            $null=$process.WaitForExit(20000)
        }
        if($calibrator -and -not $calibrator.HasExited){$null=$calibrator.WaitForExit(20000)}
        if($process){
            $exited=$process.HasExited
            if($exited -and $stderr){$stderr.GetAwaiter().GetResult()|Set-Content -LiteralPath (Join-Path $directory 'stderr.log') -Encoding utf8NoBOM}
            [ordered]@{RecordedAtUtc=[datetime]::UtcNow.ToString('o');Completed=$completed;Exited=$exited;ExitCode=$(if($exited){$process.ExitCode}else{$null});ForcedCleanup=$false;SHA256=$build.SHA256;ExecutableMatches=(Get-FileHash -LiteralPath $build.Executable).Hash -eq $build.SHA256;MachineCalibration=$true}|ConvertTo-Json|Set-Content -LiteralPath (Join-Path $directory 'completion.json') -Encoding utf8NoBOM
            if($exited){$process.Dispose()}
        }
        if($calibrator.HasExited){$calibrator.Dispose()}
    }
}
