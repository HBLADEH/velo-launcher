#Requires -Version 7.4
$ErrorActionPreference='Stop'
function Assert($Condition,[string]$Message){if(-not $Condition){throw $Message}}
$fixture=Join-Path ([IO.Path]::GetTempPath()) ('velo-rs12-frame-data-'+[guid]::NewGuid().ToString('N'))
try{
    $seed=Join-Path $fixture 'source/seed'
    New-Item -ItemType Directory -Path $seed -Force|Out-Null
    $exe=Join-Path $fixture 'unrunnable-executable-fixture.exe'
    'Non-executable identity fixture; no program is launched.'|Set-Content -LiteralPath $exe -Encoding utf8NoBOM
    $sha=(Get-FileHash -LiteralPath $exe).Hash
    $build=@{Executable=$exe;SHA256=$sha}
    @{SeedDirectory=$seed;WorkspaceCommit='synthetic';Builds=@{software=$build;femtovg=$build;wails=$build}}|ConvertTo-Json -Depth 8|Set-Content -LiteralPath (Join-Path $fixture 'source/manifest.json') -Encoding utf8NoBOM
    @{version=1;hotkey='Alt+Space';disable_hotkey_fullscreen=$true;max_results=8;theme='light';result_layout='list';launch_at_startup=$false;space_launch=$false;filter_noise=$true;search=@{fuzzy=$true;history_weight=1};custom_directories=@();scan_program_files=$true;refresh_minutes=1440;auto_check_updates=$false}|ConvertTo-Json -Depth 8|Set-Content -LiteralPath (Join-Path $seed 'config.json') -Encoding utf8NoBOM
    $index=Join-Path $fixture 'reference-index.json'
    @{version=1;apps=@(@{id=('0'*31+'1')},@{id=('0'*31+'2')})}|ConvertTo-Json -Depth 5|Set-Content -LiteralPath $index -Encoding utf8NoBOM
    $output=Join-Path $fixture 'prepared'
    $null=& (Join-Path $PSScriptRoot 'prepare-rs12-frame-fixtures.ps1') -PreparedDirectory (Join-Path $fixture 'source') -BaselineIndexes @($index) -OutputDirectory $output
    foreach($count in 1000,10000){
        $prepared=Join-Path $output ([string]$count)
        $manifest=Get-Content -Raw -LiteralPath (Join-Path $prepared 'manifest.json')|ConvertFrom-Json
        $library=Get-Content -Raw -LiteralPath (Join-Path $prepared 'seed/application-library.json')|ConvertFrom-Json
        $cache=Get-Content -Raw -LiteralPath (Join-Path $prepared 'seed/index.json')|ConvertFrom-Json
        $config=Get-Content -Raw -LiteralPath (Join-Path $prepared 'seed/config.json')|ConvertFrom-Json
        Assert ($library.applications.Count -eq $count -and $cache.apps.Count -eq $count -and $manifest.ExpectedCatalogCount -eq $count) 'Both initial cache and library must carry the requested workload size.'
        Assert ($library.pinned.Count -eq 4 -and @($library.applications|Group-Object id|Where-Object Count -gt 1).Count -eq 0) 'Pins and application identities must remain deterministic and unique.'
        Assert ($library.applications[0].id -eq ('0'*31+'1') -and $library.applications[1].id -eq ('0'*31+'2')) 'Every supplied discovery identity must be shadowed with synthetic display data.'
        $url=$library.applications[0].icon_url
        Assert ($url -match '^/icons/[0-9a-f]{32}\.png$') 'The Rust cache reader rejects icon names outside the 32-hex namespace.'
        $image=[Drawing.Image]::FromFile((Join-Path $manifest.SeedDirectory ('cache/icons/'+[IO.Path]::GetFileName($url))))
        try{Assert ($image.Width -eq 32 -and $image.Height -eq 32) 'The advertised fixture icon must decode as actual pixels.'}finally{$image.Dispose()}
        $preparedIcons=Get-Content -Raw -LiteralPath (Join-Path $manifest.SeedDirectory 'frame-icon-preparation.json')|ConvertFrom-Json
        Assert ($preparedIcons.prepared_visible_items -eq 16 -and @($preparedIcons.icon_urls|Sort-Object -Unique).Count -eq 16) 'Actual publish-time path signatures need separate cached PNGs for the first visible rows.'
        foreach($iconUrl in $preparedIcons.icon_urls){Assert (Test-Path -LiteralPath (Join-Path $manifest.SeedDirectory ('cache/icons/'+[IO.Path]::GetFileName($iconUrl)))) 'A recomputed visible icon URL must have real cached pixels.'}
        Assert (-not(Test-Path -LiteralPath $library.applications[0].exec_path) -and -not $config.launch_at_startup -and -not $config.auto_check_updates -and -not $config.space_launch) 'Controlled fixtures must not create launch targets or enable startup/update behavior.'
    }
    $rejected=$false
    try{& (Join-Path $PSScriptRoot 'prepare-rs12-frame-fixtures.ps1') -PreparedDirectory (Join-Path $fixture 'source') -BaselineIndexes @($index) -OutputDirectory $output}catch{$rejected=$_.Exception.Message -like 'Use a fresh fixture directory*'}
    Assert $rejected 'Earlier prepared fixture identities must not be overwritten.'
    'changed'|Set-Content -LiteralPath $exe -Encoding utf8NoBOM
    $rejected=$false
    try{& (Join-Path $PSScriptRoot 'prepare-rs12-frame-fixtures.ps1') -PreparedDirectory (Join-Path $fixture 'source') -BaselineIndexes @($index) -OutputDirectory (Join-Path $fixture 'changed-exe')}catch{$rejected=$_.Exception.Message -like 'Frozen measurement executable changed*'}
    Assert $rejected 'A changed frozen executable must be rejected before a new fixture set is created.'
}finally{
    if(Test-Path -LiteralPath $fixture){
        $resolved=(Resolve-Path -LiteralPath $fixture).Path
        $temporary=[IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\')+'\'
        if(-not $resolved.StartsWith($temporary,[StringComparison]::OrdinalIgnoreCase) -or (Split-Path -Leaf $resolved) -notlike 'velo-rs12-frame-data-*'){throw 'Unexpected fixture cleanup target.'}
        Remove-Item -LiteralPath $resolved -Recurse -Force
    }
}
'RS-12 frame fixture checks passed (1000/10000 catalogs, discovery overlays, real icon decoding/cache namespace, isolation and frozen identities).'
