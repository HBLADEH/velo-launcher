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

### Workspace 径向预览

从仓库根目录运行。`Ctrl+Alt+K` 呼出测试 Launcher，`Ctrl+Alt+Space` 呼出独立的径向工作台。可在测试实例的 **设置 → 工作台** 保存配置。下例显式启用 `XBUTTON2`，可省略 `--workspace-xbutton` 单独体验键盘入口。

```powershell
./target/artifacts/software/velo-slint.exe --background --instance-id=workspace-radial --data-dir=.dev-data/workspace-radial --hotkey=Ctrl+Alt+K --workspace-preview=radial --workspace-xbutton=xbutton2 --measurement-log=target/workspace-radial-software.log
```

备用热键冲突时可指定 `--workspace-hotkey=Ctrl+Alt+W`。退出测试实例后，可在同一隔离目录用另一种渲染器复验，为每次运行指定独立日志。验证已保存的工作台开关、绑定和布局时，保留实例、数据目录和日志参数，去掉 `--workspace-preview`、`--workspace-xbutton` 和 `--workspace-hotkey`。

快捷键呼出时先显示 Logo，再展开扇区。可用 `--reduce-motion` 比较完整菜单直接到位的效果。长按划选直接显示当前区域，悬停、键盘焦点或手势高亮的动作名称显示在 Logo 下方。

桌面专项应逐次运行，每次使用新输出目录。`-SideButton` 通过 Windows 输入队列验证侧键。`-SettingsRoundTrip` 会逐项保存侧键选项并检查禁用后释放窗口、热键和线程。`-Mode disabled` 用没有工作台配置的隔离目录验证启动默认值及两侧键透传。`-Monitor` 按系统枚举顺序选屏，编号从 0 开始。两屏启用时可加 `-CrossMonitors`，验证同一个窗口在另一屏呼出后返回起始屏。用 `result.json` 中的 `MonitorPrimary`、`MonitorBounds` 与 `EffectiveDpi` 核对起始环境。

```powershell
pwsh -NoProfile -STA -File ./scripts/test-workspace-preview.ps1 -Executable target/artifacts/software/velo-slint.exe -OutputDirectory target/workspace-radial-software -Mode radial -SideButton xbutton1 -SettingsRoundTrip
```

槽位编辑专项单独使用 `-EditSlots`，检查拖入空弧片、方向键选择后的交换与 `Ctrl+S` 保存。`-MeasureResources` 会记录使用前、可见和隐藏的资源读数。加 `-SideButton xbutton1 -SideSamples 8` 可同时采集手势延迟，原版和关闭工作台使用 `-Mode disabled -MeasureResources`。

实体侧键复验时逐个运行渲染器。选择普通应用窗口，在两块屏幕中部检查短按、长按上划与下划、回 Logo 后松开及 Esc。在日志中用 `workspace_side_release` 核对选择结果，用 `workspace_action_dispatched` 核对派发次数，用 `workspace_hidden` 核对收起原因与焦点归还。按 [实施步骤](plans/velo-workspace-v0.2/implementation.md#p4--兼容验证与发布准备) 记录实际 Windows 版本、显示器坐标、缩放与构建身份。

- **浏览器透传**　在独立浏览器窗口准备两条历史记录。分别保存工作台禁用、浏览器按完整路径排除两种配置，用实体侧键检查前进和后退。清除排除规则并启用工作台后，再检查浏览器进入和退出 F11 全屏时侧键的去向。
- **驱动改键**　用鼠标驱动把当前绑定的侧键改为普通键盘按键，确认测试窗口收到按键且工作台保持隐藏。用备用热键呼出后，恢复鼠标默认映射，再确认侧键可呼出。
- **活动手势中拔插**　在副屏按住侧键并划至弧片高亮，在 8 秒长按上限内拔掉副屏。核对工作台在松开侧键前以 `display_changed` 收起，且本次无动作派发。保持同一实例运行，在主屏用备用热键和侧键分别呼出。接回副屏后记录工作区和缩放，在两屏重复呼出、定位和 Esc 焦点归还检查。

### Workspace P0 预览

构建后从仓库根目录运行以下命令。`Ctrl+Alt+K` 呼出测试实例的 Launcher，`Ctrl+Alt+Space` 呼出 Workspace。上区的「收起」用于验证焦点归还，下区的「高亮」用于验证点击。退出测试实例的托盘菜单后，可将 `shaped` 改为 `rounded` 比较轮廓，或更换渲染器产物。

```powershell
./target/artifacts/software/velo-slint.exe --background --instance-id=workspace-preview --data-dir=.dev-data/workspace-preview --hotkey=Ctrl+Alt+K --workspace-preview=shaped
```

热键被占用时可另选 `--workspace-hotkey=Ctrl+Alt+W`。桌面专项使用独立的测试窗口、热键与数据目录，检查原生命中、焦点和工作区，并记录屏幕采样。默认测第一个活动显示器，`-Monitor 1` 可选第二个。每轮用新的 PowerShell 进程和 `-OutputDirectory`，采样期间保持桌面空闲。

```powershell
pwsh -NoProfile -STA -File ./scripts/test-workspace-preview.ps1 -Executable target/artifacts/software/velo-slint.exe -OutputDirectory target/workspace-p0-software -Mode shaped
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
