param(
    [ValidateSet('All', 'FemtoVG', 'Software')][string]$Renderer = 'All',
    [switch]$SkipChecks,
    [switch]$SmokeTest
)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'lib/rust-environment.ps1')
$environment = Initialize-VeloRustEnvironment
Push-Location $environment.RustRoot
try {
    $toolchainText = Get-Content -LiteralPath 'rust-toolchain.toml' -Raw
    $pinnedRust = [regex]::Match($toolchainText, '(?m)^channel\s*=\s*"([^"]+)"').Groups[1].Value
    if (-not $pinnedRust) { throw 'No pinned Rust channel in rust/rust-toolchain.toml.' }
    $rustcVersion = @(& rustc --version --verbose)
    if ($LASTEXITCODE -ne 0) { throw 'The pinned Rust toolchain is unavailable.' }
    Write-Output $rustcVersion
    $cargoVersion = & cargo --version
    if ($LASTEXITCODE -ne 0) { throw 'cargo is unavailable.' }
    Write-Output $cargoVersion
    if ($rustcVersion -notcontains "release: $pinnedRust" -or $cargoVersion -notlike "cargo $pinnedRust *") {
        throw 'rustc/cargo do not match rust/rust-toolchain.toml.'
    }
    if (-not $SkipChecks) {
        & cargo fmt --all -- --check
        if ($LASTEXITCODE -ne 0) { throw 'Rust formatting failed.' }
    }
    $renderers = if ($Renderer -eq 'All') { @('femtovg', 'software') } else { @($Renderer.ToLowerInvariant()) }
    foreach ($name in $renderers) {
        $feature = "renderer-$name"
        if (-not $SkipChecks) {
            & cargo clippy --locked --no-default-features --features $feature --all-targets -- -D warnings
            if ($LASTEXITCODE -ne 0) { throw "clippy failed: $name" }
            & cargo test --locked --no-default-features --features $feature
            if ($LASTEXITCODE -ne 0) { throw "Rust tests failed: $name" }
        }
        & cargo build --release --locked --no-default-features --features $feature
        if ($LASTEXITCODE -ne 0) { throw "Release build failed: $name" }

        $outputDirectory = Join-Path $environment.RustRoot "target/artifacts/$name"
        New-Item -ItemType Directory -Path $outputDirectory -Force | Out-Null
        $executable = Join-Path $outputDirectory 'velo-slint.exe'
        Copy-Item -LiteralPath 'target/x86_64-pc-windows-msvc/release/velo-slint.exe' -Destination $executable -Force
        $metadataText = & cargo metadata --locked --format-version 1 --filter-platform x86_64-pc-windows-msvc --no-default-features --features $feature
        if ($LASTEXITCODE -ne 0) { throw "Dependency metadata failed: $name" }
        $metadata = $metadataText | ConvertFrom-Json
        $slintPackage = $metadata.packages | Where-Object name -eq 'slint'
        $slintNode = $metadata.resolve.nodes | Where-Object id -eq $slintPackage.id
        $rendererFeatures = @($slintNode.features | Where-Object { $_ -like 'renderer-*' })
        $winitBackendPackage = $metadata.packages | Where-Object name -eq 'i-slint-backend-winit'
        $winitBackendNode = $metadata.resolve.nodes | Where-Object id -eq $winitBackendPackage.id
        $backendRendererFeatures = @($winitBackendNode.features | Where-Object { $_ -like 'renderer-*' })
        if ($slintPackage.version -ne '1.18.1' -or $rendererFeatures.Count -ne 1 -or $rendererFeatures[0] -ne $feature -or
            $backendRendererFeatures.Count -ne 1 -or $backendRendererFeatures[0] -ne $feature) {
            throw "Unexpected Slint version or renderer features: $($rendererFeatures -join ', ')"
        }
        $metadataText | Set-Content -LiteralPath (Join-Path $outputDirectory 'cargo-metadata.json') -Encoding utf8
        $resolvedIds = @($metadata.resolve.nodes.id)
        $metadata.packages | Where-Object { $_.id -in $resolvedIds } | Sort-Object name, version |
            Select-Object name, version, license, license_file, repository |
            ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $outputDirectory 'dependency-licenses.json') -Encoding utf8
        & cargo tree --locked --no-default-features --features $feature -e normal,build,features |
            Set-Content -LiteralPath (Join-Path $outputDirectory 'cargo-tree.txt') -Encoding utf8
        if ($LASTEXITCODE -ne 0) { throw "Dependency tree failed: $name" }
        $sourcePaths = @('Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml', 'windows-toolchain.json', '.cargo/config.toml', 'build.rs', '../scripts/verify-rust.ps1', '../scripts/lib/rust-environment.ps1', '../scripts/measure-resources.ps1', '../scripts/measure-slint-ab.ps1', '../scripts/collect-latency.ps1', '../scripts/collect-slint-latency.ps1', '../scripts/lib/resource-sample.ps1')
        # Include UI modules, embedded assets, and native validation fixtures.
        $sourcePaths += @(Get-ChildItem -LiteralPath 'src', 'ui', 'examples' -Recurse -File | ForEach-Object {
            $_.FullName.Substring($environment.RustRoot.Length + 1).Replace('\', '/')
        })
        $sourcePaths = @($sourcePaths | Sort-Object -Unique)
        $sources = foreach ($source in $sourcePaths) {
            [ordered]@{ Path = $source; SHA256 = (Get-FileHash -LiteralPath $source -Algorithm SHA256).Hash }
        }
        $identity = [ordered]@{
            Renderer = $name
            Backend = 'winit'
            Target = 'x86_64-pc-windows-msvc'
            Rustc = (& rustc --version)
            Cargo = (& cargo --version)
            MsvcToolsVersion = $environment.MsvcToolsVersion
            WindowsSdkVersion = $environment.WindowsSdkVersion
            CompilerFileVersion = $environment.CompilerFileVersion
            LinkerFileVersion = $environment.LinkerFileVersion
            SlintVersion = $slintPackage.version
            SlintFeatures = $slintNode.features
            WinitBackendFeatures = $winitBackendNode.features
            WorkspaceCommit = (& git rev-parse HEAD)
            WorkspaceDirty = [bool](& git status --porcelain)
            ExecutableSHA256 = (Get-FileHash -LiteralPath $executable -Algorithm SHA256).Hash
            ExecutableBytes = (Get-Item -LiteralPath $executable).Length
            Sources = @($sources)
        }
        $identity | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $outputDirectory 'build-identity.json') -Encoding utf8

        if ($SmokeTest) {
            $smokeLog = Join-Path $outputDirectory 'smoke.stderr.log'
            $process = Start-Process -FilePath $executable -ArgumentList '--smoke-test', '--diagnostics' `
                -WindowStyle Hidden -PassThru -RedirectStandardError $smokeLog
            if (-not $process.WaitForExit(15000)) {
                Stop-Process -Id $process.Id -ErrorAction SilentlyContinue
                throw "Window smoke test timed out: $name"
            }
            $process.Refresh()
            $smokeText = Get-Content -LiteralPath $smokeLog -Raw
            Write-Output $smokeText
            if ($process.ExitCode -ne 0 -or $smokeText -notmatch 'event=smoke_result passed=true') {
                throw "Window smoke test failed: $name (exit $($process.ExitCode))"
            }
            # Dispatch through the real Slint component/focus path as a separate
            # check. This is repeatable integration coverage, not native IME proof.
            $inputLog = Join-Path $outputDirectory 'input-smoke.stderr.log'
            $inputProcess = Start-Process -FilePath $executable -ArgumentList '--input-smoke-test', '--diagnostics' `
                -WindowStyle Hidden -PassThru -RedirectStandardError $inputLog
            if (-not $inputProcess.WaitForExit(15000)) {
                Stop-Process -Id $inputProcess.Id -ErrorAction SilentlyContinue
                throw "Input component smoke test timed out: $name"
            }
            $inputProcess.Refresh()
            $inputText = Get-Content -LiteralPath $inputLog -Raw
            Write-Output $inputText
            if ($inputProcess.ExitCode -ne 0 -or $inputText -notmatch 'event=input_smoke_result passed=true' -or $inputText -notmatch 'event=smoke_result passed=true') {
                throw "Input component smoke test failed: $name (exit $($inputProcess.ExitCode))"
            }
        }
        Write-Host "Verified $name Release: $executable"
    }
} finally { Pop-Location }
