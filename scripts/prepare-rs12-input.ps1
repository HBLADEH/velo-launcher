#Requires -Version 7.4
[CmdletBinding()]
param([Parameter(Mandatory)][string]$PreparedDirectory,[Parameter(Mandatory)][string]$OutputDirectory,[string]$WailsProbeExecutable)
$ErrorActionPreference='Stop'
$workspace=Split-Path -Parent $PSScriptRoot
. (Join-Path $PSScriptRoot 'lib/resource-sample.ps1')
$PreparedDirectory=(Resolve-Path -LiteralPath $PreparedDirectory).Path
$OutputDirectory=$ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($OutputDirectory)
if(Test-Path -LiteralPath $OutputDirectory){throw 'Use a fresh input measurement directory.'}
$reference=Read-VeloMetadataJson (Join-Path $PreparedDirectory 'manifest.json')
New-Item -ItemType Directory -Path (Join-Path $OutputDirectory 'binaries/wails') -Force|Out-Null
$probe=if($WailsProbeExecutable){(Resolve-Path -LiteralPath $WailsProbeExecutable).Path}else{Join-Path $workspace 'build/bin/velo-launcher-rs12-input.exe'}
$info=@(& go version -m $probe)
if($LASTEXITCODE -ne 0 -or -not($info -match 'latencyprobe')){throw 'Build the opt-in Wails actual-input probe first.'}
$destination=Join-Path $OutputDirectory 'binaries/wails/velo-launcher-rs12-input.exe'
Copy-Item -LiteralPath $probe -Destination $destination
$builds=[ordered]@{}
foreach($renderer in 'software','femtovg'){
    $exe=Join-Path $PreparedDirectory "binaries/$renderer/velo-slint.exe"
    $identity=Read-VeloRustBuildIdentity $exe
    if($identity.ExecutableSHA256 -ne $reference.Builds.$renderer.ExecutableSHA256){throw 'Frozen Slint binary changed.'}
    $builds[$renderer]=@{Executable=$exe;SHA256=$identity.ExecutableSHA256;Scope='Original frozen RS-12 Slint executable with opt-in actual-input log; normal business lifecycle.'}
}
$sources=@(foreach($name in @('latency_probe_windows.go','scripts/latency-probe.js','scripts/lib/rs12-input-observer.cs','scripts/collect-rs12-input.ps1','main.go','app.go','internal/app/service.go','internal/app/quick.go','frontend/dist/index.html','go.mod','go.sum')){
    @{Path=$name;SHA256=(Get-FileHash -LiteralPath (Join-Path $workspace $name)).Hash}
})
$builds.wails=@{Executable=$destination;SHA256=(Get-FileHash -LiteralPath $destination).Hash;GoBuildInfo=$info;Scope='v0.9.11 behavior baseline rebuilt with latencyprobe and actual trusted input callback; a separate measurement binary, not the official release SHA. Runtime differences from tag are two fixed-clock test seams delegating to time.Now plus instrumentation.'}
[ordered]@{PreparedAtUtc=[datetime]::UtcNow.ToString('o');SeedDirectory=Join-Path $PreparedDirectory 'seed';WorkspaceCommit=(& git -C $workspace rev-parse HEAD);Builds=$builds;Sources=$sources;ColdSamples=0;Method='Passive physical Alt+Space event lower bound to actual focused accepted character; typing delay included and injected starts excluded.'}|ConvertTo-Json -Depth 12|Set-Content -LiteralPath (Join-Path $OutputDirectory 'manifest.json') -Encoding utf8NoBOM
'RS-12 actual-input builds frozen; original resource binaries and results preserved.'
