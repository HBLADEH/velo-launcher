# Velo

[简体中文](README.md) · [Download](https://github.com/HBLADEH/velo-launcher/releases/latest) · [User guide (Chinese)](docs/user-guide.md)

A Chinese application launcher for **Windows 10/11 x64**. Press `Alt+Space`, search by name, Chinese pinyin or initials, and press `Enter` to launch.

**Velo v0.10.0 continues the Go 0.9.x version sequence and moves the product to Rust + Slint.** The native, single-process application requires no Go, Node.js, WebView2 or local HTTP server at runtime. Go/Wails is retired; Rust 0.1.0 users can also upgrade. See the [v0.10.0 release notes (Chinese)](docs/releases/0.10.0.md).

## Install

Download `velo-rust-x64-installer.exe` from the [latest release](https://github.com/HBLADEH/velo-launcher/releases/latest), run it and approve Windows UAC. Software is the default renderer. The OpenGL alternative is `velo-rust-femtovg-x64-installer.exe`; exit Velo from its tray menu before switching renderers.

This release provides system installers, with `SHA256SUMS.txt`, a license archive and a build manifest. `velo-launcher-amd64-installer.exe` is a byte-identical copy of the default Software installer for legacy Go update checks.

Official **Go v0.9.11 system installations** can check for updates in Settings / About. When running normally without elevation and the installation directory is not writable, the old updater selects the compatible installer for an in-place upgrade. You can also exit Velo from its tray and install manually. Uninstall other old builds while preserving data before installing Rust. Portable users should exit and run the installer manually. Existing hotkeys are preserved; new installations default to `Alt+Space`.

Settings, history, custom applications and pins remain in `%LOCALAPPDATA%\Velo`. Before opening the data store, Rust saves a byte-verified snapshot under `%LOCALAPPDATA%\VeloRustMigrationBackups`; failure prevents initialization. Uninstall preserves user data and recovery archives.

## Features

- Name, pinyin, initials, keyword and fuzzy search with local history ranking.
- Start menu, desktop, Windows Apps, Program Files and custom directory indexing.
- Pins, frequently used apps, system shortcuts, custom apps and file drops.
- List and grid views, keyboard navigation, native context menus, tray and global hotkey.
- Light/dark/system themes, configuration backups and SHA-256 verified Rust updates.

![Rust interface with synthetic data](docs/baselines/2026-11-07-slint-rs13/completion-2026-11-12/static/software/light-home.png)

## Develop

Product sources live in `rust/`. Install the exact Rust/MSVC/SDK versions locked by `rust/rust-toolchain.toml` and `rust/windows-toolchain.json`, following the [development guide](docs/development.md). Use PowerShell 7.4+.

```powershell
$env:GOTOOLCHAIN = 'go1.25.6' # Historical compatibility tests only
./scripts/verify-rust.ps1 -Product -SmokeTest
```

Both renderers are built separately under `rust/target/product-artifacts/`. Use isolated preview identities and data directories for development; see [Rust documentation](rust/README.md). Never test against installed user data.

Go/Wails sources and scripts remain solely as historical compatibility references, with no new product builds. See [legacy notes](docs/legacy-go-wails.md).

## Validation and license

[RS-14 / G4 acceptance](docs/baselines/2026-11-07-slint-rs14/machine-completion-2026-11-14/g4-acceptance.md) covers the existing Windows 11 / Intel UHD 630 host and isolated Windows 10 guest. Unavailable physical hardware was skipped. Renderer resource and animation tradeoffs remain documented in the [refactor plan](docs/rust-slint-refactor-plan.md); no universal memory budget is promised.

Report issues with the Windows version, Velo version and renderer, removing personal information. See [contribution guidelines](CONTRIBUTING.md).

Velo is [MIT licensed](LICENSE); dependencies retain their own terms. Slint uses Royalty-free 2.0, with the actual AboutSlint available in Settings / About. Complete notices ship in the installers and license archive. [Fluent icons](rust/ui/assets/fluent/LICENSE.txt) use MIT.
