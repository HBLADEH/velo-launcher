#Requires -Version 7.4
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$PreparedDirectory,
    [Parameter(Mandatory)][string[]]$BaselineIndexes,
    [Parameter(Mandatory)][string]$OutputDirectory
)
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'lib/resource-sample.ps1')
$manifest=Read-VeloMetadataJson (Join-Path $PreparedDirectory 'manifest.json')
$OutputDirectory=$ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($OutputDirectory)
if(Test-Path -LiteralPath $OutputDirectory){throw 'Use a fresh fixture directory; preserve earlier input identities.'}
$ids=[Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
$referenceHashes=@(foreach($path in $BaselineIndexes){
    $index=Read-VeloMetadataJson $path
    foreach($item in $index.apps){if(-not [string]::IsNullOrWhiteSpace($item.id)){[void]$ids.Add($item.id)}}
    (Get-FileHash -LiteralPath $path).Hash
})
if($ids.Count -eq 0 -or $ids.Count -gt 1000){throw 'Reference application ID union must contain 1–1000 items for both controlled sizes.'}
foreach($edition in 'software','femtovg','wails'){
    if((Get-FileHash -LiteralPath $manifest.Builds.$edition.Executable).Hash -ne $manifest.Builds.$edition.SHA256){throw 'Frozen measurement executable changed.'}
}
New-Item -ItemType Directory -Path $OutputDirectory|Out-Null
$targetRoot=Join-Path $OutputDirectory ('uncreated-targets-'+[guid]::NewGuid().ToString('N'))
if(Test-Path -LiteralPath $targetRoot){throw 'Synthetic launch targets must not exist.'}
$orderedIds=[string[]]@($ids)
[Array]::Sort($orderedIds,[StringComparer]::Ordinal)
$config=Read-VeloMetadataJson (Join-Path $manifest.SeedDirectory 'config.json')
$config.launch_at_startup=$false;$config.auto_check_updates=$false;$config.refresh_minutes=1440
$config.theme='light';$config.result_layout='list';$config.max_results=8;$config.space_launch=$false
$config.custom_directories=@();$config.scan_program_files=$false
Add-Type -AssemblyName System.Drawing
$seeds=@(foreach($count in 1000,10000){
    $directory=Join-Path $OutputDirectory ([string]$count)
    $seed=Join-Path $directory 'seed'
    $icons=Join-Path $seed 'cache/icons'
    New-Item -ItemType Directory -Path $icons -Force|Out-Null
    # A code-generated opaque checker is a shared test texture, not a product icon.
    $iconName=[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes('Velo RS12 opaque checker icon'))).Substring(0,32).ToLowerInvariant()+'.png'
    if($iconName -notmatch '^[0-9a-f]{32}\.png$'){throw 'Fixture icon must use the existing generated-cache namespace.'}
    $icon=Join-Path $icons $iconName
    $bitmap=[Drawing.Bitmap]::new(32,32)
    try{
        for($y=0;$y -lt 32;$y++){for($x=0;$x -lt 32;$x++){
            $color=if(($x -lt 16) -eq ($y -lt 16)){[Drawing.Color]::FromArgb(255,50,100,150)}else{[Drawing.Color]::FromArgb(255,190,210,230)}
            $bitmap.SetPixel($x,$y,$color)
        }}
        $bitmap.Save($icon,[Drawing.Imaging.ImageFormat]::Png)
    }finally{$bitmap.Dispose()}
    $applications=@(for($number=0;$number -lt $count;$number++){
        $path=Join-Path $targetRoot ('App-{0:D5}.exe' -f $number)
        $id=if($number -lt $orderedIds.Count){$orderedIds[$number]}else{
            $bytes=[Text.Encoding]::UTF8.GetBytes($path.ToLowerInvariant()+[char]0)
            [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($bytes)[0..15]).ToLowerInvariant()
        }
        [pscustomobject][ordered]@{pinned=$false;id=$id;name=('RS12 测试 App {0:D5}' -f $number);path=$path;exec_path=$path;arguments='';working_directory=$targetRoot;icon_path=$icon;icon_url=('/icons/'+$iconName);description='Synthetic RS-12 frame workload';source='Manual';keywords=@('rs12','app','测试')}
    })
    if(@($applications|Group-Object id|Where-Object Count -gt 1).Count){throw 'Synthetic fixture contains duplicate application IDs.'}
    # Existing discovered IDs are deliberately shadowed with synthetic labels.
    # Reject the run unless the actual published catalog still equals this size.
    $config|ConvertTo-Json -Depth 20|Set-Content -LiteralPath (Join-Path $seed 'config.json') -Encoding utf8NoBOM
    @{version=1;files=@{};apps=$applications}|ConvertTo-Json -Depth 20|Set-Content -LiteralPath (Join-Path $seed 'index.json') -Encoding utf8NoBOM
    @{version=1;applications=$applications;pinned=@($applications|Select-Object -First 4)}|ConvertTo-Json -Depth 20|Set-Content -LiteralPath (Join-Path $seed 'application-library.json') -Encoding utf8NoBOM
    '{}'|Set-Content -LiteralPath (Join-Path $seed 'history.json') -Encoding utf8NoBOM
    # publish() recomputes library icon URLs from path/mtime even when a saved
    # URL is syntactically valid. Prepare the actual first visible cache keys.
    $previousGoToolchain=$env:GOTOOLCHAIN
    try{
        $env:GOTOOLCHAIN='go1.25.6'
        Push-Location (Split-Path -Parent $PSScriptRoot)
        try{& go run ./cmd/rs12-icons prepare-frame-icons $seed|Out-Host;if($LASTEXITCODE -ne 0){throw 'Actual icon signature preparation failed.'}}finally{Pop-Location}
    }finally{$env:GOTOOLCHAIN=$previousGoToolchain}
    $inputs=[ordered]@{}
    foreach($file in @('config.json','index.json','history.json','application-library.json',('cache/icons/'+$iconName))){$inputs[$file]=(Get-FileHash -LiteralPath (Join-Path $seed $file)).Hash}
    $inputs['frame-icon-preparation.json']=(Get-FileHash -LiteralPath (Join-Path $seed 'frame-icon-preparation.json')).Hash
    [ordered]@{PreparedAtUtc=[datetime]::UtcNow.ToString('o');SeedDirectory=$seed;WorkspaceCommit=$manifest.WorkspaceCommit;Builds=$manifest.Builds;ExpectedCatalogCount=$count;Inputs=$inputs;Scope='Controlled synthetic library overlays discovered IDs; normal scan retained. Exact published count must be checked after scan. 10000 is an expansion workload loaded from a test file, not an increased UI import limit.'}|ConvertTo-Json -Depth 20|Set-Content -LiteralPath (Join-Path $directory 'manifest.json') -Encoding utf8NoBOM
    [ordered]@{Count=$count;ShadowedReferenceIDs=$orderedIds.Count;Inputs=$inputs}
})
[ordered]@{PreparedAtUtc=[datetime]::UtcNow.ToString('o');ReferenceIndexSHA256=$referenceHashes;ReferenceIDUnionCount=$orderedIds.Count;Seeds=$seeds;TargetFilesCreated=$false;ProductSourcesChanged=$false;Scope='Synthetic names/paths/icon, empty history, four pins; original binaries. Discovered IDs shadowed to keep normal refresh from adding personal labels. Validate exact post-scan count before accepting UI timing; unexpected discovered IDs invalidate that condition.'}|ConvertTo-Json -Depth 10|Set-Content -LiteralPath (Join-Path $OutputDirectory 'identity.json') -Encoding utf8NoBOM
'Prepared controlled 1000/10000-item frame fixtures; no application was started.'
