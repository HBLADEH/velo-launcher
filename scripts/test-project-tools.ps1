#Requires -Version 7.4
$ErrorActionPreference = 'Stop'
$temporaryBase = [IO.Path]::GetFullPath([IO.Path]::GetTempPath())
$fixture = Join-Path $temporaryBase ('velo-project-tools-' + [guid]::NewGuid().ToString('N'))

function Assert-Rejected([scriptblock]$Action, [string]$Message) {
    try { & $Action | Out-Null } catch {
        if ($_.Exception.Message -like $Message) { return }
        throw
    }
    throw "Expected rejection matching: $Message"
}

function Test-Cleanup {
    # A deterministic process snapshot keeps the test independent of the host.
    $processes = [Collections.Generic.List[object]]::new()
    $processes.Add(@{ Name='cargo.exe'; ExecutablePath="$fixture/other-rust-toolchain/cargo.exe" })
    function Get-CimInstance([string]$ClassName) {
        if ($ClassName -ne 'Win32_Process') { throw "Unexpected CIM class: $ClassName" }
        $processes
    }

    $clean = "$fixture/scripts/clean.ps1"
    New-Item -ItemType Directory -Path "$fixture/scripts", "$fixture/src", "$fixture/.dev-data", "$fixture/target/debug", "$fixture/target/artifacts", "$fixture/target/previews", "$fixture/target/tools" | Out-Null
    Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'clean.ps1') -Destination $clean
    Set-Content -LiteralPath "$fixture/Cargo.toml", "$fixture/src/main.rs" -Value 'synthetic project marker'
    $disposable = "$fixture/target/debug/disposable"
    $retained = @("$fixture/target/artifacts/keep", "$fixture/target/previews/keep", "$fixture/target/tools/keep")
    Set-Content -LiteralPath (@("$fixture/.dev-data/keep.json", $disposable) + $retained) -Value 'synthetic'

    & $clean -WhatIf
    if (-not (Test-Path -LiteralPath $disposable)) { throw 'WhatIf deleted files.' }
    $processes.Add(@{ Name='velo.exe'; ExecutablePath="$fixture\target\previews\velo.exe" })
    Assert-Rejected { & $clean } '*processes in target before cleaning*'
    $processes.RemoveAt(1)
    $lockPath = Join-Path $fixture 'target/debug/.cargo-build-lock'
    $lock = [IO.File]::Open($lockPath, 'Create', 'ReadWrite', 'ReadWrite')
    try { Assert-Rejected { & $clean } '*Cargo build using this target directory*' }
    finally { $lock.Dispose() }
    if (-not (Test-Path -LiteralPath $disposable)) { throw 'Cleanup removed files while target was in use.' }

    & $clean
    if ((Test-Path -LiteralPath "$fixture/target/debug") -or @($retained | Where-Object { -not (Test-Path -LiteralPath $_) }).Count) {
        throw 'Default cleanup did not preserve current artifacts or remove the build cache.'
    }
    & $clean -All
    if ((Test-Path -LiteralPath "$fixture/target") -or -not (Test-Path -LiteralPath "$fixture/.dev-data/keep.json")) {
        throw 'Full cleanup changed development data or left build outputs.'
    }
}

function Test-ReleaseAssets {
    $assets = Join-Path $fixture 'assets'
    New-Item -ItemType Directory -Path $assets | Out-Null
    $check = Join-Path $PSScriptRoot 'check-release-assets.ps1'
    $manifest = 'release-manifest.json'
    $license = 'velo-rust-9.8.7-licenses.zip'
    $software = 'velo-rust-x64-installer.exe'
    $alias = 'velo-launcher-amd64-installer.exe'
    $names = @($alias, $software, 'velo-rust-femtovg-x64-installer.exe', $manifest, $license)
    $sums = Join-Path $assets 'SHA256SUMS.txt'
    function Write-TestChecksums([string[]]$Names) {
        $Names | ForEach-Object { '{0}  {1}' -f (Get-FileHash -LiteralPath (Join-Path $assets $_)).Hash, $_ } | Set-Content -LiteralPath $sums
    }
    foreach ($name in $names) { Set-Content -LiteralPath (Join-Path $assets $name) -Value 'synthetic asset bytes' }
    Set-Content -LiteralPath (Join-Path $assets $manifest) -Value '{"Version":"9.8.7"}'
    Write-TestChecksums $names
    & $check -Directory $assets

    Add-Content -LiteralPath (Join-Path $assets $license) -Value 'corrupt'
    Assert-Rejected { & $check -Directory $assets } "*SHA-256 mismatch: $license"
    Remove-Item -LiteralPath (Join-Path $assets $license)
    $withoutLicense = @($names | Where-Object { $_ -ne $license })
    Write-TestChecksums $withoutLicense
    Assert-Rejected { & $check -Directory $assets } "*not covered by checksums: $license"
    Set-Content -LiteralPath (Join-Path $assets $license) -Value 'synthetic asset bytes'

    Add-Content -LiteralPath (Join-Path $assets $software) -Value 'corrupt'
    Write-TestChecksums $names
    Assert-Rejected { & $check -Directory $assets } '*legacy installer alias must contain exactly*'
    Set-Content -LiteralPath $sums -Value (('a' * 64) + '  ../outside.exe')
    Assert-Rejected { & $check -Directory $assets } '*non-local asset name*'
}

