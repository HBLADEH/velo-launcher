#Requires -Version 7.4
[CmdletBinding()]
param([Parameter(Mandatory)][string]$PreparedDirectory,[Parameter(Mandatory)][string]$CalibratorExecutable,[Parameter(Mandatory)][string]$OutputDirectory)
$ErrorActionPreference='Stop'
$CalibratorExecutable=(Resolve-Path -LiteralPath $CalibratorExecutable).Path
$OutputDirectory=$ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($OutputDirectory)
if(Test-Path -LiteralPath $OutputDirectory){throw 'Preserve earlier calibrated cold cases.'}
$calibration=Get-Content -Raw -LiteralPath (Join-Path (Split-Path -Parent $CalibratorExecutable) 'identity.json')|ConvertFrom-Json
if((Get-FileHash -LiteralPath $CalibratorExecutable).Hash -ne $calibration.ExecutableSHA256){throw 'Cold calibrator identity changed.'}
& (Join-Path $PSScriptRoot 'prepare-rs12-cold.ps1') -PreparedDirectory $PreparedDirectory -OutputDirectory $OutputDirectory
if(-not $?){throw 'Cold cases were not prepared.'}
foreach($directory in Get-ChildItem -LiteralPath $OutputDirectory -Directory){
    $path=Join-Path $directory.FullName 'case.json';$case=Get-Content -Raw -LiteralPath $path|ConvertFrom-Json
    $case|Add-Member -NotePropertyName InputSource -NotePropertyValue 'MachineCalibration'
    $case|ConvertTo-Json -Depth 8|Set-Content -LiteralPath $path -Encoding utf8NoBOM
}
$helpers=Join-Path $OutputDirectory 'calibrator';New-Item -ItemType Directory -Path $helpers|Out-Null
Copy-Item -LiteralPath $CalibratorExecutable -Destination $helpers
Copy-Item -LiteralPath (Join-Path (Split-Path -Parent $CalibratorExecutable) 'identity.json') -Destination $helpers
$tools=[ordered]@{}
foreach($name in 'start-rs12-calibrated-cold.ps1','start-rs12-cold.ps1','collect-rs12-cold.ps1','export-rs12-readiness.ps1'){$tools[$name]=(Get-FileHash -LiteralPath (Join-Path $PSScriptRoot $name)).Hash}
[ordered]@{PreparedAtUtc=[datetime]::UtcNow.ToString('o');CaseRoot=$OutputDirectory;CalibratorExecutable=Join-Path $helpers ([IO.Path]::GetFileName($CalibratorExecutable));CalibratorSHA256=$calibration.ExecutableSHA256;CalibratorSourceSHA256=$calibration.SourceSHA256;ToolsSHA256=$tools;Sequence=@('software-1','femtovg-1','wails-1','femtovg-2','wails-2','software-2','wails-3','software-3','femtovg-3');MachineInput=$true;ColdCalibrationSamplesTaken=0;Scope='Nine future independent boot cases. No reboot or startup-setting change. Each launch still requires manual first-Velo-launch confirmation; input is a guarded native machine calibration, separate from prior physical cold character observations.'}|ConvertTo-Json -Depth 8|Set-Content -LiteralPath (Join-Path $OutputDirectory 'calibration-manifest.json') -Encoding utf8NoBOM
'Nine isolated calibrated cold cases prepared. Each requires its own later independent boot; none started.'
