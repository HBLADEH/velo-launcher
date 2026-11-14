#Requires -Version 7.4
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'lib/resource-sample.ps1')
Add-Type -Path (Join-Path $PSScriptRoot 'lib/rs12-input-observer.cs')
Add-Type -Path (Join-Path $PSScriptRoot 'lib/rs12-window-observer.cs')
function Assert($Condition,[string]$Message){if(-not $Condition){throw $Message}}
$environmentId=Get-VeloColdEnvironmentId -WindowsVersion 'test-version' -WindowsBuild 'test-build' -Processors @('CPU B','CPU A')
$reorderedEnvironmentId=Get-VeloColdEnvironmentId -WindowsVersion 'test-version' -WindowsBuild 'test-build' -Processors @('CPU A','CPU B')
$changedEnvironmentId=Get-VeloColdEnvironmentId -WindowsVersion 'test-version' -WindowsBuild 'changed-build' -Processors @('CPU A','CPU B')
Assert ($environmentId -eq '0E3ADA4408EA948256EBE7F61FFF24E6A76CE50E35272C5D534CB5BC1A138C40' -and $environmentId -eq $reorderedEnvironmentId) 'Cold environment IDs must use stable field and CPU ordering.'
Assert ($environmentId -ne $changedEnvironmentId) 'A changed Windows build must change the cold environment ID.'
$windowBefore=[VeloRS12Window.State]::new();$windowBefore.DPI=96;$windowBefore.Width=640
$windowAfter=[VeloRS12Window.State]::new();$windowAfter.DPI=96;$windowAfter.Width=640;$windowAfter.UnixMicroseconds=500
Assert (-not [VeloRS12Window.Observer]::Changed($windowBefore,$windowAfter)) 'Clock movement alone must not create a window-state transition.'
$windowAfter.DPI=144
Assert ([VeloRS12Window.Observer]::Changed($windowBefore,$windowAfter)) 'Native DPI must be observed even while hidden and before resize.'
$windowAfter.DPI=96;$windowAfter.Visible=$true
Assert ([VeloRS12Window.Observer]::Changed($windowBefore,$windowAfter)) 'Visibility/focus changes must not be collapsed into a steady state.'
Assert ([VeloRS12Input.Observer]::EventLowerBound(1000000,20,4294967290,16000) -eq 958000) 'Unsigned keyboard event rollover must retain the conservative clock quantum.'
$accepted='event=input_accepted pid=42 unix_us=1000000 dropped=0 focused=true native_focused=true visible=true composing=false synthetic=false utf8_bytes=1'
Assert ([VeloRS12Input.Observer]::Accepted([VeloRS12Input.Observer]::Parse($accepted),42)) 'Actual focused non-composing input must be accepted.'
foreach($bad in @('native_focused=false','visible=false','composing=true','synthetic=true','pid=43','dropped=1','utf8_bytes=0')){
    $name=$bad.Split('=')[0]
    $changed=[regex]::Replace($accepted,('\b'+$name+'=[^ ]+'),$bad)
    Assert (-not [VeloRS12Input.Observer]::Accepted([VeloRS12Input.Observer]::Parse($changed),42)) "Reject incomplete input evidence: $bad"
}
Assert (-not [VeloRS12Input.Observer]::Accepted([VeloRS12Input.Observer]::Parse($accepted+' trusted=false'),42)) 'Untrusted browser input must not substitute for native input.'
Assert (-not [VeloRS12Input.Observer]::Accepted([VeloRS12Input.Observer]::Parse($accepted.Replace(' utf8_bytes=1','')),42)) 'A focus-only marker without actual character bytes is not an input observation.'
$diagnostic=[VeloRS12Input.Result]::new()
$pending=[VeloRS12Input.Observation]::new()
$pending.StartLowerUnixUs=1000000
$pending.Kind='HotkeyActivation'
[VeloRS12Input.Observer]::EndPending($diagnostic,[ref]$pending,'Retried before input')
Assert ($null -eq $pending -and $diagnostic.Observations.Count -eq 1 -and -not $diagnostic.Observations[0].Valid -and $diagnostic.Observations[0].AcceptedUnixUs -eq 0) 'A retry must retain the incomplete earlier activation without inventing an input time.'
foreach($number in 1..20){$observation=[VeloRS12Input.Observation]::new();$observation.Valid=$true;$diagnostic.Observations.Add($observation)}
Assert ([VeloRS12Input.Observer]::CountReached($diagnostic.Observations,20,'ValidSamples')) 'Twenty accepted observations should meet only the supplied sample threshold.'
Assert ([VeloRS12Input.Observer]::CountReached($diagnostic.Observations,5,'PhysicalStarts')) 'An attempt-bounded diagnostic includes failed attempts in its stop count.'
Assert (-not [VeloRS12Input.Observer]::CountReached($diagnostic.Observations,5,'UntilExit')) 'Session diagnostics must keep running beyond an IME failure and the nominal attempt count.'
$pending=[VeloRS12Input.Observation]::new()
[VeloRS12Input.Observer]::EndPending($diagnostic,[ref]$pending,'Exit before input')
Assert ($diagnostic.Observations.Count -eq 22 -and -not $diagnostic.Observations[21].Valid) 'Exiting with a pending activation must keep its failure.'
foreach($path in @('collect-rs12-input.ps1','observe-rs12-window.ps1','test-rs12-input.ps1','prepare-rs12-input.ps1','start-rs12-input-session.ps1','export-rs12-input.ps1','prepare-rs12-warm.ps1','collect-rs12-warm.ps1','collect-rs12-cold.ps1')){
    $tokens=$null;$errors=$null
    [void][System.Management.Automation.Language.Parser]::ParseFile((Join-Path $PSScriptRoot $path),[ref]$tokens,[ref]$errors)
    Assert (-not $errors) "Syntax errors: $path"
}
$fixture=Join-Path ([IO.Path]::GetTempPath()) ('velo-rs12-input-check-'+[guid]::NewGuid().ToString('N'))
try {
    New-Item -ItemType Directory -Path (Join-Path $fixture 'records/case') -Force|Out-Null
    @{ProcessId=42}|ConvertTo-Json|Set-Content -LiteralPath (Join-Path $fixture 'records/case/session.json')
    @{ExitCode=0;ForcedCleanup=$false}|ConvertTo-Json|Set-Content -LiteralPath (Join-Path $fixture 'records/case/completion.json')
    @{Kind='HotkeyActivation';Boundary='KeyDownToEditable';SHA256=('a'*64);EnvironmentId='synthetic-environment';InputId='synthetic-input';RecordedAtUtc='2026-11-04T00:00:00Z';Result=@{Observations=@(
        @{Valid=$true;StartLowerUnixUs=1000000;AcceptedUnixUs=1012000;DurationUpperMilliseconds=12;ClockUncertaintyMilliseconds=1;InjectedStart=$false;Reason='synthetic valid observation'},
        @{Valid=$false;StartLowerUnixUs=2000000;AcceptedUnixUs=0;DurationUpperMilliseconds=0;ClockUncertaintyMilliseconds=1;InjectedStart=$false;Reason='synthetic timeout retained'}
    )}}|ConvertTo-Json -Depth 8|Set-Content -LiteralPath (Join-Path $fixture 'records/case/physical-input.json')
    $null=& (Join-Path $PSScriptRoot 'export-rs12-input.ps1') -InputDirectory (Join-Path $fixture 'records') -OutputDirectory (Join-Path $fixture 'export')
    $rows=@(Import-Csv -LiteralPath (Join-Path $fixture 'export/desktop-latency.csv'))
    $summary=Get-Content -LiteralPath (Join-Path $fixture 'export/latency-summary.json') -Raw|ConvertFrom-Json
    Assert ($rows.Count -eq 2 -and $rows[0].Valid -eq 'true' -and $rows[1].Valid -eq 'false' -and -not $rows[1].DurationMilliseconds) 'Export must retain the invalid timeout without creating a duration.'
    Assert ($rows[0].CountMode -eq 'ValidSamples' -and $rows[0].ReliabilityAssessment -like 'Not established*') 'Older accepted-character records must not be upgraded to reliability approval.'
    Assert ($summary.Groups.ValidSamples -eq 1 -and $summary.Groups.InvalidSamples -eq 1 -and -not $summary.AllSampleCountsSatisfied) 'Too few supplied samples must not be reported as a complete latency matrix.'
    $diagnosticFile=Join-Path $fixture 'records/case/physical-input.json'
    $diagnosticReport=Get-Content -LiteralPath $diagnosticFile -Raw|ConvertFrom-Json
    $diagnosticReport|Add-Member -NotePropertyName CountMode -NotePropertyValue UntilExit
    $diagnosticReport.Result|Add-Member -NotePropertyName NativeEventsDropped -NotePropertyValue $true
    $diagnosticReport|ConvertTo-Json -Depth 8|Set-Content -LiteralPath $diagnosticFile
    $null=& (Join-Path $PSScriptRoot 'export-rs12-input.ps1') -InputDirectory (Join-Path $fixture 'records') -OutputDirectory (Join-Path $fixture 'export-diagnostic')
    $diagnosticRows=@(Import-Csv -LiteralPath (Join-Path $fixture 'export-diagnostic/desktop-latency.csv'))
    Assert ($diagnosticRows[0].CountMode -eq 'UntilExit' -and $diagnosticRows[0].NativeEventsDropped -eq 'True' -and $diagnosticRows[0].ReliabilityAssessment -like 'Not established*') 'Native-event truncation and stop mode must survive export independently from character timing.'
    $diagnosticReport|Add-Member -NotePropertyName MachineGeneratedInput -NotePropertyValue $true
    $diagnosticReport|ConvertTo-Json -Depth 8|Set-Content -LiteralPath $diagnosticFile
    $null=& (Join-Path $PSScriptRoot 'export-rs12-input.ps1') -InputDirectory (Join-Path $fixture 'records') -OutputDirectory (Join-Path $fixture 'export-machine')
    $machineRows=@(Import-Csv -LiteralPath (Join-Path $fixture 'export-machine/desktop-latency.csv'))
    Assert ($machineRows[0].MachineGeneratedInput -eq 'True' -and $machineRows[0].CountsAsPhysicalHotkey -eq 'False' -and $machineRows[0].Method -eq 'NativeMachineCalibrationToAcceptedCharacterUpperBound') 'Machine calibration must not inherit a physical-key label through the generic export path.'
    @{ExitCode=-1;ForcedCleanup=$true}|ConvertTo-Json|Set-Content -LiteralPath (Join-Path $fixture 'records/case/completion.json')
    $null=& (Join-Path $PSScriptRoot 'export-rs12-input.ps1') -InputDirectory (Join-Path $fixture 'records') -OutputDirectory (Join-Path $fixture 'export-abnormal')
    $abnormal=@(Import-Csv -LiteralPath (Join-Path $fixture 'export-abnormal/desktop-latency.csv'))
    Assert (@($abnormal|Where-Object Valid -eq 'true').Count -eq 0 -and $abnormal[0].Reason -like 'Normal exit was not confirmed*') 'An abnormal or forced process exit must not turn an incomplete run into a successful sample.'
    @{ExitCode=0;ForcedCleanup=$false}|ConvertTo-Json|Set-Content -LiteralPath (Join-Path $fixture 'records/case/completion.json')
    $lostLog=Join-Path $fixture 'events.log'
    'event=input_accepted pid=42 unix_us=1012000 dropped=1 focused=true native_focused=true visible=true composing=false synthetic=false utf8_bytes=1'|Set-Content -LiteralPath $lostLog
    @{ProcessId=42;MeasurementLog=$lostLog}|ConvertTo-Json|Set-Content -LiteralPath (Join-Path $fixture 'records/case/session.json')
    $null=& (Join-Path $PSScriptRoot 'export-rs12-input.ps1') -InputDirectory (Join-Path $fixture 'records') -OutputDirectory (Join-Path $fixture 'export-lost')
    $lost=@(Import-Csv -LiteralPath (Join-Path $fixture 'export-lost/desktop-latency.csv'))
    Assert (@($lost|Where-Object Valid -eq 'true').Count -eq 0 -and $lost[0].Reason -like 'Final process log reported event loss*') 'Final log loss must invalidate a sample even when the live observer had marked it valid.'
    $cold=Join-Path $fixture 'cold-same-boot'
    New-Item -ItemType Directory -Path $cold|Out-Null
    @{PreparedAtUtc=[datetime]::UtcNow.ToString('o')}|ConvertTo-Json|Set-Content -LiteralPath (Join-Path $cold 'case.json')
    $rejected=$false
    try{& (Join-Path $PSScriptRoot 'start-rs12-cold.ps1') -CaseDirectory $cold -FirstVeloLaunchThisBootConfirmed}catch{$rejected=$_.Exception.Message -like 'This case was prepared during this boot*'}
    Assert ($rejected -and -not(Test-Path -LiteralPath (Join-Path $cold 'started.json'))) 'A same-boot cold attempt must be refused before any program is launched.'

    # Exercise the real collector in separate processes against synthetic records and an isolated boot ledger.
    $collectorRoot=Join-Path $fixture 'cold-collector'
    $collectorScripts=Join-Path $collectorRoot 'scripts'
    $collectorLedger=Join-Path $collectorRoot '.reasonix/rs12-cold-boot-ledger'
    New-Item -ItemType Directory -Path (Join-Path $collectorScripts 'lib'),$collectorLedger -Force|Out-Null
    Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'collect-rs12-cold.ps1') -Destination $collectorScripts
    foreach($name in 'resource-sample.ps1','rs12-input-observer.cs'){
        Copy-Item -LiteralPath (Join-Path $PSScriptRoot ('lib/'+$name)) -Destination (Join-Path $collectorScripts 'lib')
    }
    $boot=(Get-CimInstance Win32_OperatingSystem).LastBootUpTime.ToUniversalTime()
    $bootId=$boot.Ticks.ToString()
    @{Case='fixture';BootUtc=$boot.ToString('o');OperatorConfirmedFirstLaunch=$true}|ConvertTo-Json|Set-Content -LiteralPath (Join-Path $collectorLedger ('boot-'+$bootId+'.json')) -Encoding utf8NoBOM
    $startedTicks=[datetime]::UtcNow.AddSeconds(-3).Ticks
    $acceptedUs=[long](($startedTicks-[datetime]::UnixEpoch.Ticks)/10)+1000
    $reports=@()
    $pwsh=(Get-Process -Id $PID).Path
    foreach($number in 1..2){
        $casePath=Join-Path $collectorRoot ('case-'+$number)
        New-Item -ItemType Directory -Path $casePath|Out-Null
        $logPath=Join-Path $casePath 'events.log'
        "event=input_accepted pid=2147483647 unix_us=$acceptedUs dropped=0 focused=true native_focused=true visible=true composing=false synthetic=false trusted=true utf8_bytes=1"|Set-Content -LiteralPath $logPath -Encoding utf8NoBOM
        @{Case='fixture';ActualInputProbe=$true;PreparedAtUtc=$boot.AddDays(-1).ToString('o');MeasurementLog=$logPath;SHA256=('a'*64);InputId=('b'*64)}|ConvertTo-Json|Set-Content -LiteralPath (Join-Path $casePath 'case.json') -Encoding utf8NoBOM
        @{Case='fixture';ProcessId=[int]::MaxValue;StartedTicks=$startedTicks;BootUtc=$boot.ToString('o');BootID=$bootId;OperatorConfirmedFirstLaunch=$true}|ConvertTo-Json|Set-Content -LiteralPath (Join-Path $casePath 'started.json') -Encoding utf8NoBOM
        @{Case='fixture';ExitCode=0;MatchesPreparedSHA256=$true}|ConvertTo-Json|Set-Content -LiteralPath (Join-Path $casePath 'exit.json') -Encoding utf8NoBOM
        $collectorOutput=& $pwsh -NoProfile -NonInteractive -File (Join-Path $collectorScripts 'collect-rs12-cold.ps1') -CaseDirectory $casePath 2>&1
        Assert ($LASTEXITCODE -eq 0) "Synthetic cold collector failed: $collectorOutput"
        $reports+=Get-Content -LiteralPath (Join-Path $casePath 'cold-input.json') -Raw|ConvertFrom-Json
    }
    Assert ($reports[0].Result.Completed -and $reports[1].Result.Completed) 'Both synthetic cold records must retain accepted input.'
    Assert ($reports[0].EnvironmentIdAlgorithm -eq 'ColdWindowsBuildCpuOrdinalV1' -and $reports[0].EnvironmentId -eq $reports[1].EnvironmentId) 'Cold reports in separate processes must use the same versioned environment ID.'
    $machineCase=Join-Path $collectorRoot 'machine-case'
    Copy-Item -LiteralPath (Join-Path $collectorRoot 'case-1') -Destination $machineCase -Recurse
    Remove-Item -LiteralPath (Join-Path $machineCase 'cold-input.json')
    $metadata=Get-Content -Raw -LiteralPath (Join-Path $machineCase 'case.json')|ConvertFrom-Json
    $metadata|Add-Member -NotePropertyName InputSource -NotePropertyValue MachineCalibration
    $metadata|ConvertTo-Json|Set-Content -LiteralPath (Join-Path $machineCase 'case.json') -Encoding utf8NoBOM
    $collectorOutput=& $pwsh -NoProfile -NonInteractive -File (Join-Path $collectorScripts 'collect-rs12-cold.ps1') -CaseDirectory $machineCase 2>&1
    Assert ($LASTEXITCODE -eq 0) "Machine-source synthetic cold collector failed: $collectorOutput"
    $machineReport=Get-Content -Raw -LiteralPath (Join-Path $machineCase 'cold-input.json')|ConvertFrom-Json
    Assert ($machineReport.MachineGeneratedInput -and $machineReport.InputSource -eq 'MachineCalibration') 'Native machine input must survive generic cold collection explicitly, even when the product edit says synthetic=false.'

    # Destroy a real hidden HWND while its process stays alive. No keyboard
    # hooks or product windows are needed to reproduce the exit race.
    Add-Type -TypeDefinition @'
