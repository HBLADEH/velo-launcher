param([switch]$SkipBuild)
$ErrorActionPreference = 'Stop'
Push-Location (Split-Path -Parent $PSScriptRoot)
try {
    $goSources = @((Get-ChildItem -LiteralPath . -Filter '*.go').FullName) + @('internal')
    $unformatted = & gofmt -l @goSources
    if ($LASTEXITCODE -ne 0 -or $unformatted) { throw "Run gofmt: $unformatted" }
    & go vet ./...
    if ($LASTEXITCODE -ne 0) { throw 'go vet failed' }
    & go test ./...
    if ($LASTEXITCODE -ne 0) { throw 'go test failed' }
    & wails generate module
    if ($LASTEXITCODE -ne 0) { throw 'binding generation failed' }
    & npm --prefix frontend run lint
    if ($LASTEXITCODE -ne 0) { throw 'frontend lint failed' }
    & npm --prefix frontend run build
    if ($LASTEXITCODE -ne 0) { throw 'frontend build failed' }
    if (-not $SkipBuild) {
        & wails build -clean -platform windows/amd64
        if ($LASTEXITCODE -ne 0) { throw 'Windows build failed' }
    }
} finally { Pop-Location }