function Test-ReleaseInputGate {
    $source = Join-Path $fixture 'release-source'
    $roots = '.cargo', 'src', 'ui', 'examples', 'tests', 'windows', 'licenses', 'scripts/lib',
        'target/product-artifacts/software', 'target/product-artifacts/femtovg'
    New-Item -ItemType Directory -Path ($roots | ForEach-Object { Join-Path $source $_ }) -Force | Out-Null
    Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'package-rust-release.ps1') -Destination "$source/scripts/"
    Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'lib/rust-build-identity.ps1') -Destination "$source/scripts/lib/"
    Set-Content -LiteralPath "$source/Cargo.toml", "$source/Cargo.lock", "$source/rust-toolchain.toml", "$source/windows-toolchain.json", "$source/build.rs", "$source/LICENSE" -Value 'synthetic input'
    Set-Content -LiteralPath "$source/windows/release-policy.json" -Value '{"G4Passed":true,"PublicReleaseApproved":true,"RedistributionCleared":true,"ReleaseTag":"v9.8.7"}'
    Set-Content -LiteralPath "$source/licenses/existing.txt" -Value 'synthetic notice'
    . "$source/scripts/lib/rust-build-identity.ps1"

    # Model HEAD and status without committing or configuring any Git repository.
    $gitState = @{ Commit = ('1' * 40); Dirty = $false }
    function git {
        if ($args[0] -ne '-C' -or $args[1] -ne $source) { throw 'Git query left the release fixture.' }
        Set-Variable -Name LASTEXITCODE -Value 0 -Scope 1
        switch ($args[2]) {
            'rev-parse' { $gitState.Commit }
            'status' { if ($gitState.Dirty) { '?? license-input.txt' } }
            default { throw "Unexpected Git query: $($args[2])" }
        }
    }

    $sources = @(Get-VeloRustSources -Repository $source)
    function Write-TestReceipt([string]$Renderer, [switch]$Dirty) {
        $artifact = Join-Path $source "target/product-artifacts/$Renderer"
        Set-Content -LiteralPath "$artifact/velo-slint.exe", "$artifact/cargo-metadata.json" -Value 'synthetic artifact, never executed'
        $exeHash = (Get-FileHash -LiteralPath "$artifact/velo-slint.exe").Hash
        @{
            Renderer=$Renderer; ProductIdentity=$true; Target='x86_64-pc-windows-msvc'
            ExecutableSHA256=$exeHash; WorkspaceCommit=$gitState.Commit; WorkspaceDirty=$Dirty.IsPresent; Sources=$sources
        } | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath "$artifact/build-identity.json"
        @{
            Renderer=$Renderer; ProductIdentity=$true; ChecksPassed=$true; SmokeTestPassed=$true; ExecutableSHA256=$exeHash
            BuildIdentitySHA256=(Get-FileHash -LiteralPath "$artifact/build-identity.json").Hash
            CargoMetadataSHA256=(Get-FileHash -LiteralPath "$artifact/cargo-metadata.json").Hash
        } | ConvertTo-Json | Set-Content -LiteralPath "$artifact/verification.json"
    }

    Write-TestReceipt software
    Write-TestReceipt femtovg
    foreach ($renderer in 'software', 'femtovg') {
        Read-VeloVerifiedBuild -Repository $source -Renderer $renderer -Product -Release | Out-Null
    }
    $output = Join-Path $source 'target/release'
    function Assert-PackagingDidNotStart([string]$Message) {
        Assert-Rejected { & "$source/scripts/package-rust-release.ps1" -OutputDirectory $output -Makensis 'unused.exe' } $Message
        if (Test-Path -LiteralPath $output) { throw 'Rejected release created a staging directory.' }
    }

    $gitState.Dirty = $true
    Assert-PackagingDidNotStart '*clean current worktree*'
    $gitState.Dirty = $false
    Write-TestReceipt software -Dirty
    Assert-PackagingDidNotStart '*software release receipt must bind the current clean source commit*'
    $gitState.Commit = '2' * 40
    Write-TestReceipt software
    Assert-PackagingDidNotStart '*femtovg release receipt must bind the current clean source commit*'
    Write-TestReceipt femtovg
    Set-Content -LiteralPath "$source/licenses/added.txt" -Value 'additional input, even if Git ignores it'
    Assert-PackagingDidNotStart '*Source added after verification: licenses/added.txt*'
    Remove-Item -LiteralPath "$source/licenses/added.txt"
    Add-Content -LiteralPath "$source/licenses/existing.txt" -Value 'changed input'
    Assert-PackagingDidNotStart '*Source changed after verification: licenses/existing.txt*'
    Remove-Item -LiteralPath "$source/licenses/existing.txt"
    Assert-PackagingDidNotStart '*Source removed after verification: licenses/existing.txt*'
}

