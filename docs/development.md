# Rust 开发与发布

[返回首页](../README.md) · [Rust 工程说明](../rust/README.md) · [旧版历史说明](legacy-go-wails.md)

从 0.1.0 起，Rust + Slint 是唯一产品开发主线。Go/Wails 0.9.x 停止维护；保留的 Go oracle 仅用于数据和行为契约回归。

当前版本为 **0.10.0**，发布标签为 **v0.10.0**，接续 Go 0.9.x 的版本顺序，见 [版本说明与迁移指引](releases/0.10.0.md)。CI 继续执行两种渲染器的完整检查，只在策略批准当前版本时打包正式资产。已发布的 0.1.0 标签和证据保持原身份。

## 环境

Windows 10/11 x64、PowerShell 7.4+，以及工程锁定的 Rust **1.99.1-x86_64-pc-windows-msvc**、MSVC **14.50.35717**、编译器 **19.50.35719.0**、链接器 **14.50.35719.0** 和 Windows SDK **10.0.26100.0**。先安装 rustup 及匹配的 Visual Studio C++ 组件，详细设置见 [Rust 工程说明](../rust/README.md)。

完整契约测试另需 Go **1.25.6**；产品构建和运行不依赖 Go/Wails、Node.js 或 WebView2。每份构建恰好启用一种渲染器，Software 为默认，FemtoVG 为替代。

## 验证与本地运行

以下命令在仓库根目录运行：

```powershell
$env:GOTOOLCHAIN = 'go1.25.6'
# 正式身份：格式、clippy、Rust/契约/原生测试、两份 Release 与组件冒烟
./scripts/verify-rust.ps1 -Product -SmokeTest
# 独立预览身份：使用隔离数据，供日常界面开发
./scripts/verify-rust.ps1 -SmokeTest
```

正式构建输出为 `rust/target/product-artifacts/<renderer>/velo-slint.exe`。正式普通启动要求系统安装回执与 HKLM64 注册位置匹配，应通过安装器使用。开发时使用预览构建，或同时指定独立的 `--instance-id` 和 `--data-dir`；不得指向正式数据或其子目录。运行参数、线程和模块边界见 Rust 工程说明。

`-SkipChecks` 仅供迭代，不产生可发布的完整验证证据。真实输入法、DPI、托盘、UAC 和安装器交互仍需相应桌面验证；不要用消息注入替代真实 IME 结论。

## 正式打包

`rust/Cargo.toml` 的版本是 CLI、关于页、PE 和 NSIS 的共同来源。`rust/windows/release-policy.json` 记录精确标签、默认渲染器、技术验收和发布授权；后续版本必须同步更新。标签支持 `X.Y.Z` 或 `vX.Y.Z`，打包与安装元数据只在版本比较时去掉小写 `v`，保留真实标签。首个 Rust 标签为 `0.1.0`，当前为 `v0.10.0`。

先提交并在干净源码提交上完成正式检查，然后准备固定 NSIS 并打包：

```powershell
./scripts/prepare-rust-packaging-tools.ps1 -OutputDirectory rust/target/packaging-tools-new
$tools = Get-Content -Raw rust/target/packaging-tools-new/tools.json | ConvertFrom-Json
./scripts/package-rust-release.ps1 -OutputDirectory rust/target/release-new -Makensis $tools.Compiler
```

输出目录必须全新。打包校验完整检查/冒烟回执、exe、metadata 和全部源码指纹，收集依赖许可及明确许可选项，生成两个系统安装器、许可 ZIP、构建清单和 SHA256。没有独立免安装版。内部候选继续使用 `package-rust-product.ps1`；只有显式 `-Release` 且策略批准时才接受正式包元数据。

发布时先上传 `velo-launcher-amd64-installer.exe`，再上传其余资产。此文件必须与 `velo-rust-x64-installer.exe` 字节和 SHA-256 完全相同；旧 Go 更新器选择第一个安装包，发布前后都要检查 GitHub API 和公开资产列表中的第一个 `-installer.exe` 是该 Software 副本，不能将校验清单等非安装资产计入。发布后运行 `go run ./cmd/check-legacy-update -download-dir rust/target/legacy-public-check`，另用 `-feed` 和新目录核对限流备用通道；命令只下载和校验，不执行安装。禁止提供名为 `velo-launcher.exe` 的裸 Rust 正式构建充当免安装迁移包。

在隔离测试客户机验证最终安装包的新装、原位接管、升级、失败恢复、回退和卸载。不要在宿主机正式安装中运行测试。只有验证后的精确文件才进入 GitHub Release，保留校验值与构建身份。发布采用正式 Release，并显式设为 latest，避免旧 Go 的数值更大版本影响 GitHub 的默认选择。

`.github/workflows/ci.yml` 使用具有锁定原生工具链的 Windows runner 验证 Rust，旧 Go 工作流只保留手动历史复现入口。CI 不会自动发布，也不在 runner 上运行系统接管测试。

## 数据与恢复

系统安装、卸载与回退共享 `Global\Velo.MachineInstaller` 互斥锁，覆盖不同 renderer 和目标目录使用的共同 HKLM 注册。忙时返回 33 并保持程序、注册与事务记录不变；进程异常终止后，新持有者仍按持久化 journal 恢复。最终安装器验收包含真实 NSIS 的跨进程互斥拒绝。

正式数据为 `%LOCALAPPDATA%\Velo`；有效旧配置读取后不重写。数据层打开前为每个安装事务建立 `VeloRustMigrationBackups` 原文快照，失败时拒绝初始化。更新安装前仍要求 SHA-256 校验与配置备份成功，安装版通过 UAC 系统安装器更新，保持当前 renderer。

安装目录中的 `.velo-rollback` 保存程序、注册与快捷方式恢复档案；有原始 Go 快照时，`uninstall.exe /ROLLBACK` 可恢复旧程序。普通卸载保留用户数据、未知文件和恢复档案。用户启动实例以普通权限运行，安装器不以管理员身份遍历用户数据。

## 证据与许可

[G4 当前机器验收](baselines/2026-11-07-slint-rs14/machine-completion-2026-11-14/g4-acceptance.md) 仅覆盖记录中的现有 Windows 11 宿主机和 Windows 10 客户机。历史性能和失败样本保持原构建身份，不把它们改标为最终发行构建。

Slint 使用 Royalty-free 2.0 并保留真实 AboutSlint；项目 MIT 不替代第三方条款。许可文本及其出处由打包和审计脚本检查，不修改原文以匹配新校验值。不要提交 `rust/target/`、本地日志、用户数据或凭据。
