#Requires -Version 7.4
[CmdletBinding()]
param([Parameter(Mandatory)][ValidateSet('software','femtovg')][string]$Renderer)

$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $true
. (Join-Path $PSScriptRoot 'lib/rust-environment.ps1')
$environment = Initialize-VeloRustEnvironment -HostedCI
Push-Location $environment.RustRoot
try {
    Write-Output "Hosted compatibility check: MSVC $($environment.MsvcToolsVersion), link $($environment.LinkerFileVersion), SDK $($environment.WindowsSdkVersion)"
    & rustc --version
    & cargo fmt --all -- --check
    & (Join-Path $PSScriptRoot 'test-project-tools.ps1')
    $features = "renderer-$Renderer,product"
    & cargo clippy --locked --no-default-features --features $features --all-targets -- -D warnings
    # These tests need taskbar, global hotkey or modal focus ownership. They
    # remain required in verify-rust.ps1 on the interactive release runner.
    & cargo test --locked --no-default-features --features $features -- `
        --skip platform::windows::desktop::tests::native_endpoint_second_launch_tray_recovery_and_cleanup `
        --skip platform::windows::hotkey::tests::native_conflict_rollback_dispatch_rebind_and_cleanup `
        --skip platform::windows::system_tools::tests::native_recycle_confirmation_focuses_ok_and_enter_confirms_without_deleting
} finally { Pop-Location }
