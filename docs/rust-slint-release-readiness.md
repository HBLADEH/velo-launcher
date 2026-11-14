# Rust 构建、内部安装器与发行准备

正式发行构建与最终安装器已完成验证，见 [0.1.0 发行验证](baselines/2026-11-15-rust-0.1.0/README.md)。

2026-11-15 发布授权：所有者明确要求发布 Rust 首个正式版本，精确标签 `0.1.0`，并停止 Go/Wails 产品开发。现行 `release-policy.json` 记录 G4 当前机器验收、已完成的许可技术审查和本次公开发布授权。正式入口为 `verify-rust.ps1 -Product -SmokeTest` 与 `package-rust-release.ps1`，仅分发系统安装器；每份发行包重新绑定源码、exe、许可和 SHA256，旧包及下文历史状态不回写。具体步骤见[当前开发说明](development.md)。

2026-11-15：[RS-14 / G4 当前机器验收完成](baselines/2026-11-07-slint-rs14/machine-completion-2026-11-14/g4-acceptance.md)。正式身份、打开数据层前原文快照、系统接管/故障恢复/升级/Go回退/卸载、两 renderer 完整检查、普通权限会话、搜狗真实键盘及最终清理通过。范围只含现有宿主机及其 Windows10 客户机，不可用实体驱动等按用户要求跳过；原预算差异、历史失败和前轮 CI 身份保留。下文旧日期状态按原范围保留，不冒称新远端 CI 或公共发行许可通过；未发布或替换宿主机正式程序。

正式候选构建入口为 `./scripts/verify-rust.ps1 -Product -SmokeTest`；打包使用 `package-rust-product.ps1`，仅在 `rust/target` 生成内部候选。预览身份与正式身份回执不能混用。`prepare-rust-machine-install.ps1` 生成客户机输入和独立的替换后故障注入夹具；`test-rust-machine-install.ps1` 的 Prepare / Switch / Rollback / Cleanup 只允许测试虚拟机执行，夹具不进入产品包。程序沿用原系统安装位置，用户快照则由每个普通权限用户首次启动前在后台完成；卸载/回退不删除数据。正式命名不代表发布许可通过，`0.1.0` 和 `RedistributionCleared=false` 继续保留。

