param(
    [ValidateSet('All', 'FemtoVG', 'Software')][string]$Renderer = 'All',
    [switch]$SkipChecks,
    [switch]$SmokeTest,
    [switch]$Product,
    [switch]$HostedCI,
    [switch]$ChecksOnly
)
$ErrorActionPreference = 'Stop'
if ($HostedCI -and (-not $Product -or $SkipChecks -or $SmokeTest)) {
    throw 'Hosted validation requires -Product and runs the non-desktop checks.'
}
if ($ChecksOnly -and -not $HostedCI) { throw '-ChecksOnly requires -HostedCI.' }
. (Join-Path $PSScriptRoot 'lib/rust-environment.ps1')
. (Join-Path $PSScriptRoot 'lib/rust-build-identity.ps1')
$environment = Initialize-VeloRustEnvironment -HostedCI:$HostedCI
if ($HostedCI) {
    Write-Output "Hosted validation: MSVC $($environment.MsvcToolsVersion), link $($environment.LinkerFileVersion), SDK $($environment.WindowsSdkVersion)"
}
$renderers = if ($Renderer -eq 'All') { @('femtovg', 'software') } else { @($Renderer.ToLowerInvariant()) }
$artifactGroup = if ($Product) { 'product-artifacts' } else { 'artifacts' }
if (-not $ChecksOnly) {
    foreach ($name in $renderers) {
        $receipt = Join-Path $environment.RustRoot "target/$artifactGroup/$name/verification.json"
        if (Test-Path -LiteralPath $receipt) { Remove-Item -LiteralPath $receipt }
    }
}
Push-Location $environment.RustRoot
try {
    $toolchainText = Get-Content -LiteralPath 'rust-toolchain.toml' -Raw
    $pinnedChannel = [regex]::Match($toolchainText, '(?m)^channel\s*=\s*"([^"]+)"').Groups[1].Value
    $pinnedRust = [regex]::Match($pinnedChannel, '^(\d+\.\d+\.\d+)-x86_64-pc-windows-msvc$').Groups[1].Value
    if (-not $pinnedRust) { throw 'Pin an exact Rust version and x86_64-pc-windows-msvc host in rust-toolchain.toml.' }
    $rustcVersion = @(& rustc --version --verbose)
    if ($LASTEXITCODE -ne 0) { throw 'The pinned Rust toolchain is unavailable.' }
    Write-Output $rustcVersion
    $cargoVersion = & cargo --version
    if ($LASTEXITCODE -ne 0) { throw 'cargo is unavailable.' }
    Write-Output $cargoVersion
    if ($rustcVersion -notcontains "release: $pinnedRust" -or
        $rustcVersion -notcontains 'host: x86_64-pc-windows-msvc' -or
        $cargoVersion -notlike "cargo $pinnedRust *") {
        throw 'rustc/cargo do not match rust-toolchain.toml.'
    }
    if (-not $ChecksOnly) {
        $workspace = Get-VeloWorkspaceIdentity -Repository $environment.RustRoot
        $sources = @(Get-VeloRustSources -Repository $environment.RustRoot)
    }
    if (-not $SkipChecks) {
        & (Join-Path $PSScriptRoot 'test-project-tools.ps1')
        & cargo fmt --all -- --check
        if ($LASTEXITCODE -ne 0) { throw 'Rust formatting failed.' }
    }
    foreach ($name in $renderers) {
        $feature = "renderer-$name"
        if ($Product) { $feature += ',product' }
        if (-not $SkipChecks) {
            & cargo clippy --locked --no-default-features --features $feature --all-targets -- -D warnings
            if ($LASTEXITCODE -ne 0) { throw "clippy failed: $name" }
            $testArguments = @('test', '--locked', '--no-default-features', '--features', $feature)
            if ($HostedCI) {
                # These tests need taskbar, global hotkey or modal focus ownership.
                $testArguments += @(
                    '--',
                    '--skip', 'platform::windows::desktop::tests::native_endpoint_second_launch_tray_recovery_and_cleanup',
                    '--skip', 'platform::windows::hotkey::tests::native_conflict_rollback_dispatch_rebind_and_cleanup',
                    '--skip', 'platform::windows::system_tools::tests::native_recycle_confirmation_focuses_ok_and_enter_confirms_without_deleting'
                )
            }
            & cargo @testArguments
            if ($LASTEXITCODE -ne 0) { throw "Rust tests failed: $name" }
        }
        if ($ChecksOnly) { continue }
        if (-not $SkipChecks -and -not $HostedCI) {
            & (Join-Path $PSScriptRoot 'test-data-compat.ps1') -Renderer $name
            $nativeDirectory = Join-Path $environment.RustRoot ('target/compat-rs10/' + [guid]::NewGuid().ToString('N'))
            New-Item -ItemType Directory -Path $nativeDirectory -Force | Out-Null
            & cargo build --locked --no-default-features --features $feature --example rs11_update_capture
            if ($LASTEXITCODE -ne 0) { throw "RS-11 update capture helper build failed: $name" }
            & cargo run --locked --no-default-features --features $feature --example rs10_windows -- (Join-Path $nativeDirectory 'native.json')
            if ($LASTEXITCODE -ne 0) { throw "RS-10 native Windows verification failed: $name" }
            & cargo run --locked --no-default-features --features $feature --example rs11_windows -- `
                (Join-Path $environment.RustRoot 'target/x86_64-pc-windows-msvc/debug/examples/rs11_update_capture.exe') (Join-Path $nativeDirectory 'rs11-native.json')
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
        if (-not $SkipChecks -and -not $HostedCI) {
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
            WorkspaceCommit = $workspace.Commit
            WorkspaceDirty = $workspace.Dirty
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
                foreach ($example in 'rs11_update_capture','rs10_windows') {
                    & cargo build --locked --no-default-features --features $feature --example $example
                    if ($LASTEXITCODE -ne 0) { throw "Business UI fixture helper build failed: $name/$example" }
                }
            }
            $businessDirectory = Join-Path $outputDirectory ('business-' + [guid]::NewGuid().ToString('N'))
            & (Join-Path $PSScriptRoot 'test-business-ui.ps1') -Executable $executable `
                -Helper (Join-Path $environment.RustRoot 'target/x86_64-pc-windows-msvc/debug/examples/rs10_windows.exe') -OutputDirectory $businessDirectory
            $pagesDirectory = Join-Path $outputDirectory ('pages-' + [guid]::NewGuid().ToString('N'))
            & (Join-Path $PSScriptRoot 'test-pages-ui.ps1') -Executable $executable `
                -Helper (Join-Path $environment.RustRoot 'target/x86_64-pc-windows-msvc/debug/examples/rs10_windows.exe') -OutputDirectory $pagesDirectory
        }
        # A successful test cannot certify source files edited while it was running.
        Assert-VeloRustSources -Repository $environment.RustRoot -Recorded $sources
        $current = Get-VeloWorkspaceIdentity -Repository $environment.RustRoot
        if ($current.Commit -ne $workspace.Commit -or $current.Dirty -ne $workspace.Dirty) {
            throw 'Worktree changed during verification; run verification again.'
        }
        [ordered]@{
            RecordedAtUtc = [DateTime]::UtcNow.ToString('o')
            Renderer = $name
            ProductIdentity = $Product.IsPresent
            ChecksPassed = -not $SkipChecks.IsPresent
            SmokeTestPassed = $SmokeTest.IsPresent
            VerificationScope = if ($HostedCI) { 'hosted-ci' } else { 'desktop' }
            ExecutableSHA256 = $identity.ExecutableSHA256
            BuildIdentitySHA256 = (Get-FileHash -LiteralPath (Join-Path $outputDirectory 'build-identity.json')).Hash
            CargoMetadataSHA256 = (Get-FileHash -LiteralPath (Join-Path $outputDirectory 'cargo-metadata.json')).Hash
        } | ConvertTo-Json | Set-Content -LiteralPath $verificationPath -Encoding utf8NoBOM
        Write-Host "Verified $name Release: $executable"
    }
} finally { Pop-Location }
