# RS-14 本地发行工程与包级回退续验

2026-11-12（北京时间）。按“所有不需人工介入的计划项继续处理”的目标，本轮补齐 Windows PE 资源、版本同步、依赖许可文本、实际 NSIS 内部预览安装器及 Go→Rust→Go 包级数据回退。**本机自动验证通过，G4 和正式发行仍未放行**；完整目标继续执行，未将本轮阶段交付改成全目标完成。

## 本轮已完成

- **版本与 Windows 资源。** `Cargo.toml` 的版本同时供 CLI、更新判断、关于页、PE File/ProductVersion、嵌入 manifest 与 NSIS 使用；不再在关于页写死版本。最终两 exe 的实际资源检查确认 `0.1.0`、渲染器标识、x64 Windows GUI、`asInvoker`、PerMonitorV2 和 longPathAware；直接导入表不含 WebView 或动态 VC runtime。检查不等同 Windows 10/其他驱动实机兼容。
- **许可文本。** 对两 renderer 的保守 Cargo resolved 清单，共 30 项缺少根许可的包补充 **41 份**精确版本/源码修订文本，保留 SHA、来源 URL 或 Cargo 内置 LICENSES 来源。最终两包 `MissingLicenseTexts` 都为 0；根文件缺失统计与补充收录分开，不把文本齐全当成嵌套 vendor/SPDX 选择或公开再分发审查通过。[文本来源](license-text-provenance.json)与[包清单](packages.json)可复查。
- **ZIP 与拒绝检查。** 两份当前包各拒绝跳过检查、缺 smoke、exe/元数据/源码哈希不匹配五类输入，逐项校验 **Software 586 / FemtoVG 590** 个 ZIP 文件。原有源文件的旧时间戳只在 staging 副本中归一化至 ZIP 支持范围，不修改 Cargo 缓存。
- **实际 NSIS 安装器。** 便携 NSIS 3.13 仅位于任务目录，经已锁定 archive/compiler SHA256 验证；没有全局安装编译器。两份实际安装器各通过七项：错误 `/D` 目标拒绝、中文空格路径安装、安装后程序实际窗口 smoke、锁定程序拒绝且原文件不变、备份后失败恢复原字节、升级替换、卸载保留未知文件和五份合成数据。测试用编译期精确目录边界，禁止系统卸载项/开始菜单写入；使用与内部候选相同的提取、替换、恢复和显式文件卸载逻辑。[Software](software-installer.json) / [FemtoVG](femtovg-installer.json)保存实际结果。
- **包级 Go→Rust→Go。** 由原冻结 Wails v0.9.11 程序创建隔离 Go portable ZIP，启动读取 1000 项合成目录；实际 NSIS 安装 Rust 到同一程序目录，Rust 读取同一数据后正常退出；卸载 Rust 保留原 Go 程序，再启动 Go 仍读到 1000 项。配置、历史、应用库和原始备份字节三阶段均不变。两条链路见 [Software](software-rollback.json) / [FemtoVG](femtovg-rollback.json)。这是实际包转换和读取回退，写入兼容继续由现有 Go/Rust 双向契约验证支撑；没有据此声称正式管理员 NSIS 升级已经验收。
- **内部候选与 CI 准备。** 两份未限制测试目标目录的每用户内部候选已编译，分别见 [Software](software-candidate.json) / [FemtoVG](femtovg-candidate.json)，未执行。手动 workflow 已加入固定 NSIS 准备及两 renderer 实际隔离安装器测试。远端只读查询确认 **self-hosted runner 数量为 0**，[远端状态](remote-runners.json)另存；没有推送、注册 runner、触发远端 CI 或发布。

## 验证与原件

[最终构建](build-identities.json)逐项核对两 renderer 各 **206 项输入**及验证回执，包含新增 Windows/许可资源。`verify-rust.ps1 -SmokeTest`、各 **80 项 Rust 测试、22/7/33 项输入/业务/页面断言**、格式/clippy、Go/Rust 数据、NTFS 失败保持、原生/启动及 Release 通过。PE metadata 检查已进入正式验证入口，任何失败不留下完整成功回执。旧版 Go 1.25.6 的 `verify.ps1 -SkipBuild`，含 Go/Vue 及共享采集工具回归，也通过，[验证日志 SHA](verification.json)保留本机原件身份。

首次 FemtoVG 包揭示它独有的 `i-slint-renderer-femtovg` 尚缺补充文本；该旧包测试与日志保留，补齐后重新完整验证并生成最终包，最终没有缺文本项。下载 NSIS 的第一次请求返回 HTML，哈希拒绝，没有执行；采用官方 SourceForge 直链取得与固定分发哈希匹配的 archive，最终 SHA256 见[工具身份](nsis-identity.json)。

安装器先将原 exe 移作备份，替换失败恢复旧字节；卸载仅使用生成的文件清单，**不递归删除安装目录或用户数据**。正式候选会建立独立 `Velo Rust Preview` 每用户入口/卸载项，仍使用 `VeloSlintPreview` 数据；本轮只编译，未执行这些注册分支。测试版锁定安装目录并抑制注册/快捷方式，因此不会将它的结果扩大为系统注册或 UAC 验收。

二进制、ZIP、编译/安装日志和测试数据留在 `rust/target/rs14-*-20261112/`，不加入 Git；本目录只保存可公开的哈希、结果和资源。原始 seed 与正式数据不同；公开结果不包含探针 endpoint token、个人查询或文件选择器内容。

## 后续范围与目标状态

原 RS-12 的冻结测量已完成，旧“0/9 冷样本”等状态保留在旧日期原件，不再列为当前未完成项。RS-13 最新自主原生结果见[48 段/14 类续验](../../2026-11-07-slint-rs13/completion-2026-11-12/autonomous-followup/README.md)，对应原 exe 身份，不改标为本轮带资源的新构建物理输入或性能证据。

[剩余工作审计](remaining-work.json)保留完整目标。可继续自主推进的项包括：当前构建的 renderer 资源/兼容证据评审、许可选项及嵌套 vendor 审查、独立原生曲线与视觉取舍判断、正式模式安装/更新集成的本地准备。用户暂不可操作的真实 IME/缩放、管理员 UAC/登录会话、未提供的 Windows 10/其他驱动机器，以及正式发布/远端 runner 配置，保持条件和证据边界；不反复请求用户参与。

默认 renderer 未选择、`RedistributionCleared=false`、G4 未放行。未安装或替换正式 Velo，未提交、推送或发布。[清理](cleanup.json)核对包测试前后五项正式文件状态和预览卸载注册状态一致，测试程序均已退出；该哈希括号从包测试前开始，不夸大覆盖更早未括住的构建阶段。

本目录[证据审计](evidence-audit.json)、[工具来源](tool-identities.json)和 [SHA256SUMS.public.txt](SHA256SUMS.public.txt)可复查。
