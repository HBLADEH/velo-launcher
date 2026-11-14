#Requires -Version 7.4
[CmdletBinding()]
param([Parameter(Mandatory)][string[]]$InputDirectory,[Parameter(Mandatory)][string]$OutputDirectory)
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'lib/resource-sample.ps1')
$InputDirectory=@($InputDirectory|ForEach-Object{(Resolve-Path -LiteralPath $_).Path})
$OutputDirectory=$ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($OutputDirectory)
if(Test-Path -LiteralPath $OutputDirectory){throw 'Use a fresh export directory.'}
$rows=@(foreach($root in $InputDirectory){foreach($file in Get-ChildItem -LiteralPath $root -Recurse -File -Filter '*-input.json'){
    $report=Read-VeloMetadataJson $file.FullName
    if(-not $report.PSObject.Properties['Result']){continue}
    $identityPath=Join-Path $file.DirectoryName 'session.json'
    $identity=Read-VeloMetadataJson $(if(Test-Path -LiteralPath $identityPath){$identityPath}else{Join-Path $file.DirectoryName 'started.json'})
    $logPath=if($identity.PSObject.Properties['MeasurementLog']){$identity.MeasurementLog}else{$null}
    if(-not $logPath -and (Test-Path -LiteralPath (Join-Path $file.DirectoryName 'case.json'))){
        $logPath=(Read-VeloMetadataJson (Join-Path $file.DirectoryName 'case.json')).MeasurementLog
    }
    $finalEventLoss=$false
    if($logPath -and (Test-Path -LiteralPath $logPath)){
        foreach($line in Read-VeloEventLines $logPath){
            if($line -match ('\bpid='+[regex]::Escape([string]$identity.ProcessId)+'\b') -and $line -match '\bdropped=([1-9][0-9]*)\b'){$finalEventLoss=$true}
        }
    }
    $completionPath=Join-Path $file.DirectoryName 'completion.json'
    if(-not(Test-Path -LiteralPath $completionPath)){$completionPath=Join-Path $file.DirectoryName 'exit.json'}
    $normalExit=$false
    if(Test-Path -LiteralPath $completionPath){
        $completion=Read-VeloMetadataJson $completionPath
        $normalExit=$completion.PSObject.Properties['ExitCode'] -and $null -ne $completion.ExitCode -and $completion.ExitCode -eq 0
        if($completion.PSObject.Properties['ForcedCleanup'] -and $completion.ForcedCleanup){$normalExit=$false}
    }
    $countMode=if($report.PSObject.Properties['CountMode']){$report.CountMode}else{'ValidSamples'}
    $machineInput=$report.PSObject.Properties['MachineGeneratedInput'] -and $report.MachineGeneratedInput
    $nativeEventsDropped=$report.Result.PSObject.Properties['NativeEventsDropped'] -and $report.Result.NativeEventsDropped
    foreach($observation in $report.Result.Observations){
        $valid=$observation.Valid -and $normalExit -and -not $finalEventLoss -and $observation.AcceptedUnixUs -gt $observation.StartLowerUnixUs
        [pscustomobject][ordered]@{
            Kind=$report.Kind;Boundary=$report.Boundary;Valid=$valid.ToString().ToLowerInvariant()
            DurationMilliseconds=$(if($valid){$observation.DurationUpperMilliseconds})
            TimestampUtc=$(if($observation.AcceptedUnixUs -gt 0){[datetime]::UnixEpoch.AddTicks([long]$observation.AcceptedUnixUs*10).ToString('o')}else{$report.RecordedAtUtc})
            BootId=$(if($report.Kind -eq 'ColdStartup'){$report.BootId}else{''});ExecutableSHA256=$report.SHA256;EnvironmentId=$report.EnvironmentId;InputId=$report.InputId
            Method=$(if($machineInput){'NativeMachineCalibrationToAcceptedCharacterUpperBound'}elseif($report.Kind -eq 'HotkeyActivation'){'PhysicalKeyboardToAcceptedCharacterUpperBound'}elseif($report.Kind -eq 'ColdStartup'){'IndependentBootToAcceptedCharacterUpperBound'}else{'ProcessCreationToAcceptedCharacterUpperBound'})
            MachineGeneratedInput=[bool]$machineInput;CountsAsPhysicalHotkey=($report.Kind -eq 'HotkeyActivation' -and -not $machineInput)
            Evidence=(Split-Path -Leaf $root)+'/'+$file.FullName.Substring($root.Length+1).Replace('\','/');ProcessId=$identity.ProcessId
            Reason=$(if($finalEventLoss){'Final process log reported event loss; attempt retained as invalid.'}elseif(-not $normalExit){'Normal exit was not confirmed; incomplete/abnormal attempt retained as invalid.'}else{$observation.Reason});ClockUncertaintyMilliseconds=$(if($report.Kind -eq 'HotkeyActivation'){$observation.ClockUncertaintyMilliseconds}else{$null});StartupAddedGuardMilliseconds=$(if($report.Kind -ne 'HotkeyActivation'){1.0}else{$null});InjectedStart=$observation.InjectedStart
            CountMode=$countMode;NativeEventsDropped=$nativeEventsDropped
            ReliabilityAssessment='Not established by accepted-character count; inspect operator reports and full attempt/native-event capture separately.'
            ClockScope=$(if($report.Kind -eq 'HotkeyActivation'){'Windows keyboard-event quantum plus observed UTC/monotonic consistency; manual typing delay retained.'}else{'Reported 1ms is an added guard, not an independently calibrated startup error bound. UTC steps and manual delay remain limitations.'})
        }
    }
}})
if(-not $rows.Count){throw 'No actual-input records exist.'}
New-Item -ItemType Directory -Path $OutputDirectory|Out-Null
$csv=Join-Path $OutputDirectory 'desktop-latency.csv'
$rows|Export-Csv -LiteralPath $csv -Encoding utf8NoBOM -NoTypeInformation
$null=& (Join-Path $PSScriptRoot 'summarize-latency.ps1') -InputPath $csv -OutputPath (Join-Path $OutputDirectory 'latency-summary.json')
[ordered]@{Rows=$rows.Count;Valid=@($rows|Where-Object Valid -eq 'true').Count;Method='Observed conservative bounds ending at actual accepted characters. Manual typing delay remains. Cold conditions are never inferred from warm sessions.'}|ConvertTo-Json