function Test-ReleasePublisherValidation {
    $source = Join-Path $fixture 'release-publisher'
    $assets = Join-Path $source 'target/release-assets'
    New-Item -ItemType Directory -Path (Join-Path $source 'scripts'), (Join-Path $source 'docs/releases'), $assets -Force | Out-Null
    Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'publish-rust-release.ps1') -Destination (Join-Path $source 'scripts/publish-rust-release.ps1')
    Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'check-release-assets.ps1') -Destination (Join-Path $source 'scripts/check-release-assets.ps1')
    Set-Content -LiteralPath (Join-Path $source 'docs/releases/9.8.7.md') -Value 'Synthetic release notes.'
    [IO.File]::WriteAllText((Join-Path $assets 'velo-rust-x64-installer.exe'), 'software-bytes')
    [IO.File]::WriteAllText((Join-Path $assets 'velo-launcher-amd64-installer.exe'), 'software-bytes')
    [IO.File]::WriteAllText((Join-Path $assets 'velo-rust-femtovg-x64-installer.exe'), 'femtovg-bytes')
    [IO.File]::WriteAllText((Join-Path $assets 'velo-rust-9.8.7-licenses.zip'), 'license-bytes')
    $commit = '1' * 40
    $builds = @(
        @{ Renderer='software'; Installer='velo-rust-x64-installer.exe'; SourceCommit=$commit },
        @{ Renderer='femtovg'; Installer='velo-rust-femtovg-x64-installer.exe'; SourceCommit=$commit }
    )
    foreach ($build in $builds) {
        $build.InstallerSHA256 = (Get-FileHash -LiteralPath (Join-Path $assets $build.Installer)).Hash
    }
    function Write-ReleaseTestChecksums {
        @('velo-launcher-amd64-installer.exe', 'velo-rust-x64-installer.exe', 'velo-rust-femtovg-x64-installer.exe',
            'velo-rust-9.8.7-licenses.zip', 'release-manifest.json') |
            ForEach-Object { '{0}  {1}' -f (Get-FileHash -LiteralPath (Join-Path $assets $_)).Hash.ToLowerInvariant(), $_ } |
            Set-Content -LiteralPath (Join-Path $assets 'SHA256SUMS.txt')
    }
    @{ Tag='v9.8.7'; Version='9.8.7'; PublicReleaseApproved=$true; RedistributionCleared=$true; Builds=$builds } |
        ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $assets 'release-manifest.json')
    Write-ReleaseTestChecksums
    $publisher = Join-Path $source 'scripts/publish-rust-release.ps1'
    & $publisher -Tag 'v9.8.7' -Commit $commit -Directory $assets -ValidateOnly | Out-Null
    Assert-Rejected { & $publisher -Tag 'v9.8.7' -Commit ('2' * 40) -Directory $assets -ValidateOnly } '*not bound to the tagged commit*'

    # Stub every GitHub command, using real digests from the synthetic assets.
    $state = @{ Commit = $commit; Exists = $false; IsDraft = $true; BadDigest = $true; Target = $null }
    $uploaded = [Collections.Generic.List[object]]::new()
    function gh {
        $repo = 'HBLADEH/velo-launcher'
        if ($args[0] -eq 'api') {
            switch -Exact ($args[1]) {
                "repos/$repo/commits/refs/tags/v9.8.7" { return $state.Commit }
                "repos/$repo/releases?per_page=100" { if ($state.Exists) { return 'v9.8.7' }; return }
                default { throw "Unexpected GitHub endpoint: $($args[1])" }
            }
        }
        if ($args[0] -ne 'release' -or $args[2] -ne 'v9.8.7' -or $repo -notin $args) { throw 'Unexpected GitHub release command.' }
        switch -Exact ($args[1]) {
            'create' {
                if ('--draft' -notin $args) { throw 'Release must start as a draft.' }
                $targetOption = [array]::IndexOf($args, '--target')
                if ($targetOption -ge 0) {
                    $state.Target = $args[$targetOption + 1]
                } elseif ('--verify-tag' -notin $args) {
                    throw 'A draft requires an existing tag or explicit target commit.'
                }
                $state.Exists = $true
            }
            'upload' {
                $repoOption = [array]::IndexOf($args, '--repo')
                if (-not $state.Exists -or -not $state.IsDraft -or $repoOption -lt 4) { throw 'Invalid draft upload.' }
                $paths = $args[3..($repoOption - 1)]
                if ($uploaded.Count -eq 0 -and ($paths.Count -ne 1 -or [IO.Path]::GetFileName($paths[0]) -ne 'velo-launcher-amd64-installer.exe')) {
                    throw 'The legacy alias must be uploaded by itself before all other assets.'
                }
                foreach ($path in $paths) {
                    $digest = (Get-FileHash -LiteralPath $path).Hash.ToLowerInvariant()
                    if ($state.BadDigest -and $uploaded.Count -eq 0) { $digest = '0' * 64 }
                    $uploaded.Add(@{ name = [IO.Path]::GetFileName($path); state = 'uploaded'; digest = "sha256:$digest" })
                }
            }
            'view' { @{ tagName = 'v9.8.7'; isDraft = $state.IsDraft; targetCommitish = $state.Target; assets = $uploaded.ToArray() } | ConvertTo-Json -Depth 5 -Compress }
            'download' {
                if (-not $state.Exists -or -not $state.IsDraft) { throw 'No draft to download.' }
                $destination = $args[([array]::IndexOf($args, '--dir') + 1)]
                New-Item -ItemType Directory -Path $destination | Out-Null
                foreach ($asset in $uploaded) {
                    Copy-Item -LiteralPath (Join-Path $assets $asset.name) -Destination $destination
                }
            }
            'edit' {
                if ($uploaded.Count -ne 6 -or $state.BadDigest -or '--draft=false' -notin $args) { throw 'Attempt to publish an unverified draft.' }
                $state.IsDraft = $false
            }
            default { throw "Unexpected GitHub command: $($args[1])" }
        }
    }
    $token = $env:GH_TOKEN
    try {
        $env:GH_TOKEN = 'local-test-stub'
        $publish = { & $publisher -Tag 'v9.8.7' -Commit $commit -Directory $assets }
        $state.Commit = '2' * 40
        Assert-Rejected $publish '*Remote release tag no longer points*'
        if ($state.Exists) { throw 'Publisher created a release for a moved tag.' }
        $state.Commit = $commit
        Assert-Rejected $publish '*Uploaded asset failed SHA-256 verification*'
        if (-not $state.IsDraft) { throw 'Digest mismatch did not leave a draft.' }
        Assert-Rejected $publish '*A release or draft already exists*'

        $state.Exists = $false
        $state.BadDigest = $false
        $uploaded.Clear()
        & $publish | Out-Null
        if ($state.IsDraft) { throw 'A fully verified draft was not published.' }

        $state.Exists = $false
        $state.IsDraft = $true
        $uploaded.Clear()
        $staged = @(& $publisher -Tag 'v9.8.7' -Commit $commit -Directory $assets -StageDraft)
        $checksumDigest = (Get-FileHash -LiteralPath (Join-Path $assets 'SHA256SUMS.txt')).Hash.ToLowerInvariant()
        if (-not $state.IsDraft -or "Velo-Release-Draft: $checksumDigest" -notin $staged) { throw 'Staging must return the checksum list digest and leave a draft.' }
        Assert-Rejected {
            & $publisher -Tag 'v9.8.7' -Commit $commit -Directory (Join-Path $source 'wrong-digest') -DraftChecksumsSHA256 ('0' * 64)
        } '*does not match the digest in the tag annotation*'
        if (-not $state.IsDraft) { throw 'A draft with the wrong checksum list digest was published.' }
        & $publisher -Tag 'v9.8.7' -Commit $commit -Directory (Join-Path $source 'verified-draft') -DraftChecksumsSHA256 $checksumDigest | Out-Null
        if ($state.IsDraft) { throw 'The verified staged draft was not published.' }
    } finally { $env:GH_TOKEN = $token }
}

