# Rust + Slint 开发与发布

[返回首页](../README.md) · [工程说明](rust-engineering.md) · [文档索引](README.md)

项目是仓库根目录下的单一 Rust + Slint Cargo 应用，`src/` 为 Rust 源码，`ui/` 为 Slint 页面，`tests/` 为回归测试，`windows/` 为原生打包资源。编译和完整测试使用下面锁定的原生工具链。

## 环境

Windows 10/11 x64、PowerShell 7.4+、rustup，以及锁定的 Rust **1.99.1-x86_64-pc-windows-msvc**、MSVC **14.50.35717**、编译器 **19.50.35719.0**、链接器 **14.50.35719.0** 和 Windows SDK **10.0.26100.0**。安装对应 Visual Studio C++ x64 组件，版本由 `rust-toolchain.toml`、`windows-toolchain.json` 校验；不自动替换为最新版本。

```powershell
rustup toolchain install 1.99.1-x86_64-pc-windows-msvc --profile minimal --component rustfmt --component clippy
```

脚本自动初始化匹配的 MSVC/SDK 环境，首次下载依赖需要联网。每份构建只启用一种 renderer；Software 为默认，FemtoVG 为 OpenGL 替代。

## 日常编译和预览

从仓库根目录运行：

```powershell
# 编译 Software Release 并启动独立预览；跳过完整测试
./scripts/run-preview.ps1 -Build
# 已编译时直接启动
./scripts/run-preview.ps1
# 只编译，不启动
./scripts/verify-rust.ps1 -Renderer Software -SkipChecks
```

预览用 **Ctrl+Alt+K** 呼出，数据保存在 `.dev-data/manual-preview/`，不会接触正式 `%LOCALAPPDATA%\Velo`。从托盘退出预览后再重新构建。`run-preview.ps1` 使用 `target/previews/<renderer>/` 的副本，避免锁定构建产物。Everything 默认关闭；测试前运行 Everything，并在设置 → 搜索中启用、保存后按 Tab 切换。

```powershell
# 替代渲染器
./scripts/run-preview.ps1 -Build -Renderer femtovg
# 两 renderer 完整检查：格式、clippy、单测、固定契约、原生集成、Release 和界面冒烟
./scripts/verify-rust.ps1 -SmokeTest
# 发版前验证正式 product feature
./scripts/verify-rust.ps1 -Product -SmokeTest
```

产物为 `target/artifacts/<renderer>/velo-slint.exe`；加 `-Product` 后为 `target/product-artifacts/<renderer>/velo-slint.exe`。正式 exe 普通启动需要系统安装回执，日常开发使用预览入口。`-SkipChecks` 仅供迭代，不是正式验收证据。真实 IME、UAC、多屏与安装器仍需对应桌面验证。

直接使用 Cargo 时，先在根目录加载原生环境：

```powershell
. ./scripts/lib/rust-environment.ps1
$veloEnvironment = Initialize-VeloRustEnvironment
cargo build --release --locked --no-default-features --features renderer-software
```

## 清理磁盘空间

`target/` 是 Cargo 缓存、测试、预览和打包产物，占用的是磁盘空间；删除后下次编译会重建。

```powershell
# 删除编译缓存和临时测试，保留当前 artifacts、product-artifacts、previews、tools
./scripts/clean.ps1
# 先查看将清理的目录
./scripts/clean.ps1 -WhatIf
# 连当前产物一起清理，类似 cargo clean；.dev-data 保留
./scripts/clean.ps1 -All
```

清理前退出本项目的预览及构建、测试进程，清理期间不要启动新构建。脚本检查本项目 `target/` 中的运行程序和 Cargo 锁，不受其他仓库编译影响。测试截图与报告需先整理到 `docs/baselines/`；不要把虚拟机磁盘、长视频或历次包永久堆放在 target 中。`-LegacyReports` 可同时移除旧 `.reasonix/`、`.playwright-mcp/` 本地采集缓存。

## 发版准备

新功能与仓库整理见 [v0.11.0 版本说明](releases/0.11.0.md)。`Cargo.toml` 是 CLI、关于页、PE 和 NSIS 的版本来源。`windows/release-policy.json` 记录发布标签、许可和验收范围。

先在干净提交上完成正式检查，准备固定 NSIS，再生成系统安装器：

```powershell
./scripts/prepare-rust-packaging-tools.ps1 -OutputDirectory target/packaging-tools
$tools = Get-Content -Raw target/packaging-tools/tools.json | ConvertFrom-Json
# 本地候选，生成但不执行安装器
./scripts/package-rust-product.ps1 -Renderer software -OutputDirectory target/candidate-software -Makensis $tools.Compiler
# 发布策略批准后，生成两 renderer 的正式资产；要求干净提交和匹配的验证回执
./scripts/package-rust-release.ps1 -OutputDirectory target/release-0.11.0 -Makensis $tools.Compiler
```

输出目录必须全新。正式打包要求干净工作区及与当前 `HEAD` 对应的双渲染器验证回执。打包会重核 exe、Cargo 元数据、输入文件清单及 SHA-256。新增、删除或修改输入后需要重新验证。保留真实 AboutSlint，Slint 使用 Royalty-free 2.0。系统安装、更新校验、首次数据快照、失败恢复与旧 Go 回退逻辑继续保留。

README 的 Rust Windows CI 徽章对应两个渲染器的托管检查。推送已批准的版本 tag 后，托管 `windows-2025` runner 另行完成正式构建、打包和发布。完整桌面冒烟和隔离客户机验收用于准备版本的发行证据。操作见[推送 tag 发版](rust-slint-release-readiness.md#推送-tag-发版)。

在隔离客户机验证新装、升级、失败恢复和卸载；不得用宿主正式安装作测试。正式资产需包含先上传的 `velo-launcher-amd64-installer.exe`（默认 Software 安装器的同字节副本）、其余安装器、SHA-256 清单和发行清单对应版本的 `velo-rust-<版本>-licenses.zip`。发布器逐项比对 GitHub 上的 SHA-256 后才将 Release 草稿公开。
