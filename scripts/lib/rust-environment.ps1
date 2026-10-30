function Initialize-VeloRustEnvironment {
    $ErrorActionPreference = 'Stop'
    if ([Environment]::OSVersion.Platform -ne 'Win32NT') { throw 'Rust validation currently requires Windows x64.' }
    $rustRoot = Join-Path (Split-Path -Parent (Split-Path -Parent $PSScriptRoot)) 'rust'
    $toolchain = Get-Content -LiteralPath (Join-Path $rustRoot 'windows-toolchain.json') -Raw | ConvertFrom-Json
    foreach ($override in @('RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS', 'CARGO_BUILD_RUSTFLAGS', 'CARGO_BUILD_TARGET', 'CARGO_TARGET_DIR', 'RUSTC', 'RUSTC_WRAPPER', 'RUSTC_WORKSPACE_WRAPPER', 'RUSTUP_TOOLCHAIN', 'CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_RUSTFLAGS')) {
        if ([Environment]::GetEnvironmentVariable($override)) { throw "Unset $override before using the locked Rust validation entry point." }
    }

    # rustup may have been installed without changing the user's global PATH.
    $cargoCommand = Get-Command cargo.exe -ErrorAction SilentlyContinue
    if (-not $cargoCommand) {
        $cargoBin = Join-Path $env:USERPROFILE '.cargo/bin'
        if (-not (Test-Path -LiteralPath (Join-Path $cargoBin 'cargo.exe'))) {
            throw 'Install rustup and the toolchain in rust/rust-toolchain.toml first; see rust/README.md.'
        }
        $env:PATH = "$cargoBin;$env:PATH"
    }

    $vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio/Installer/vswhere.exe'
    if (-not (Test-Path -LiteralPath $vswhere)) { throw 'Visual Studio Installer/vswhere.exe was not found.' }
    $installations = @(& $vswhere -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath)
    if ($LASTEXITCODE -ne 0) { throw 'vswhere failed.' }
    $vsPath = $installations | Where-Object {
        Test-Path -LiteralPath (Join-Path $_ "VC/Tools/MSVC/$($toolchain.msvcToolsVersion)/bin/Hostx64/x64/link.exe")
    } | Select-Object -First 1
    if (-not $vsPath) { throw "MSVC $($toolchain.msvcToolsVersion) x64 is required by rust/windows-toolchain.json." }
    Import-Module (Join-Path $vsPath 'Common7/Tools/Microsoft.VisualStudio.DevShell.dll')
    Enter-VsDevShell -VsInstallPath $vsPath -SkipAutomaticLocation -Arch amd64 -HostArch amd64 `
        -DevCmdArguments "-vcvars_ver=$($toolchain.msvcToolsVersion) -winsdk=$($toolchain.windowsSdkVersion)" | Out-Host
    if ($env:VCToolsVersion.TrimEnd('\') -ne $toolchain.msvcToolsVersion -or
        $env:WindowsSDKVersion.TrimEnd('\') -ne $toolchain.windowsSdkVersion) {
        throw 'The active MSVC/Windows SDK does not match rust/windows-toolchain.json.'
    }
    $linker = Join-Path $vsPath "VC/Tools/MSVC/$($toolchain.msvcToolsVersion)/bin/Hostx64/x64/link.exe"
    $compiler = Join-Path $vsPath "VC/Tools/MSVC/$($toolchain.msvcToolsVersion)/bin/Hostx64/x64/cl.exe"
    $compilerVersion = (Get-Item -LiteralPath $compiler).VersionInfo.FileVersion
    $linkerVersion = (Get-Item -LiteralPath $linker).VersionInfo.FileVersion
    if ($compilerVersion -ne $toolchain.compilerFileVersion -or $linkerVersion -ne $toolchain.linkerFileVersion) {
        throw 'The compiler/linker file versions do not match rust/windows-toolchain.json.'
    }
    $env:CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER = $linker
    [pscustomobject]@{
        RustRoot = $rustRoot
        MsvcToolsVersion = $toolchain.msvcToolsVersion
        WindowsSdkVersion = $toolchain.windowsSdkVersion
        CompilerFileVersion = $compilerVersion
        LinkerFileVersion = $linkerVersion
        Linker = $linker
    }
}
