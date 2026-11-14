#Requires -Version 7.4
[CmdletBinding()]
param([Parameter(Mandatory)][string]$ObserverExecutable,[Parameter(Mandatory)][string]$OutputDirectory)
$ErrorActionPreference='Stop'
$ObserverExecutable=(Resolve-Path -LiteralPath $ObserverExecutable).Path
$OutputDirectory=$ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($OutputDirectory)
if(Test-Path -LiteralPath $OutputDirectory){throw 'Preserve earlier native frame regressions.'}
$identity=Get-Content -Raw -LiteralPath (Join-Path (Split-Path -Parent $ObserverExecutable) 'identity.json')|ConvertFrom-Json
if((Get-FileHash -LiteralPath $ObserverExecutable).Hash -ne $identity.ExecutableSHA256){throw 'Native observer identity differs.'}
New-Item -ItemType Directory -Path $OutputDirectory|Out-Null
$capture=Join-Path $OutputDirectory 'capture'
& $ObserverExecutable calibrate-resize 0 3 0 $capture
if($LASTEXITCODE -ne 0){throw 'Native geometry calibration failed; retain its records.'}
$null=& (Join-Path $PSScriptRoot 'summarize-rs12-frames.ps1') -CaptureDirectory $capture -OutputPath (Join-Path $OutputDirectory 'summary.json')
$events=@(Get-Content -LiteralPath (Join-Path $capture 'events.jsonl')|ForEach-Object{$_|ConvertFrom-Json})
$geometries=@($events|Where-Object event -eq 'geometry_observed')
$frames=@($events|Where-Object event -eq 'frame')
if($geometries.Count -lt 3 -or @($frames|Where-Object{$_.present_qpc -lt $_.geometry_observed_qpc}).Count){throw 'A desktop frame predates its observed window geometry.'}
if(@($frames|Group-Object width,height).Count -lt 2){throw 'Native calibration did not actually change capture dimensions.'}
if(@(Get-ChildItem -LiteralPath $capture -File -Filter '*.bmp').Count){throw 'No product/desktop images are needed for this geometry regression.'}
[ordered]@{RecordedAtUtc=[datetime]::UtcNow.ToString('o');Passed=$true;ObserverSHA256=$identity.ExecutableSHA256;ObserverSourceSHA256=$identity.SourceSHA256;Frames=$frames.Count;GeometryTransitions=$geometries.Count;CapturedSizes=@($frames|Group-Object width,height|Select-Object Name,Count);NoImagesSaved=$true;ProductProcessesStarted=$false;Scope='Self-owned non-activating test window resized/moved on its timer; present timestamps must follow observed geometry. No input synthesized; no application frame/readiness acceptance implied.'}|ConvertTo-Json -Depth 6|Set-Content -LiteralPath (Join-Path $OutputDirectory 'verification.json') -Encoding utf8NoBOM
'Native frame geometry regression passed; self-owned window only.'