最新[本机 CI / Windows 10 续验](baselines/2026-11-07-slint-rs14/host-completion-2026-11-13/README.md)已完成真实 GitHub CI（[43370007545](https://github.com/HBLADEH/velo-launcher/actions/runs/43370007545)，临时分支，完整 success）和 Windows 10 Pro 22H2 虚拟机中两 renderer 的十组检查。runner 已退出并清理注册；“runner=0/未推送”只保留为前轮历史状态，不再是当前 CI 待办。原 GNU host 选择缺口与 NSIS HTTPS 降级拒绝分别修复为完整 MSVC host 锁定、同 SHA256 的离线包入口，原失败保留。正式 Go 系统安装/数据副本回退续验进度及剩余环境见[最新待办](baselines/2026-11-07-slint-rs14/host-completion-2026-11-13/remaining-work.json)，G4仍未放行。

2026-11-13 [RS-14 续验](baselines/2026-11-07-slint-rs14/completion-2026-11-13/README.md)已选择 **Software 为已测 Windows 11 x64 的默认 renderer**，FemtoVG 保留为显式替代构建。当前 exe 的三态/100 次显隐补验、许可选项与嵌套声明、真实每用户注册/卸载、两条包级 Go/Rust 回退，以及两 renderer 各一组真实 UAC 确认/取消均已执行；最终检查与适用构建见新证据。远端 runner 仍为 0、Rust workflow 未推送，Windows 10/其他驱动等未实测，**RS-14/G4 继续保留这些待验项**。

下文 2026-11-12 数字为原检查点，不能替代本轮程序与安装器身份。

2026-11-12 [本地发行续验](baselines/2026-11-07-slint-rs14/completion-2026-11-12/README.md)完成 PE 版本/DPI/权限资源、版本同步、许可文本补充、实际 NSIS 内部安装器和两条 Go/Rust 包级回退。两种构建各 206 项输入、完整 Rust 检查及旧版 Go/Vue 回归通过。默认 renderer、公开再分发、正式管理员安装和 G4 仍未放行；当前原型/预览继续隔离正式 Velo 数据。

## 可重复的本地命令

在仓库根目录使用锁定 Rust/MSVC/Windows SDK 和 PowerShell 7.4+。所有输出目录必须全新：

```powershell
$env:GOTOOLCHAIN = 'go1.25.6'
./scripts/verify-rust.ps1 -SmokeTest
./scripts/test-rust-preview.ps1 -Renderer software -OutputDirectory rust/target/preview-software
./scripts/test-rust-preview.ps1 -Renderer femtovg -OutputDirectory rust/target/preview-femtovg
./scripts/audit-rust-licenses.ps1 -Renderer software -OutputPath rust/target/preview-software/license-audit.json
./scripts/audit-rust-licenses.ps1 -Renderer femtovg -OutputPath rust/target/preview-femtovg/license-audit.json

./scripts/prepare-rust-packaging-tools.ps1 -OutputDirectory rust/target/packaging-tools
$tools = Get-Content -Raw rust/target/packaging-tools/tools.json | ConvertFrom-Json
./scripts/test-rust-installer.ps1 -Renderer software -OutputDirectory rust/target/install-software -Makensis $tools.Compiler
./scripts/test-rust-installer.ps1 -Renderer femtovg -OutputDirectory rust/target/install-femtovg -Makensis $tools.Compiler
./scripts/test-rust-user-install.ps1 -Renderer software -OutputDirectory rust/target/user-install-software -Makensis $tools.Compiler
./scripts/test-rust-user-install.ps1 -Renderer femtovg -OutputDirectory rust/target/user-install-femtovg -Makensis $tools.Compiler
```

`verification.json` 绑定 exe、构建身份、Cargo metadata 与完整检查/smoke 范围；开始验证时删除旧回执，失败不会留下新的成功回执。打包再次核对全部源码和元数据；`SkipChecks`、缺 smoke 或任一哈希变化会在生成包前拒绝。

`Cargo.toml` 的版本是 CLI、更新判断、关于页、PE File/ProductVersion、manifest 和 NSIS 的共同来源。当前 `0.1.0` 是重构内部版本，不是替代正式 Go v0.9.11 的发布标签。`verify-rust.ps1` 使用锁定 SDK 的 rc/mt/dumpbin 核对实际 PE：x64 Windows GUI、renderer 名称、普通用户权限、PerMonitorV2、longPathAware，以及没有直接 WebView/动态 VC runtime 导入。

## 内部 ZIP 与许可

ZIP 包含 exe、完整身份/回执、项目/资源/依赖许可、依赖 inventory 和内外两层 SHA256。解压后运行 `RUN-PREVIEW.ps1`，使用 `--instance-id=preview` 与 `%LOCALAPPDATA%\VeloSlintPreview`；两种 renderer 共用该预览身份/数据，切换前完全退出，避免热键争用。直接运行 exe 仍是 Rust prototype 隔离目录。

依赖清单保守包含 resolved build/optional 节点，不把它们全部当作运行时链接依赖。原 30 项/41 份根文本补充继续保留；11 月 13 日增加精确 ANGLE submodule 的 BSD 文本，当前补充共 42 份。每个包记录明确许可选项，AND 两侧条款同时保留；新增未审查表达式会拒绝打包。递归收集 3 份嵌套文本，并从 Khronos XML/IDL 保存 11 份内嵌声明及其源文件/提取文本 SHA。两包 `MissingLicenseTexts=0`，根文件缺失统计仍单列。Slint 选择 Royalty-free 2.0 并保留真实 AboutSlint；技术审查见[许可审查](baselines/2026-11-07-slint-rs14/completion-2026-11-13/license-review.md)。内部包仍为 `RedistributionCleared=false`，不自动发布。

## 实际 NSIS 与回退

便携 NSIS 版本和 archive/compiler SHA256 固定在 `rust/windows/nsis-toolchain.json`。准备脚本只解压到 `rust/target`，不全局安装。下载返回 HTML 或哈希不一致时拒绝执行。安装器脚本由 Rust 工程独立提供，不依赖 Go/Wails 或 WebView 安装逻辑。

网络返回 HTTPS→HTTP 重定向时仍然拒绝降级。已有下载包或 CI 预缓存可使用 `prepare-rust-packaging-tools.ps1 -ArchivePath <ZIP完整路径>`，或在 runner 进程设置 `VELO_NSIS_ARCHIVE`；两条入口均按同一个固定 archive SHA256 校验，再核对解压后的 compiler SHA256/版本。错误本地 ZIP 在创建输出前拒绝，不能用离线入口跳过完整性检查。

`test-rust-installer.ps1` 编译实际 NSIS，并将安装目录编译期固定到一个测试路径，禁止系统卸载项和快捷方式写入。八个检查覆盖：错误目标拒绝、中文空格路径、安装后程序窗口 smoke、锁定程序拒绝、**锁定卸载在删除 payload 前拒绝**、失败恢复旧 exe 精确字节、升级替换、卸载保留未知文件与五份合成数据。安装先备份旧 exe，失败还原；卸载只删除生成清单中的文件，不递归删除安装目录或用户数据。NSIS 编译使用 `/WX`，不再忽略卸载命令引号等警告。

可为审查编译不固定测试目录的内部候选，以下命令只生成安装器，不执行：

```powershell
./scripts/package-rust-installer.ps1 -PreviewContent rust/target/preview-software/valid/content -OutputDirectory rust/target/candidate-software -Makensis $tools.Compiler
```

内部候选默认安装到 `%LOCALAPPDATA%\Programs\VeloSlintPreview`，使用独立的每用户开始菜单/卸载项及预览数据目录。`test-rust-user-install.ps1` 已执行不禁用注册的实际候选：程序目录通过 `/D` 留在任务目录，真实 HKCU/开始菜单分支执行并清理；默认预览数据未打开。x64 注册视图固定为 64 位，卸载命令正确引用中文空格路径。已有预览注册/快捷方式时测试拒绝覆盖。

真实 UAC 更新使用 `test-rust-nsis-update.ps1` 的 `Prepare → Schedule → Verify → Finalize`。Prepare 只生成具体 NSIS 包、生产更新脚本、原配置备份及受保护任务目录；Schedule 才弹出 UAC，由用户操作；Verify 检查确认替换或取消保留、原始备份、错误原因及后台重启；用户从托盘正常退出后 Finalize 恢复原权限。两个 renderer 的确认/取消共四组已在本机完成，不修改系统安全策略、不替换正式应用。它补齐的是**真实 NSIS 在隔离受保护目录的提权更新链路**，没有冒充正式 Go 管理员安装器的全系统迁移。

`test-rust-package-rollback.ps1` 接受已冻结的 Wails `latencyprobe` exe/合成 seed manifest（`Builds.wails`、`SeedDirectory`、`Inputs`、`ExpectedCatalogCount`），逐项核对身份后，创建隔离 Go portable ZIP → 实际 NSIS Rust → 保留 Go 程序回退。两种 renderer 均验证三阶段能读取同一 1000 项数据，配置/历史/应用库与原始备份字节不变；写入兼容由已有 Go/Rust 双向契约另证。该范围不冒充正式 Go NSIS→管理员 Rust 安装器的系统级迁移。

## CI 与尚未完成的门槛

[手动 Rust workflow](../.github/workflows/rust-preview.yml)仅 `workflow_dispatch`、`contents: read`，不自动运行外部 PR、不发布 Release。自托管 Windows x64 runner 需要 `velo-locked-toolchain` 标签、精确 Rust/MSVC/SDK、PowerShell 7.4+、Go 1.25.6、可交互且无其他 Velo 的桌面；单任务串行验证两 renderer，再测 ZIP、准备固定 NSIS、运行实际隔离安装器并上传内部产物。

2026-11-12 只读查询显示仓库自托管 runner 数为 0，远端也尚无这份本地 Rust workflow。没有注册 runner、推送或触发远端任务；本地 YAML/命令通过不记为远端 CI 通过。

RS-12 九份独立冷样本及测量矩阵已按[本机冻结程序验收](baselines/2026-11-04-slint-rs12/completion-2026-11-11/README.md)完成，不再将旧的 0/9 作为当前待办；FemtoVG 内存超预算、慢查询与历史失焦原因未知保留。RS-13 的 48 段原生/43 张页面见[自主续验](baselines/2026-11-07-slint-rs13/completion-2026-11-12/autonomous-followup/README.md)，保留原 exe 身份与精确曲线/物理输入限制。

最新[剩余工作](baselines/2026-11-07-slint-rs14/host-completion-2026-11-13/remaining-work.json)取代此前审计作为当前入口。renderer 决策、RS-13 本机视觉/真实输入、许可选项与嵌套 vendor、实际每用户注册与真实 NSIS UAC 链路、真实 GitHub CI、Windows 10 虚拟机自动组件/安装检查，以及官方 Go 系统安装到独立 Rust 数据副本的回退和末尾清理已补齐。该手动副本路径保留 Go 和原始备份；自动原位接管的正式 Rust 系统安装器尚未实现，其他实体 GPU/第三方 IME 没有通过结论。临时源码分支/runner 的外部写入由用户明确授权，runner任务后清理；未发布或替换宿主机正式程序。
