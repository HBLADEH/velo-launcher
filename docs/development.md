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

### Workspace 双模式预览

从仓库根目录构建并运行隔离实例。备用快捷键打开短按工作台，实体前进侧键可用于短按或划向已绑定的方向。初次打开时可在 **设置 → 工作台 → 长按快捷盘** 分配方向。

```powershell
./scripts/verify-rust.ps1 -SmokeTest
./target/artifacts/software/velo-slint.exe --background --instance-id=workspace-v03 --data-dir=.dev-data/workspace-v03 --hotkey=Ctrl+Alt+K --workspace-preview=dual --workspace-xbutton=xbutton2
```

自动专项使用独立配置和合成侧键。按需将渲染器改为 `femtovg`，并为每轮指定新的输出目录。

```powershell
pwsh -NoProfile -STA -File ./scripts/test-workspace-preview.ps1 -Executable target/artifacts/software/velo-slint.exe -OutputDirectory target/workspace-v03-software -Mode dual -SideButton xbutton1 -Theme light -MeasureResources
```

调试工作台应用身份、触发与派发可附带 `--measurement-log=target/workspace-v03.log`。日志记录模式、方向和结果类别。 [实施说明](plans/velo-workspace-v0.3/implementation.md)

备用键冲突时可为隔离预览指定 `--workspace-hotkey=Ctrl+Alt+W`。用 `-Mode legacy` 验证旧配置经普通入口启动、首次保存、原文备份和禁用。`-Mode disabled` 验证默认关闭及两侧键透传。`-MeasureResources` 记录资源读数，双模式可附加 `-SideSamples 8` 采集延迟。`-Monitor` 按系统枚举顺序选屏，编号从 0 开始。两块显示器都启用时，可附加 `-CrossMonitors` 验证窗口跨屏复用和 DPI。

实体侧键复验时逐个运行渲染器。在普通应用窗口中检查短按、划向已绑定方向、回到中心和 `Esc` 取消。在日志中用 `workspace_side_release` 核对选择，用 `workspace_action_dispatched` 核对派发次数，用 `workspace_hidden` 核对收起原因与焦点归还。

- **浏览器透传**　在独立浏览器窗口准备两条历史记录。分别保存工作台禁用、浏览器按完整路径排除两种配置，用实体侧键检查前进和后退。清除排除规则并启用工作台后，再检查浏览器进入和退出 F11 全屏时侧键的去向。
- **驱动改键**　用鼠标驱动把当前绑定的侧键改为普通键盘按键，确认测试窗口收到按键且工作台保持隐藏。用备用热键呼出后，恢复鼠标默认映射，再确认侧键可呼出。
- **活动手势中拔插**　在副屏按住侧键并划至方向卡片高亮，在 8 秒长按上限内拔掉副屏。核对工作台在松开前以 `display_changed` 收起且派发次数为零。保持同一实例运行，在主屏分别用备用热键和侧键呼出。接回后在两屏复验定位和焦点归还。

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
