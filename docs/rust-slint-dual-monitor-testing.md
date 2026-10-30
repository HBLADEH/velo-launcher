# Rust + Slint 双屏电脑复验清单

交接日期：2026-10-30（北京时间）。分支：`codex/rust-slint-refactor`。当前 RS-01 至 RS-05、RS-07 已完成本机 Windows 11 验证，RS-06 的物理多屏/混合 DPI、拔插、休眠和实际通知区入口仍待补验，G2 尚未放行。详细状态见 [重构计划](rust-slint-refactor-plan.md)、[RS-06](baselines/2026-10-28-slint-rs06/README.md)与 [RS-07 补验](baselines/2026-10-29-slint-rs07/followup-2026-10-30/README.md)。

本次交接包含源码、锁定依赖、构建脚本及公开证据。`rust/target/`、`.reasonix/`、原始日志、测试数据和 exe 不随 Git 提交，另一台电脑需要本地构建。现有 Go 产品与数据不替换，业务迁移尚未开始。

## 获取与构建

在仓库根目录执行：

```powershell
git fetch origin
git switch codex/rust-slint-refactor
git pull --ff-only origin codex/rust-slint-refactor
./scripts/verify-rust.ps1 -SmokeTest
```

先按 [Rust 工程说明](../rust/README.md#已锁定的构建环境)安装 Rust/Cargo 1.99.1、MSVC 14.50.35717 和 Windows SDK 10.0.26100.0。验证脚本查找匹配版本，缺少时会报错；其他 MSVC/SDK 组合尚未验证。此入口只要求 Rust 构建环境，不需要常驻 Go、Wails 或 WebView 服务。

成功后两份程序分别为 `rust/target/artifacts/software/velo-slint.exe` 与 `rust/target/artifacts/femtovg/velo-slint.exe`，各目录还有 `build-identity.json`、依赖 features 与许可证信息。保留本机生成的身份，不把前一台电脑的 exe SHA 改写成这一轮结果。Software 与 FemtoVG 都要验证，同一时刻只运行其中一份测试实例；默认渲染器仍留待 RS-14 决定。

## 正常模式启动与记录

从普通权限 PowerShell 启动，不加 `--isolated`；隔离模式没有托盘、热键、失焦隐藏或正常动态尺寸，不适合 RS-06 验收。

```powershell
$runRoot = Join-Path $PWD ('.reasonix/rs06-dual-' + (Get-Date -Format 'yyyyMMdd-HHmmss'))
New-Item -ItemType Directory -Path $runRoot | Out-Null

# 先测试 Software；导入、查询和主题只在当前进程保留。
& ./rust/target/artifacts/software/velo-slint.exe `
    --instance-id=rs06-dual-software "--data-dir=$runRoot/software-data" `
    --hotkey=Ctrl+Alt+K --diagnostics `
    "--measurement-log=$runRoot/software-events.log" 2> "$runRoot/software-stderr.log"

# Software 通过底栏或托盘“退出”结束后，再测试 FemtoVG。
& ./rust/target/artifacts/femtovg/velo-slint.exe `
    --instance-id=rs06-dual-femtovg "--data-dir=$runRoot/femtovg-data" `
    --hotkey=Ctrl+Alt+K --diagnostics `
    "--measurement-log=$runRoot/femtovg-events.log" 2> "$runRoot/femtovg-stderr.log"
```

实例标识与数据目录必须成对指定，原型拒绝 Go 的 `%LOCALAPPDATA%\Velo` 目录。使用 `Ctrl+Alt+K` 避开 Go 产品默认的 Alt+Space；若当前电脑也占用了它，可通过 `--hotkey` 换一个组合，并在报告中记录。每次启动的日志目标必须不存在，重复运行使用新的 `$runRoot` 或日志名。

关闭按钮、Alt+F4 和空查询 Esc 都只隐藏。显隐保留同一个 HWND；主动隐藏会结束导入页的失焦保持模式。测试结束请使用底栏或托盘“退出”，再核对无残留进程/托盘及热键。

## 必验场景

先记录系统版本、GPU/驱动、主副屏分辨率、缩放比例、排列方向、任务栏位置、输入法和每份 exe 身份；截图与日志按渲染器和场景分开保存。

| 场景 | 实际操作与通过边界 |
| --- | --- |
| 主屏/副屏首次显示 | 全新进程分别在指针位于主屏、副屏时启动，初始位置使用相应工作区；搜索框无需再次点击即可实际输入。第二次测试换新日志文件 |
| 跨屏拖动 | 从顶部空白区域拖过两个屏幕，结束后窗口留在目标屏；不能被拉回旧屏或跳出工作区。输入、查询和原 HWND 保留 |
| 混合 DPI | 两屏使用不同系统缩放，例如 100% / 150% 或 125% / 200%，在同一进程中往返拖动。记录原生 DPI 变化、文字/图标/候选框、布局及实际输入；不以 `SLINT_SCALE_FACTOR` 代替系统 DPI |
| 负坐标布局 | 操作者将副屏排到主屏左侧或上方，记录实际负坐标；重复拖动、显示和输入检查，测试后恢复原布局 |
| 动态高度和任务栏 | 主页、外观面板、导入页请求 640 × 660；`code` 查询有 2 项、请求高度 264，`codex` 为空结果、请求高度 210，大量结果最高 720。物理窗口服从当前屏幕工作区，受限时内容可滚动，不能覆盖任务栏或持续尺寸抖动 |
| 显隐和位置 | 拖到副屏后反复用测试热键显隐、输入和切换页面，位置与会话草稿保留；在同一进程内显示不应每次重新居中到主屏 |
| 物理拔插/屏幕移除 | 窗口在副屏时，由操作者拔出该屏或调整显示连接，窗口应回到现存工作区并可实际输入。记录是否物理拔插；几何注入不能代替这一项 |
| 睡眠与恢复 | 由操作者安排手动睡眠并唤醒，检查热键、托盘、窗口位置、DPI 及实际输入；未实际执行的场景继续记待验证 |
| 实际 Shell 托盘入口 | 用真实通知区图标测试左键呼出、右键打开/设置/退出与菜单取消，检查不同 DPI 下图标清晰度。记录是否使用通知区折叠面板；原生消息探针不代替实际点击 |
| 临时焦点及返回 | 微软拼音候选框、外观面板、导入页、原生文件选择/右键菜单/确认框操作后不误隐藏，返回搜索后能输入；普通搜索失焦到无关窗口仍隐藏 |
| 退出清理 | 每份程序都通过明确“退出”结束，无残留 `velo-slint` 进程、主窗口、托盘端点或测试快捷键注册 |

本轮主要补 RS-06，不要求重做内存矩阵或将任何渲染器标为默认。遇到未通过场景时保存当时的日志、截图和复现步骤；没有条件执行的项目明确记录待验证，不以其他场景替代。

## 可选原生快照

[rs06_native_probe.rs](../rust/examples/rs06_native_probe.rs)无参数运行时只读取显示器、主窗口与托盘注册状态。新电脑需要单独构建这个 example：

```powershell
. ./scripts/lib/rust-environment.ps1
$rustEnvironment = Initialize-VeloRustEnvironment
Push-Location $rustEnvironment.RustRoot
try {
    cargo build --release --locked --no-default-features --features renderer-software --example rs06_native_probe
    if ($LASTEXITCODE -ne 0) { throw 'Native probe build failed.' }
} finally { Pop-Location }

& ./rust/target/x86_64-pc-windows-msvc/release/examples/rs06_native_probe.exe |
    Set-Content -LiteralPath "$runRoot/native-before.txt"
```

跨屏拖动、拔插或恢复后再无参数运行，保存为不同文件名；对照实际显示器坐标、工作区、DPI、PID/HWND。探针的消息/几何注入参数不用于上述物理验收。

建议报告格式：渲染器与 SHA、系统/显示器配置、场景步骤、预期与实际结果、是否真实输入、截图/日志路径、退出清理结果。原始本机日志保留在 `.reasonix/`；后续整理成脱敏证据后更新 RS-06 状态，G2 仍由实际通过证据决定。
