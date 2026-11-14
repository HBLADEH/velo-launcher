param(
    [ValidateSet('All', 'FemtoVG', 'Software')][string]$Renderer = 'All',
    [switch]$SkipChecks,
    [switch]$SmokeTest,
    [switch]$Product
)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'lib/rust-environment.ps1')
$environment = Initialize-VeloRustEnvironment
$renderers = if ($Renderer -eq 'All') { @('femtovg', 'software') } else { @($Renderer.ToLowerInvariant()) }
$artifactGroup = if ($Product) { 'product-artifacts' } else { 'artifacts' }
foreach ($name in $renderers) {
    $receipt = Join-Path $environment.RustRoot "target/$artifactGroup/$name/verification.json"
    if (Test-Path -LiteralPath $receipt) { Remove-Item -LiteralPath $receipt }
}
Push-Location $environment.RustRoot
try {
    $toolchainText = Get-Content -LiteralPath 'rust-toolchain.toml' -Raw
    $pinnedChannel = [regex]::Match($toolchainText, '(?m)^channel\s*=\s*"([^"]+)"').Groups[1].Value
    $pinnedRust = [regex]::Match($pinnedChannel, '^(\d+\.\d+\.\d+)-x86_64-pc-windows-msvc$').Groups[1].Value
    if (-not $pinnedRust) { throw 'Pin an exact Rust version and x86_64-pc-windows-msvc host in rust/rust-toolchain.toml.' }
    $rustcVersion = @(& rustc --version --verbose)
    if ($LASTEXITCODE -ne 0) { throw 'The pinned Rust toolchain is unavailable.' }
    Write-Output $rustcVersion
    $cargoVersion = & cargo --version
    if ($LASTEXITCODE -ne 0) { throw 'cargo is unavailable.' }
    Write-Output $cargoVersion
    if ($rustcVersion -notcontains "release: $pinnedRust" -or
        $rustcVersion -notcontains 'host: x86_64-pc-windows-msvc' -or
        $cargoVersion -notlike "cargo $pinnedRust *") {
        throw 'rustc/cargo do not match rust/rust-toolchain.toml.'
    }
    if (-not $SkipChecks) {
        & cargo fmt --all -- --check
        if ($LASTEXITCODE -ne 0) { throw 'Rust formatting failed.' }
        & (Join-Path $PSScriptRoot 'test-search-compat.ps1') -GoOnly
        & (Join-Path $PSScriptRoot 'test-discovery-compat.ps1') -GoOnly
        Push-Location ..
        try {
            & go run ./cmd/rs11-compat
            if ($LASTEXITCODE -ne 0) { throw 'RS-11 version/system tool Go oracle failed.' }
        } finally { Pop-Location }
    }
    foreach ($name in $renderers) {
        $feature = "renderer-$name"
        if ($Product) { $feature += ',product' }
        if (-not $SkipChecks) {
            & cargo clippy --locked --no-default-features --features $feature --all-targets -- -D warnings
            if ($LASTEXITCODE -ne 0) { throw "clippy failed: $name" }
            & cargo test --locked --no-default-features --features $feature
            if ($LASTEXITCODE -ne 0) { throw "Rust tests failed: $name" }
            & (Join-Path $PSScriptRoot 'test-data-compat.ps1') -Renderer $name
            $nativeDirectory = Join-Path $environment.RustRoot ('target/compat-rs10/' + [guid]::NewGuid().ToString('N'))
            New-Item -ItemType Directory -Path $nativeDirectory -Force | Out-Null
            & cargo run --locked --no-default-features --features $feature --example rs10_windows -- (Join-Path $nativeDirectory 'native.json')
            if ($LASTEXITCODE -ne 0) { throw "RS-10 native Windows verification failed: $name" }
            & cargo run --locked --no-default-features --features $feature --example rs11_windows -- `
                (Join-Path $environment.RustRoot 'target/x86_64-pc-windows-msvc/debug/examples/rs10_windows.exe') (Join-Path $nativeDirectory 'rs11-native.json')
            if ($LASTEXITCODE -ne 0) { throw "RS-11 native update verification failed: $name" }
        }
        & cargo build --release --locked --no-default-features --features $feature
        if ($LASTEXITCODE -ne 0) { throw "Release build failed: $name" }

        $outputDirectory = Join-Path $environment.RustRoot "target/$artifactGroup/$name"
        New-Item -ItemType Directory -Path $outputDirectory -Force | Out-Null
        $verificationPath = Join-Path $outputDirectory 'verification.json'
        $executable = Join-Path $outputDirectory 'velo-slint.exe'
        Copy-Item -LiteralPath 'target/x86_64-pc-windows-msvc/release/velo-slint.exe' -Destination $executable -Force
        & (Join-Path $PSScriptRoot 'test-rust-metadata.ps1') -Renderer $name -Executable $executable -OutputDirectory (Join-Path $outputDirectory ('metadata-' + [guid]::NewGuid().ToString('N')))
        if (-not $SkipChecks) {
            $startupDirectory = Join-Path $outputDirectory ('startup-' + [guid]::NewGuid().ToString('N'))
            & cargo run --locked --no-default-features --features $feature --example rs11_startup -- $executable $startupDirectory
            if ($LASTEXITCODE -ne 0) { throw "RS-11 background startup verification failed: $name" }
        }
        $metadataText = & cargo metadata --locked --format-version 1 --filter-platform x86_64-pc-windows-msvc --no-default-features --features $feature
        if ($LASTEXITCODE -ne 0) { throw "Dependency metadata failed: $name" }
        $metadata = $metadataText | ConvertFrom-Json
        $slintPackage = $metadata.packages | Where-Object name -eq 'slint'
        $slintNode = $metadata.resolve.nodes | Where-Object id -eq $slintPackage.id
        $rendererFeatures = @($slintNode.features | Where-Object { $_ -like 'renderer-*' })
        $winitBackendPackage = $metadata.packages | Where-Object name -eq 'i-slint-backend-winit'
        $winitBackendNode = $metadata.resolve.nodes | Where-Object id -eq $winitBackendPackage.id
        $backendRendererFeatures = @($winitBackendNode.features | Where-Object { $_ -like 'renderer-*' })
        $rootFeatures = @(($metadata.resolve.nodes | Where-Object id -eq $metadata.resolve.root).features)
        if ($slintPackage.version -ne '1.18.1' -or $rendererFeatures.Count -ne 1 -or $rendererFeatures[0] -ne "renderer-$name" -or
            $backendRendererFeatures.Count -ne 1 -or $backendRendererFeatures[0] -ne "renderer-$name" -or
            ($Product -and $rootFeatures -notcontains 'product')) {
            throw "Unexpected Slint version or renderer features: slint=$($rendererFeatures -join ', ') root=$($rootFeatures -join ', ')"
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
        $sourcePaths += @('../scripts/test-data-compat.ps1', '../tests/fixtures/compat/rs08.json', '../cmd/rs08-compat/main.go', '../internal/compat/rs08.go', '../internal/compat/rs08_test.go', '../internal/app/compat_test.go')
        $sourcePaths += @('../internal/model/app.go', '../internal/config/config.go', '../internal/config/backup.go', '../internal/storage/json.go', '../internal/history/history.go', '../internal/indexer/indexer.go', '../internal/app/library.go')
        $sourcePaths += @('../scripts/test-search-compat.ps1', '../tests/fixtures/compat/rs09.json', '../cmd/rs09-tables/main.go', '../internal/compat/rs09.go', '../internal/app/rs09_compat_test.go', '../internal/app/service.go', '../internal/app/quick.go', '../internal/search/search.go', 'licenses/go-pinyin-MIT.txt', 'licenses/Go-BSD.txt')
        $sourcePaths += @('../scripts/test-discovery-compat.ps1', '../tests/fixtures/compat/rs10.json', '../cmd/rs10-compat/main.go', '../internal/indexer/filter.go', '../internal/icon/cache.go', '../internal/platform/apps_windows.go', '../internal/platform/item_actions_windows.go', '../internal/platform/icon_windows.go')
        $sourcePaths += @('../scripts/test-business-ui.ps1')
        $sourcePaths += @('../scripts/capture-rust-visual.ps1', '../scripts/package-rust-preview.ps1')
        $sourcePaths += @('../scripts/test-rust-preview.ps1')
        $sourcePaths += @('../scripts/lib/rust-license-policy.ps1')
        $sourcePaths += @('../scripts/audit-rust-licenses.ps1')
        $sourcePaths += @('../scripts/test-rust-metadata.ps1', '../scripts/package-rust-installer.ps1', '../scripts/test-rust-installer.ps1')
        $sourcePaths += @('../scripts/test-rust-user-install.ps1')
        $sourcePaths += @('../scripts/test-rust-nsis-update.ps1')
        $sourcePaths += @('../scripts/package-rust-product.ps1', '../scripts/test-rust-machine-install.ps1', '../scripts/prepare-rust-machine-install.ps1', '../scripts/test-rust-machine-cleanup.ps1')
        $sourcePaths += @('../scripts/package-rust-release.ps1')
        $sourcePaths += @('../scripts/test-pages-ui.ps1', '../tests/fixtures/compat/rs11.json', '../tests/fixtures/compat/rs11.expected.json', '../cmd/rs11-compat/main.go', '../internal/systemtools/catalog.go', '../internal/systemtools/rs11_compat_test.go', '../internal/update/version.go', '../internal/update/client.go', '../internal/platform/actions_windows.go', '../internal/platform/update_script.go')
        # Include UI modules, assets, native probes, and shared data-contract tests.
        $sourcePaths += @(Get-ChildItem -LiteralPath 'src', 'ui', 'examples', 'tests', 'windows', 'licenses' -Recurse -File | ForEach-Object {
            $_.FullName.Substring($environment.RustRoot.Length + 1).Replace('\', '/')
        })
        $sourcePaths = @($sourcePaths | Sort-Object -Unique)
        $sources = foreach ($source in $sourcePaths) {
            [ordered]@{ Path = $source; SHA256 = (Get-FileHash -LiteralPath $source -Algorithm SHA256).Hash }
        }
        $identity = [ordered]@{
            Renderer = $name
            ProductIdentity = $Product.IsPresent
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
            if ($SkipChecks) {
                & cargo build --locked --no-default-features --features $feature --example rs10_windows
                if ($LASTEXITCODE -ne 0) { throw "Business UI fixture helper build failed: $name" }
            }
            $businessDirectory = Join-Path $outputDirectory ('business-' + [guid]::NewGuid().ToString('N'))
            & (Join-Path $PSScriptRoot 'test-business-ui.ps1') -Executable $executable `
                -Helper (Join-Path $environment.RustRoot 'target/x86_64-pc-windows-msvc/debug/examples/rs10_windows.exe') -OutputDirectory $businessDirectory
            $pagesDirectory = Join-Path $outputDirectory ('pages-' + [guid]::NewGuid().ToString('N'))
            & (Join-Path $PSScriptRoot 'test-pages-ui.ps1') -Executable $executable `
                -Helper (Join-Path $environment.RustRoot 'target/x86_64-pc-windows-msvc/debug/examples/rs10_windows.exe') -OutputDirectory $pagesDirectory
        }
        [ordered]@{
            RecordedAtUtc = [DateTime]::UtcNow.ToString('o')
            Renderer = $name
            ProductIdentity = $Product.IsPresent
            ChecksPassed = -not $SkipChecks.IsPresent
            SmokeTestPassed = $SmokeTest.IsPresent
            ExecutableSHA256 = $identity.ExecutableSHA256
            BuildIdentitySHA256 = (Get-FileHash -LiteralPath (Join-Path $outputDirectory 'build-identity.json')).Hash
            CargoMetadataSHA256 = (Get-FileHash -LiteralPath (Join-Path $outputDirectory 'cargo-metadata.json')).Hash
        } | ConvertTo-Json | Set-Content -LiteralPath $verificationPath -Encoding utf8NoBOM
        Write-Host "Verified $name Release: $executable"
    }
} finally { Pop-Location }
