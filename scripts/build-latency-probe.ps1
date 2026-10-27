[CmdletBinding()]
param([string]$OutputName = 'velo-launcher-latency.exe')
$ErrorActionPreference = 'Stop'
Push-Location (Split-Path -Parent $PSScriptRoot)
try {
    $index = Join-Path $PWD 'frontend/dist/index.html'
    if (-not (Test-Path -LiteralPath $index)) { throw 'Build the production frontend first.' }
    $original = [IO.File]::ReadAllBytes($index)
    try {
        $html = [Text.Encoding]::UTF8.GetString($original)
        $probe = [IO.File]::ReadAllText((Join-Path $PSScriptRoot 'latency-probe.js'), [Text.Encoding]::UTF8)
        if (-not $html.Contains('<script type="module"')) { throw 'Cannot find the production module entry.' }
        $html = $html.Replace('<script type="module"', ("<script>`n" + $probe + "`n</script>`n<script type=`"module`""))
        [IO.File]::WriteAllText($index, $html, [Text.UTF8Encoding]::new($false))
        & wails build -platform windows/amd64 -tags latencyprobe -o $OutputName -s -skipbindings
        if ($LASTEXITCODE -ne 0) { throw 'Latency probe build failed.' }
    } finally {
        [IO.File]::WriteAllBytes($index, $original)
    }
} finally { Pop-Location }