function Test-ReleaseTagGate {
    $PSNativeCommandUseErrorActionPreference = $true
    $source = Join-Path $fixture 'release-tag-gate'
    New-Item -ItemType Directory -Path (Join-Path $source 'scripts'), (Join-Path $source 'windows'),
        (Join-Path $source 'docs/releases') -Force | Out-Null
    Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'check-rust-release-tag.ps1') -Destination (Join-Path $source 'scripts/check-rust-release-tag.ps1')
    Set-Content -LiteralPath (Join-Path $source 'Cargo.toml') -Value '[package]', 'name = "release-fixture"', 'version = "9.8.7"'
    Set-Content -LiteralPath (Join-Path $source 'windows/release-policy.json') -Value '{"ReleaseTag":"v9.8.7","G4Passed":true,"RedistributionCleared":true,"PublicReleaseApproved":true}'
    Set-Content -LiteralPath (Join-Path $source 'docs/releases/9.8.7.md') -Value 'Synthetic release notes.'
    & git -C $source init --initial-branch=main --template= | Out-Null
    & git -C $source config --local user.name 'Release Fixture'
    & git -C $source config --local user.email release-fixture@example.invalid
    & git -C $source config --local core.hooksPath (Join-Path $source 'unused-hooks')
    & git -C $source config --local commit.gpgsign false
    & git -C $source config --local tag.gpgSign false
    & git -C $source add scripts/check-rust-release-tag.ps1 Cargo.toml windows/release-policy.json docs/releases/9.8.7.md
    & git -C $source commit -m 'synthetic release candidate' | Out-Null
    $mainCommit = & git -C $source rev-parse HEAD
    & git -C $source update-ref refs/remotes/origin/main $mainCommit
    $draftDigest = 'a' * 64
    & git -C $source tag -a v9.8.7 -m 'Velo v9.8.7' -m "Velo-Release-Draft: $draftDigest"
    $trailer = & git -C $source for-each-ref '--format=%(trailers:key=Velo-Release-Draft,valueonly)' refs/tags/v9.8.7
    if (($trailer | Where-Object { $_ }) -ne $draftDigest) { throw 'The release workflow cannot read the staged manifest digest from the annotated tag.' }
    $gate = Join-Path $source 'scripts/check-rust-release-tag.ps1'
    $tagObject = & git -C $source rev-parse refs/tags/v9.8.7
    foreach ($eventSha in $mainCommit, $tagObject) {
        $accepted = & $gate -Tag v9.8.7 -EventSha $eventSha
        if (@($accepted).Count -ne 1 -or $accepted -ne $mainCommit) { throw 'Valid main version tag was not returned.' }
    }

    & git -C $source tag v9.8.6
    Assert-Rejected { & $gate -Tag v9.8.6 -EventSha $mainCommit } '*Push an annotated release tag*'
    & git -C $source tag -fa v9.8.6 -m 'wrong version' | Out-Null
    Assert-Rejected { & $gate -Tag v9.8.6 -EventSha $mainCommit } '*Tag, Cargo package version and release policy must match*'
    $policyPath = Join-Path $source 'windows/release-policy.json'
    Set-Content -LiteralPath $policyPath -Value '{"ReleaseTag":"v9.8.7","G4Passed":true,"RedistributionCleared":true,"PublicReleaseApproved":false}'
    Assert-Rejected { & $gate -Tag v9.8.7 -EventSha $mainCommit } '*explicit PublicReleaseApproved approval*'

    & git -C $source checkout -q -B codex/unreleased $mainCommit
    Set-Content -LiteralPath (Join-Path $source 'release-change.txt') -Value 'not merged to main'
    & git -C $source add release-change.txt
    & git -C $source commit -m 'unreleased commit' | Out-Null
    $offMain = & git -C $source rev-parse HEAD
    Assert-Rejected { & $gate -Tag v9.8.7 -EventSha $mainCommit } '*must name the same commit*'
    & git -C $source tag -fa v9.8.7 -m 'off-main tag' | Out-Null
    Assert-Rejected { & $gate -Tag v9.8.7 -EventSha $offMain } '*reachable from origin/main*'
}

try {
    Test-Cleanup
    Test-ReleaseAssets
    Test-ReleaseInputGate
    Test-ReleasePublisherValidation
    Test-ReleaseTagGate
    Write-Output 'Project tools passed: scoped cleanup, release checksums, fail-before-staging and tag publisher validations.'
} finally {
    $path = [IO.Path]::GetFullPath($fixture)
    if (-not $path.StartsWith($temporaryBase, [StringComparison]::OrdinalIgnoreCase) -or [IO.Path]::GetFileName($path) -notlike 'velo-project-tools-*') { throw 'Test cleanup left its synthetic root.' }
    Remove-Item -LiteralPath $path -Recurse -Force
}
