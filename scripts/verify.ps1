param([switch]$SkipBuild)
$ErrorActionPreference = 'Stop'
Push-Location (Split-Path -Parent $PSScriptRoot)
try {
    $goSources = @((Get-ChildItem -LiteralPath . -Filter '*.go').FullName) + @('internal')
    $unformatted = & gofmt -l @goSources
    if ($LASTEXITCODE -ne 0 -or $unformatted) { throw "Run gofmt: $unformatted" }
    & npm --prefix frontend run lint
    if ($LASTEXITCODE -ne 0) { throw 'frontend lint failed' }
    # 先生成 frontend/dist：main.go 的 //go:embed all:frontend/dist 要求它存在
    & npm --prefix frontend run build
    if ($LASTEXITCODE -ne 0) { throw 'frontend build failed' }
    & wails generate module
    if ($LASTEXITCODE -ne 0) { throw 'binding generation failed' }
    & go vet ./...
    if ($LASTEXITCODE -ne 0) { throw 'go vet failed' }
    & go test ./...
    if ($LASTEXITCODE -ne 0) { throw 'go test failed' }
    if (-not $SkipBuild) {
        & wails build -clean -platform windows/amd64
        if ($LASTEXITCODE -ne 0) { throw 'Windows build failed' }
    }
} finally { Pop-Location }
