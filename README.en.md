# Velo

[简体中文](README.md) · [Download](https://github.com/HBLADEH/velo-launcher/releases/latest) · [User guide (Chinese)](docs/user-guide.md)

A Chinese application launcher for **Windows 10/11 x64**. Press `Alt+Space`, search by name, Chinese pinyin or initials, and press `Enter` to launch.

**0.1.0 is the first official Rust + Slint release. Version numbering starts again at 0.1.0. Future product development uses Rust; Go/Wails 0.9.x is retired.** The native, single-process application requires no Go, Node.js, WebView2 or local HTTP server at runtime.

## Install

Download `velo-rust-x64-installer.exe` from the [latest release](https://github.com/HBLADEH/velo-launcher/releases/latest), run it and approve Windows UAC. Software is the default renderer. The OpenGL alternative is `velo-rust-femtovg-x64-installer.exe`; exit Velo from its tray menu before switching renderers.

This release provides system installers, with `SHA256SUMS.txt`, a license archive and a build manifest.

For migration, exit the old application and install manually. The verified official **Go v0.9.11 system installation** supports replacement in its existing directory. Uninstall other old builds while preserving data before installing Rust. The old Go updater cannot handle the reset version numbering.

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
