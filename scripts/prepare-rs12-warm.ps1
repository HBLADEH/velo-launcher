#Requires -Version 7.4
[CmdletBinding()]
param([Parameter(Mandatory)][string]$PreparedDirectory,[Parameter(Mandatory)][string]$OutputDirectory,[Parameter(Mandatory)][ValidateSet('software','femtovg','wails')][string]$Edition,[ValidateRange(1,3)][int]$Count=3,[string]$WebViewCacheSource)
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'lib/resource-sample.ps1')
$manifest=Read-VeloMetadataJson (Join-Path (Resolve-Path -LiteralPath $PreparedDirectory).Path 'manifest.json')
$OutputDirectory=$ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($OutputDirectory)
if(Test-Path -LiteralPath $OutputDirectory){throw 'Use a new warm-start case directory.'}
if($Edition -eq 'wails' -and (-not $WebViewCacheSource -or -not(Test-Path -LiteralPath $WebViewCacheSource -PathType Container))){throw 'Wails warm startup requires an exited test profile WebView cache; a fresh profile is a different cold-cache dimension.'}
New-Item -ItemType Directory -Path $OutputDirectory|Out-Null
$build=$manifest.Builds.$Edition
$shell=New-Object -ComObject WScript.Shell
$cases=@(foreach($round in 1..$Count){
    $name="$Edition-warm-$round"
    $root=Join-Path $OutputDirectory $name
    New-Item -ItemType Directory -Path (Join-Path $root 'binary'),(Join-Path $root 'local') -Force|Out-Null
    $exe=Join-Path $root ('binary/'+[IO.Path]::GetFileName($build.Executable))
    Copy-Item -LiteralPath $build.Executable -Destination $exe
    if((Get-FileHash -LiteralPath $exe).Hash -ne $build.SHA256){throw 'Warm-start executable copy mismatch.'}
    $data=Join-Path $root 'local/Velo'
    Copy-Item -LiteralPath $manifest.SeedDirectory -Destination $data -Recurse
    $log=Join-Path $root 'input-events.log'
    $arguments=''
    if($Edition -eq 'wails'){
        Copy-Item -LiteralPath $WebViewCacheSource -Destination (Join-Path $data 'webview') -Recurse
        $probe=Join-Path $root 'probe'
        New-Item -ItemType Directory -Path $probe|Out-Null
        $log=Join-Path $probe 'accepted-input.log'
        @{LOCALAPPDATA=(Join-Path $root 'local');VELO_LATENCY_DIR=$probe;VELO_LATENCY_ACTUAL_INPUT='1';VELO_LATENCY_SHA256=$build.SHA256}|ConvertTo-Json|Set-Content -LiteralPath ($exe+'.probe.json') -Encoding utf8NoBOM
    }else{
        $arguments='--instance-id=rs12-'+$name+' --data-dir="'+$data+'" --measurement-business --measurement-log="'+$log+'"'
    }
    $shortcut=$shell.CreateShortcut((Join-Path $OutputDirectory ($name+'.lnk')))
    $shortcut.TargetPath=$exe
    $shortcut.Arguments=$arguments
    $shortcut.WorkingDirectory=Split-Path -Parent $exe
    $shortcut.Save()
    $fingerprints=[ordered]@{}
    foreach($file in 'config.json','index.json','history.json','application-library.json'){$fingerprints[$file]=(Get-FileHash -LiteralPath (Join-Path $data $file)).Hash}
    $inputId=[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes(($fingerprints|ConvertTo-Json -Compress))))
    $case=[ordered]@{Name=$name;Edition=$Edition;Executable=$exe;SHA256=$build.SHA256;DataDirectory=$data;MeasurementLog=$log;InputId=$inputId;Round=$round;Kind='WarmStartup';WebViewCacheSeeded=$Edition -eq 'wails';Scope='Direct Explorer launch from prepared shortcut; actual accepted character ends timing. Normal OS cache retained; manual startup/typing delay is not subtracted.'}
    $case|ConvertTo-Json|Set-Content -LiteralPath (Join-Path $root 'case.json') -Encoding utf8NoBOM
    $case
})
$os=Get-CimInstance Win32_OperatingSystem
$environment=@{Windows=$os.Version;Build=$os.BuildNumber;BootUtc=$os.LastBootUpTime.ToUniversalTime().ToString('o');CPU=@((Get-CimInstance Win32_Processor).Name)}
[ordered]@{Edition=$Edition;Cases=$cases;Environment=$environment;EnvironmentId=[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes(($environment|ConvertTo-Json -Compress))))}|ConvertTo-Json -Depth 10|Set-Content -LiteralPath (Join-Path $OutputDirectory 'warm-plan.json') -Encoding utf8NoBOM
"$Count direct desktop warm-start shortcuts prepared. No program was launched."