using System;
using System.ComponentModel;
using System.Runtime.InteropServices;
using System.Threading;
public sealed class VeloRS12WindowFixture : IDisposable {
    [DllImport("user32.dll",CharSet=CharSet.Unicode,SetLastError=true)] private static extern IntPtr CreateWindowEx(uint exStyle,string className,string title,uint style,int x,int y,int width,int height,IntPtr parent,IntPtr menu,IntPtr instance,IntPtr parameter);
    [DllImport("user32.dll")] private static extern bool DestroyWindow(IntPtr window);
    private readonly ManualResetEventSlim ready=new(false),close=new(false);
    private readonly Thread owner;
    private readonly Timer timer;
    private int error;
    public long HWND { get; private set; }
    public VeloRS12WindowFixture() {
        owner=new Thread(() => {
            var window=CreateWindowEx(0,"STATIC","",0,0,0,64,64,IntPtr.Zero,IntPtr.Zero,IntPtr.Zero,IntPtr.Zero);
            HWND=window.ToInt64();
            error=Marshal.GetLastWin32Error();
            ready.Set();
            close.Wait();
            if(window!=IntPtr.Zero) DestroyWindow(window);
        }) { IsBackground=true };
        timer=new Timer(_ => close.Set(),null,Timeout.Infinite,Timeout.Infinite);
        owner.Start();
        ready.Wait();
        if(HWND==0) { Dispose(); throw new Win32Exception(error); }
    }
    public void CloseAfter(int milliseconds) { timer.Change(milliseconds,Timeout.Infinite); }
    public void Dispose() { timer.Dispose(); close.Set(); owner.Join(); ready.Dispose(); close.Dispose(); }
}
'@
    Assert (-not [VeloRS12Window.Observer]::WindowOwnedByProcess(0,$PID) -and -not [VeloRS12Input.Observer]::WindowOwnedByProcess(0,$PID)) 'A failed HWND lookup must never retain a stale owner PID.'
    $windowFixture=[VeloRS12WindowFixture]::new()
    try {
        Assert ([VeloRS12Window.Observer]::WindowOwnedByProcess($windowFixture.HWND,$PID)) 'Fixture must start with an owned HWND.'
        $windowFixture.CloseAfter(250)
        $capture=[VeloRS12Window.Observer]::Run($PID,(Get-Process -Id $PID).StartTime.ToUniversalTime().Ticks,$windowFixture.HWND,10,20)
        Assert ($capture.TargetEnded -and $capture.Polls -gt 0 -and -not $capture.Truncated) 'Window destruction must preserve earlier native observations and end without a bounds error.'
        Assert (-not [VeloRS12Window.Observer]::WindowOwnedByProcess($windowFixture.HWND,$PID)) 'A destroyed fixture HWND must lose ownership while its process is still running.'
    } finally { $windowFixture.Dispose() }
    $windowFixture=[VeloRS12WindowFixture]::new()
    try {
        $windowFixture.CloseAfter(250)
        $watch=[Diagnostics.Stopwatch]::StartNew()
        $inputCapture=[VeloRS12Input.Observer]::Run($PID,$windowFixture.HWND,(Join-Path $fixture 'absent-input.log'),'WarmStartup',1,10,1)
        Assert ($inputCapture.EndReason -eq 'TargetWindowEnded' -and $watch.Elapsed.TotalSeconds -lt 5) 'A passive input observer must stop promptly on window destruction, not wait for the session timeout.'
        Assert (-not $inputCapture.Completed -and $inputCapture.Observations.Count -eq 1 -and -not $inputCapture.Observations[0].Valid) 'Ending without input must preserve the pending failure rather than invent a successful sample.'
    } finally { $windowFixture.Dispose() }
} finally {
    if(Test-Path -LiteralPath $fixture){
        $resolved=(Resolve-Path -LiteralPath $fixture).Path
        $temp=[IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\')+'\'
        if(-not $resolved.StartsWith($temp,[StringComparison]::OrdinalIgnoreCase) -or (Split-Path -Leaf $resolved) -notlike 'velo-rs12-input-check-*'){throw 'Unexpected RS-12 fixture cleanup target.'}
        Remove-Item -LiteralPath $resolved -Recurse -Force
    }
}
'RS-12 passive input observer checks passed (event rollover, focus, composition, synthetic/untrusted input, PID, event loss and real hidden-window destruction). No hook was installed.'
